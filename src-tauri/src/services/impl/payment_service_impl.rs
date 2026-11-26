use crate::core::dtos::{RecordPaymentPayload, RecordRefundPayload};
use crate::core::entities::PaymentSettlement;
use crate::core::error::AppError;
use crate::core::money::{round2, MONEY_EPS};
use crate::core::value_objects::{PaymentKind, PaymentStatus, TransactionType};
use crate::data::contracts::unit_of_work::TransactionContext;
use crate::services::contracts::payment_service::{BillUpfront, PaymentService};

#[derive(Clone, Copy, Debug, Default)]
pub struct PaymentServiceImpl;

impl PaymentServiceImpl {
    pub fn new() -> Self {
        Self
    }
}

fn validation(field: &str, message: impl Into<String>) -> AppError {
    AppError::ValidationError { field: field.to_string(), message: message.into() }
}

/// `(balance_due, party_id)` of the bill a ledger row is booked against.
fn load_bill(
    uow: &mut dyn TransactionContext,
    tx_type: TransactionType,
    tx_id: i64,
) -> Result<(f64, Option<i64>), AppError> {
    match tx_type {
        TransactionType::Sale => {
            let sale = uow.sales().find_transaction_by_id(tx_id)?.ok_or_else(|| AppError::NotFound {
                entity: "SaleTransaction".to_string(),
                id: tx_id.to_string(),
            })?;
            Ok((sale.balance_due, sale.party_id))
        }
        TransactionType::Purchase => {
            let pur = uow.purchases().find_transaction_by_id(tx_id)?.ok_or_else(|| AppError::NotFound {
                entity: "PurchaseTransaction".to_string(),
                id: tx_id.to_string(),
            })?;
            Ok((pur.balance_due, pur.party_id))
        }
    }
}

/// Rebuilds a bill's cached money fields from its ledger rows (DD-01, LLD §6.4). Idempotent.
///
/// `paid = Σ(UPFRONT + SETTLEMENT)`, `credit = Σ RETURN_CREDIT`, `balance = total − paid − credit`
/// (an overshoot is rejected), status `PAID` at zero balance, else `PARTIAL` once anything has
/// been paid or credited, else `UNPAID`. Called after every balance-reducing insert; this is the
/// only place the header money fields are written after the header is created.
pub fn recompute_bill(
    uow: &mut dyn TransactionContext,
    tx_type: TransactionType,
    tx_id: i64,
) -> Result<(), AppError> {
    let total = match tx_type {
        TransactionType::Sale => uow
            .sales()
            .find_transaction_by_id(tx_id)?
            .map(|s| s.total_amount)
            .ok_or_else(|| AppError::NotFound { entity: "SaleTransaction".to_string(), id: tx_id.to_string() })?,
        TransactionType::Purchase => uow
            .purchases()
            .find_transaction_by_id(tx_id)?
            .map(|p| p.total_amount)
            .ok_or_else(|| AppError::NotFound { entity: "PurchaseTransaction".to_string(), id: tx_id.to_string() })?,
    };
    let totals = uow.settlements().sum_by_kind_for_transaction(tx_type, tx_id)?;
    let paid = round2(totals.upfront + totals.settlement);
    let credit = round2(totals.return_credit);
    let mut balance = round2(total - paid - credit);
    if balance < -MONEY_EPS {
        return Err(validation("amount_paid", "Payment exceeds outstanding balance"));
    }
    if balance < 0.0 {
        balance = 0.0;
    }
    let status = if balance <= MONEY_EPS {
        PaymentStatus::Paid
    } else if paid + credit > MONEY_EPS {
        PaymentStatus::Partial
    } else {
        PaymentStatus::Unpaid
    };
    match tx_type {
        TransactionType::Sale => uow.sales().update_money_fields(tx_id, paid, balance, status),
        TransactionType::Purchase => uow.purchases().update_money_fields(tx_id, paid, balance, status),
    }
}

impl PaymentService for PaymentServiceImpl {
    fn record_payment(
        &self,
        uow: &mut dyn TransactionContext,
        payload: RecordPaymentPayload,
    ) -> Result<i64, AppError> {
        match payload.kind {
            PaymentKind::Upfront | PaymentKind::Settlement | PaymentKind::ReturnCredit => {}
            other => {
                return Err(validation(
                    "kind",
                    format!("record_payment accepts Upfront, Settlement or ReturnCredit, got {other:?}"),
                ))
            }
        }
        if payload.kind == PaymentKind::ReturnCredit && payload.return_id.is_none() {
            return Err(validation("return_id", "return_id is required for a return credit"));
        }
        if !payload.amount_paid.is_finite() {
            return Err(validation("amount_paid", "Payment amount must be a finite number"));
        }
        let amount = round2(payload.amount_paid);
        if amount <= 0.0 {
            return Err(validation(
                "amount_paid",
                format!("Payment amount must be positive, got {}", payload.amount_paid),
            ));
        }

        let (balance, bill_party) = load_bill(uow, payload.transaction_type, payload.transaction_id)?;
        if amount > balance + MONEY_EPS {
            return Err(validation("amount_paid", "Payment exceeds outstanding balance"));
        }
        if let (Some(p), Some(b)) = (payload.party_id, bill_party) {
            if p != b {
                return Err(validation("party_id", "Payment party does not match the bill's party"));
            }
        }

        let settlement = PaymentSettlement {
            id: 0,
            settlement_date: payload.settlement_date,
            party_id: payload.party_id.or(bill_party),
            kind: payload.kind,
            transaction_type: payload.transaction_type,
            transaction_id: Some(payload.transaction_id),
            return_id: payload.return_id,
            amount_paid: amount,
            payment_mode: payload.payment_mode,
            notes: payload.notes,
            created_at: chrono::Utc::now().to_rfc3339(),
        };
        let id = uow.settlements().create_settlement(&settlement)?;
        recompute_bill(uow, payload.transaction_type, payload.transaction_id)?;
        Ok(id)
    }

    fn record_refund(
        &self,
        uow: &mut dyn TransactionContext,
        payload: RecordRefundPayload,
    ) -> Result<i64, AppError> {
        if !payload.amount.is_finite() {
            return Err(validation("amount", "Refund amount must be a finite number"));
        }
        let amount = round2(payload.amount);
        if amount <= 0.0 {
            return Err(validation("amount", format!("Refund amount must be positive, got {}", payload.amount)));
        }
        // Cash direction: a customer refund goes out, a vendor refund comes in. Refund rows never
        // touch a bill balance (DD-01).
        let kind = match payload.transaction_type {
            TransactionType::Sale => PaymentKind::RefundOut,
            TransactionType::Purchase => PaymentKind::RefundIn,
        };
        let settlement = PaymentSettlement {
            id: 0,
            settlement_date: payload.refund_date,
            party_id: payload.party_id,
            kind,
            transaction_type: payload.transaction_type,
            transaction_id: payload.transaction_id,
            return_id: payload.return_id,
            amount_paid: amount,
            payment_mode: payload.payment_mode,
            notes: payload.notes,
            created_at: chrono::Utc::now().to_rfc3339(),
        };
        uow.settlements().create_settlement(&settlement)
    }

    fn delete_bill_ledger(
        &self,
        uow: &mut dyn TransactionContext,
        tx_type: TransactionType,
        tx_id: i64,
        kinds: &[PaymentKind],
    ) -> Result<f64, AppError> {
        let rows = uow.settlements().delete_by_transaction(tx_type, tx_id, kinds)?;
        let cash: f64 = rows
            .iter()
            .filter(|r| matches!(r.kind, PaymentKind::Upfront | PaymentKind::Settlement))
            .map(|r| r.amount_paid)
            .sum();
        recompute_bill(uow, tx_type, tx_id)?;
        Ok(round2(cash))
    }

    fn repoint_bill_party(
        &self,
        uow: &mut dyn TransactionContext,
        tx_type: TransactionType,
        tx_id: i64,
        party_id: Option<i64>,
    ) -> Result<(), AppError> {
        uow.settlements()
            .update_party_for_transaction(tx_type, tx_id, party_id, &[PaymentKind::Upfront, PaymentKind::Settlement])
    }

    fn set_bill_upfront(&self, uow: &mut dyn TransactionContext, u: BillUpfront) -> Result<(), AppError> {
        if !u.amount.is_finite() || u.amount < 0.0 {
            return Err(validation("amount_paid", "Upfront amount must be 0 or more"));
        }
        let amount = round2(u.amount);
        let rows: Vec<PaymentSettlement> = uow
            .settlements()
            .list_by_transaction(u.tx_type, u.tx_id)?
            .into_iter()
            .filter(|s| s.kind == PaymentKind::Upfront)
            .collect();
        let (first, extra) = match rows.split_first() {
            Some((f, rest)) => (Some(f.clone()), rest.to_vec()),
            None => (None, Vec::new()),
        };
        // A bill carries one UPFRONT row; any older duplicates fold into the kept one.
        for r in &extra {
            uow.settlements().delete_settlement(r.id)?;
        }
        match first {
            Some(row) if amount <= 0.0 => uow.settlements().delete_settlement(row.id)?,
            Some(row) => {
                let changed = (row.amount_paid - amount).abs() > 1e-9
                    || row.settlement_date != u.date
                    || row.payment_mode != u.payment_mode;
                if changed {
                    uow.settlements().update_settlement(&PaymentSettlement {
                        amount_paid: amount,
                        settlement_date: u.date.clone(),
                        payment_mode: u.payment_mode.clone(),
                        ..row
                    })?;
                }
            }
            None if amount > 0.0 => {
                uow.settlements().create_settlement(&PaymentSettlement {
                    id: 0,
                    settlement_date: u.date.clone(),
                    party_id: u.party_id,
                    kind: PaymentKind::Upfront,
                    transaction_type: u.tx_type,
                    transaction_id: Some(u.tx_id),
                    return_id: None,
                    amount_paid: amount,
                    payment_mode: u.payment_mode.clone(),
                    notes: Some(u.notes.clone()),
                    created_at: chrono::Utc::now().to_rfc3339(),
                })?;
            }
            None => {}
        }
        recompute_bill(uow, u.tx_type, u.tx_id)
    }

    fn recompute(&self, uow: &mut dyn TransactionContext, tx_type: TransactionType, tx_id: i64) -> Result<(), AppError> {
        recompute_bill(uow, tx_type, tx_id)
    }

    fn delete_return_ledger(&self, uow: &mut dyn TransactionContext, return_id: i64) -> Result<(f64, f64), AppError> {
        let rows = uow.settlements().delete_by_return(return_id)?;
        let (mut cash, mut credit) = (0.0, 0.0);
        let mut touched: Vec<(TransactionType, i64)> = Vec::new();
        for r in &rows {
            match r.kind {
                PaymentKind::RefundOut | PaymentKind::RefundIn => cash += r.amount_paid,
                PaymentKind::ReturnCredit => {
                    credit += r.amount_paid;
                    if let Some(id) = r.transaction_id {
                        if !touched.contains(&(r.transaction_type, id)) {
                            touched.push((r.transaction_type, id));
                        }
                    }
                }
                _ => {}
            }
        }
        for (t, id) in touched {
            recompute_bill(uow, t, id)?;
        }
        Ok((round2(cash), round2(credit)))
    }
}
