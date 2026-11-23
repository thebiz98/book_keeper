use std::sync::Arc;
use crate::core::dtos::{
    LedgerCashSummary, PartyDueSummary, PartyLedgerBill, PartyLedgerEntry, PartyLedgerView,
    RecordPaymentPayload, SettlePaymentPayload, SettlementResult,
};
use crate::core::error::AppError;
use crate::core::money::{cash_direction, check_date, round2, CashDirection, MONEY_EPS};
use crate::core::value_objects::{LotSource, PartyType, PaymentKind, TransactionType};
use crate::data::contracts::unit_of_work::{TransactionContext, UnitOfWork};
use crate::services::contracts::accounting_service::AccountingService;
use crate::services::contracts::payment_service::PaymentService;

fn validation(field: &str, message: impl Into<String>) -> AppError {
    AppError::ValidationError { field: field.to_string(), message: message.into() }
}

fn expected_party_type(tx_type: TransactionType) -> PartyType {
    match tx_type {
        TransactionType::Sale => PartyType::Customer,
        TransactionType::Purchase => PartyType::Vendor,
    }
}

/// Cash movement between `from` and `to` (inclusive), summed from the ledger by the DD-01 cash
/// direction table. The one function behind the live cash drawer and the cash-flow report.
pub fn ledger_cash_summary(
    tx: &mut dyn TransactionContext,
    from: &str,
    to: &str,
) -> Result<LedgerCashSummary, AppError> {
    let mut s = LedgerCashSummary::default();
    for (tx_type, kind, sum) in tx.settlements().sum_by_type_kind_between(from, to)? {
        match (tx_type, kind) {
            (TransactionType::Sale, PaymentKind::Upfront) => s.sale_upfront += sum,
            (TransactionType::Sale, PaymentKind::Settlement) => s.sale_settlement += sum,
            (TransactionType::Purchase, PaymentKind::RefundIn) => s.vendor_refund_in += sum,
            (TransactionType::Purchase, PaymentKind::Upfront) => s.purchase_upfront += sum,
            (TransactionType::Purchase, PaymentKind::Settlement) => s.purchase_settlement += sum,
            (TransactionType::Sale, PaymentKind::RefundOut) => s.customer_refund_out += sum,
            _ => debug_assert_eq!(cash_direction(tx_type, kind), CashDirection::None),
        }
    }
    s.cash_in = round2(s.sale_upfront + s.sale_settlement + s.vendor_refund_in);
    s.cash_out = round2(s.purchase_upfront + s.purchase_settlement + s.customer_refund_out);
    s.net = round2(s.cash_in - s.cash_out);
    Ok(s)
}

/// The one cash-drawer formula: `(ledger summary, expenses, net)` over `[from, to]` where
/// `net = ledger net - expenses` (DD-37: expenses are cash out of the drawer on their date).
pub fn cash_position(
    tx: &mut dyn TransactionContext,
    from: &str,
    to: &str,
) -> Result<(LedgerCashSummary, f64, f64), AppError> {
    let s = ledger_cash_summary(tx, from, to)?;
    let expenses = round2(tx.expenses().sum_between(from, to)?);
    let net = round2(s.net - expenses);
    Ok((s, expenses, net))
}

#[derive(Clone)]
pub struct AccountingServiceImpl<
    P: PaymentService = Arc<dyn PaymentService>,
    U: UnitOfWork = Arc<dyn UnitOfWork>,
> {
    pub payment_service: P,
    pub uow: U,
}

impl<P: PaymentService, U: UnitOfWork> AccountingServiceImpl<P, U> {
    pub fn new(payment_service: P, uow: U) -> Self {
        Self {
            payment_service,
            uow,
        }
    }

    /// Cash movement from the ledger over `[from, to]` (see [`ledger_cash_summary`]).
    pub fn ledger_cash_summary(&self, from: &str, to: &str) -> Result<LedgerCashSummary, AppError> {
        let mut tx = self.uow.begin_read()?;
        ledger_cash_summary(&mut *tx, from, to)
    }

    /// Net cash in the drawer for one day, derived purely from the ledger (DD-01).
    pub fn calculate_live_cash_drawer(&self, today: &str) -> Result<f64, AppError> {
        let mut tx = self.uow.begin_read()?;
        Ok(cash_position(&mut *tx, today, today)?.2)
    }

    /// Open bills `(id, balance_due)` of a party, oldest first (`transaction_date, id`).
    fn open_bills(
        tx: &mut dyn TransactionContext,
        tx_type: TransactionType,
        party_id: i64,
    ) -> Result<Vec<(i64, f64)>, AppError> {
        Ok(match tx_type {
            TransactionType::Sale => tx
                .sales()
                .list_open_for_party(party_id)?
                .into_iter()
                .map(|b| (b.id, b.balance_due))
                .collect(),
            TransactionType::Purchase => tx
                .purchases()
                .list_open_for_party(party_id)?
                .into_iter()
                .map(|b| (b.id, b.balance_due))
                .collect(),
        })
    }

    /// Spreads `amount` over the party's open bills oldest-first as ledger rows of `kind`.
    /// Stops when the amount is exhausted or the bills run out. Returns the amount applied.
    #[allow(clippy::too_many_arguments)]
    fn apply_fifo(
        &self,
        tx: &mut dyn TransactionContext,
        tx_type: TransactionType,
        party_id: i64,
        amount: f64,
        kind: PaymentKind,
        return_id: Option<i64>,
        date: &str,
        payment_mode: Option<String>,
        notes: Option<String>,
        result: &mut SettlementResult,
    ) -> Result<f64, AppError> {
        let mut remaining = round2(amount);
        let mut applied = 0.0;
        for (bill_id, balance) in Self::open_bills(tx, tx_type, party_id)? {
            if remaining <= MONEY_EPS {
                break;
            }
            let apply = round2(balance.min(remaining));
            if apply <= 0.0 {
                continue;
            }
            let id = self.payment_service.record_payment(
                tx,
                RecordPaymentPayload {
                    party_id: Some(party_id),
                    transaction_type: tx_type,
                    transaction_id: bill_id,
                    kind,
                    return_id,
                    settlement_date: date.to_string(),
                    amount_paid: apply,
                    payment_mode: payment_mode.clone(),
                    notes: notes.clone(),
                },
            )?;
            result.settlement_ids.push(id);
            result.bills_touched.push(bill_id);
            remaining = round2(remaining - apply);
            applied = round2(applied + apply);
        }
        Ok(applied)
    }
}

impl<P: PaymentService, U: UnitOfWork> AccountingService for AccountingServiceImpl<P, U> {
    fn get_party_dues_summary(&self, party_type: PartyType) -> Result<Vec<PartyDueSummary>, AppError> {
        let mut tx = self.uow.begin_read()?;
        // Header balances are ledger-consistent (PaymentService is their only writer); the
        // repository sums them per party for the right side (sales / PURCHASE-source purchases).
        let mut rows = tx.parties().get_party_due_summary(party_type)?;
        for r in &mut rows {
            r.total_invoiced = round2(r.total_invoiced);
            r.total_paid = round2(r.total_paid);
            r.balance_due = round2(r.balance_due);
        }
        Ok(rows)
    }

    fn settle_bill_payment(&self, payload: SettlePaymentPayload) -> Result<SettlementResult, AppError> {
        if !payload.amount_paid.is_finite() {
            return Err(validation("amount_paid", "Settlement amount must be a finite number"));
        }
        let amount = round2(payload.amount_paid);
        if amount <= 0.0 {
            return Err(validation("amount_paid", "Settlement amount must be positive"));
        }
        check_date("settlement_date", &payload.settlement_date)?;

        let tx_type = payload.transaction_type;
        let mut tx = self.uow.begin_transaction()?;

        let party = tx.parties().find_by_id(payload.party_id)?.ok_or_else(|| AppError::NotFound {
            entity: "Party".to_string(),
            id: payload.party_id.to_string(),
        })?;
        if party.party_type != expected_party_type(tx_type) {
            return Err(validation(
                "transaction_type",
                format!("{:?} settlements need a {:?} party", tx_type, expected_party_type(tx_type)),
            ));
        }

        let mut result = SettlementResult { settlement_ids: Vec::new(), bills_touched: Vec::new() };
        if let Some(bill_id) = payload.transaction_id {
            // Specific bill: must belong to the party (and be a PURCHASE-source bill for purchases).
            let owner = match tx_type {
                TransactionType::Sale => tx.sales().find_transaction_by_id(bill_id)?.map(|b| b.party_id),
                TransactionType::Purchase => tx
                    .purchases()
                    .find_transaction_by_id(bill_id)?
                    .filter(|b| b.source == LotSource::Purchase)
                    .map(|b| b.party_id),
            }
            .ok_or_else(|| AppError::NotFound {
                entity: match tx_type {
                    TransactionType::Sale => "SaleTransaction",
                    TransactionType::Purchase => "PurchaseTransaction",
                }
                .to_string(),
                id: bill_id.to_string(),
            })?;
            if owner != Some(payload.party_id) {
                return Err(validation("transaction_id", "Bill does not belong to this party"));
            }
            let id = self.payment_service.record_payment(
                &mut *tx,
                RecordPaymentPayload {
                    party_id: Some(payload.party_id),
                    transaction_type: tx_type,
                    transaction_id: bill_id,
                    kind: PaymentKind::Settlement,
                    return_id: None,
                    settlement_date: payload.settlement_date,
                    amount_paid: amount,
                    payment_mode: payload.payment_mode,
                    notes: payload.notes,
                },
            )?;
            result.settlement_ids.push(id);
            result.bills_touched.push(bill_id);
        } else {
            // Lump sum: reject over-settlement before any write.
            let outstanding: f64 = Self::open_bills(&mut *tx, tx_type, payload.party_id)?
                .iter()
                .map(|(_, b)| b)
                .sum();
            if amount > round2(outstanding) + MONEY_EPS {
                return Err(validation("amount_paid", "Settlement amount exceeds total outstanding dues"));
            }
            self.apply_fifo(
                &mut *tx,
                tx_type,
                payload.party_id,
                amount,
                PaymentKind::Settlement,
                None,
                &payload.settlement_date,
                payload.payment_mode,
                payload.notes,
                &mut result,
            )?;
        }
        tx.commit()?;
        Ok(result)
    }

    fn apply_return_credit(
        &self,
        uow: &mut dyn TransactionContext,
        tx_type: TransactionType,
        party_id: Option<i64>,
        target_bill: Option<i64>,
        return_id: i64,
        date: &str,
        value: f64,
    ) -> Result<f64, AppError> {
        if !value.is_finite() {
            return Err(validation("value", "Return value must be a finite number"));
        }
        let value = round2(value);
        if value <= 0.0 {
            return Ok(0.0);
        }
        if let Some(bill_id) = target_bill {
            // A linked return only ever touches its own bill (DD-03).
            let (balance, bill_party) = match tx_type {
                TransactionType::Sale => uow
                    .sales()
                    .find_transaction_by_id(bill_id)?
                    .map(|b| (b.balance_due, b.party_id)),
                TransactionType::Purchase => uow
                    .purchases()
                    .find_transaction_by_id(bill_id)?
                    .map(|b| (b.balance_due, b.party_id)),
            }
            .ok_or_else(|| AppError::NotFound {
                entity: match tx_type {
                    TransactionType::Sale => "SaleTransaction",
                    TransactionType::Purchase => "PurchaseTransaction",
                }
                .to_string(),
                id: bill_id.to_string(),
            })?;
            let credit = round2(value.min(balance));
            if credit > 0.0 {
                self.payment_service.record_payment(
                    uow,
                    RecordPaymentPayload {
                        party_id: party_id.or(bill_party),
                        transaction_type: tx_type,
                        transaction_id: bill_id,
                        kind: PaymentKind::ReturnCredit,
                        return_id: Some(return_id),
                        settlement_date: date.to_string(),
                        amount_paid: credit,
                        payment_mode: None,
                        notes: Some("Return credit".to_string()),
                    },
                )?;
            }
            return Ok(credit.max(0.0));
        }
        match party_id {
            Some(p) => {
                let mut sink = SettlementResult { settlement_ids: Vec::new(), bills_touched: Vec::new() };
                self.apply_fifo(
                    uow,
                    tx_type,
                    p,
                    value,
                    PaymentKind::ReturnCredit,
                    Some(return_id),
                    date,
                    None,
                    Some("Return credit".to_string()),
                    &mut sink,
                )
            }
            None => Ok(0.0),
        }
    }

    fn get_party_ledger(&self, party_id: i64) -> Result<PartyLedgerView, AppError> {
        let mut tx = self.uow.begin_read()?;
        let party = tx.parties().find_by_id(party_id)?.ok_or_else(|| AppError::NotFound {
            entity: "Party".to_string(),
            id: party_id.to_string(),
        })?;
        let tx_type = match party.party_type {
            PartyType::Customer => TransactionType::Sale,
            PartyType::Vendor => TransactionType::Purchase,
        };

        // Bills, both sides through the same shape (vendors: PURCHASE-source only).
        let bills: Vec<PartyLedgerBill> = match tx_type {
            TransactionType::Sale => tx
                .sales()
                .list_sales(None, None, None, Some(party_id), 0, usize::MAX)?
                .into_iter()
                .map(|b| PartyLedgerBill {
                    id: b.id,
                    transaction_type: tx_type,
                    transaction_code: b.transaction_code,
                    transaction_date: b.transaction_date,
                    total_amount: b.total_amount,
                    paid_amount: b.paid_amount,
                    balance_due: b.balance_due,
                    payment_status: b.payment_status,
                })
                .collect(),
            TransactionType::Purchase => tx
                .purchases()
                .list_purchases(None, None, None, Some(party_id), 0, usize::MAX)?
                .into_iter()
                .map(|b| PartyLedgerBill {
                    id: b.id,
                    transaction_type: tx_type,
                    transaction_code: b.transaction_code,
                    transaction_date: b.transaction_date,
                    total_amount: b.total_amount,
                    paid_amount: b.paid_amount,
                    balance_due: b.balance_due,
                    payment_status: b.payment_status,
                })
                .collect(),
        };
        let payments = tx.settlements().list_by_party(party_id)?;
        let returns: Vec<_> = tx
            .returns()
            .list_returns(None, 0, usize::MAX)?
            .into_iter()
            .filter(|r| r.party_id == Some(party_id))
            .collect();

        let code_of_bill = |id: i64| {
            bills.iter().find(|b| b.id == id).map(|b| b.transaction_code.clone())
        };
        let code_of_return = |id: i64| {
            returns.iter().find(|r| r.id == id).map(|r| r.return_code.clone())
        };

        // (date, rank, id) orders bills before returns before payments on the same day.
        let mut raw: Vec<(String, u8, i64, PartyLedgerEntry)> = Vec::new();
        for b in &bills {
            raw.push((
                b.transaction_date.clone(),
                0,
                b.id,
                PartyLedgerEntry {
                    date: b.transaction_date.clone(),
                    transaction_code: b.transaction_code.clone(),
                    transaction_type: format!("{:?}", b.transaction_type).to_uppercase(),
                    debit: b.total_amount,
                    credit: 0.0,
                    running_balance: 0.0,
                    notes: None,
                },
            ));
        }
        for r in &returns {
            raw.push((
                r.return_date.clone(),
                1,
                r.id,
                PartyLedgerEntry {
                    date: r.return_date.clone(),
                    transaction_code: r.return_code.clone(),
                    transaction_type: "RETURN".to_string(),
                    debit: 0.0,
                    credit: 0.0,
                    running_balance: 0.0,
                    notes: r.reason.clone().or_else(|| r.notes.clone()),
                },
            ));
        }
        for p in &payments {
            let credit = matches!(
                p.kind,
                PaymentKind::Upfront | PaymentKind::Settlement | PaymentKind::ReturnCredit
            );
            let code = p
                .transaction_id
                .and_then(code_of_bill)
                .or_else(|| p.return_id.and_then(code_of_return))
                .unwrap_or_default();
            raw.push((
                p.settlement_date.clone(),
                2,
                p.id,
                PartyLedgerEntry {
                    date: p.settlement_date.clone(),
                    transaction_code: code,
                    transaction_type: serde_json::to_value(p.kind)
                        .ok()
                        .and_then(|v| v.as_str().map(str::to_string))
                        .unwrap_or_default(),
                    debit: 0.0,
                    credit: if credit { p.amount_paid } else { 0.0 },
                    running_balance: 0.0,
                    notes: p.notes.clone(),
                },
            ));
        }
        raw.sort_by(|a, b| (&a.0, a.1, a.2).cmp(&(&b.0, b.1, b.2)));
        let mut running = 0.0;
        let entries: Vec<PartyLedgerEntry> = raw
            .into_iter()
            .map(|(_, _, _, mut e)| {
                running = round2(running + e.debit - e.credit);
                e.running_balance = running;
                e
            })
            .collect();

        let current_balance = round2(bills.iter().map(|b| b.balance_due).sum());
        Ok(PartyLedgerView { party, current_balance, bills, payments, returns, entries })
    }
}
