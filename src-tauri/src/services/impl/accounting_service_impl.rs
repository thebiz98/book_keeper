use std::sync::Arc;
use crate::core::dtos::{
    PartyDueSummary, PartyLedgerView, RecordPaymentPayload, SettlePaymentPayload,
};
use crate::core::error::AppError;
use crate::core::value_objects::PartyType;
use crate::data::contracts::unit_of_work::UnitOfWork;
use crate::services::contracts::accounting_service::AccountingService;
use crate::services::contracts::payment_service::PaymentService;

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

    pub fn calculate_live_cash_drawer(&self, today: &str) -> Result<f64, AppError> {
        let mut tx = self.uow.begin_transaction()?;
        let mut drawer = 0.0;

        let settlements = tx.settlements().list_all()?;
        let mut settled_sales: std::collections::HashSet<i64> = std::collections::HashSet::new();
        let mut settled_purchases: std::collections::HashSet<i64> = std::collections::HashSet::new();

        // 1. Accumulate all date-matched settlements that used physical CASH
        for s in settlements {
            if s.settlement_date == today {
                let is_cash = s.payment_mode.as_deref() == Some("CASH");
                if !is_cash {
                    continue;
                }

                match s.transaction_type.as_str() {
                    "SALE" | "CUSTOMER_SETTLEMENT" => {
                        drawer += s.amount_paid.abs();
                        settled_sales.insert(s.transaction_id);
                    }
                    "PURCHASE" | "VENDOR_SETTLEMENT" => {
                        drawer -= s.amount_paid.abs();
                        settled_purchases.insert(s.transaction_id);
                    }
                    "SALE_REFUND" => {
                        drawer -= s.amount_paid.abs();
                    }
                    "PURCHASE_REFUND" => {
                        drawer += s.amount_paid.abs();
                    }
                    _ => {}
                }
            }
        }

        // 2. Include upfront cash from sales on `today` ONLY if no settlement was logged for them
        let sales = tx.sales().list_sales(
            Some(today),
            Some(today),
            None,
            None,
            0,
            1000,
        )?;
        for s in sales {
            if s.payment_mode.as_deref() == Some("CASH") && s.paid_amount > 0.0
                && !settled_sales.contains(&s.id) {
                    drawer += s.paid_amount;
                }
        }

        // 3. Deduct upfront cash from purchases on `today` ONLY if no settlement was logged for them
        let purchases = tx.purchases().list_purchases(
            Some(today),
            Some(today),
            None,
            None,
            0,
            1000,
        )?;
        for p in purchases {
            if p.payment_mode.as_deref() == Some("CASH") && p.paid_amount > 0.0
                && !settled_purchases.contains(&p.id) {
                    drawer -= p.paid_amount;
                }
        }

        Ok(drawer)
    }
}

impl<P: PaymentService, U: UnitOfWork> AccountingService for AccountingServiceImpl<P, U> {
    fn get_party_dues_summary(&self, party_type: PartyType) -> Result<Vec<PartyDueSummary>, AppError> {
        let mut tx = self.uow.begin_transaction()?;
        let parties = tx.parties().list_by_type(party_type, true)?;
        let mut summaries = Vec::new();
        for party in parties {
            let unpaid_sales = tx.sales().list_unpaid_by_party_id(party.id)?;
            let total_due: f64 = unpaid_sales.iter().map(|s| s.balance_due).sum();
            if total_due > 0.0 {
                let oldest_bill_date = unpaid_sales.first().map(|s| s.transaction_date.clone());
                summaries.push(PartyDueSummary {
                    party_id: party.id,
                    party_name: party.name,
                    phone: party.phone,
                    party_type,
                    total_due,
                    oldest_bill_date,
                    unpaid_bills_count: unpaid_sales.len(),
                });
            }
        }
        Ok(summaries)
    }

    fn settle_bill_payment(&self, payload: SettlePaymentPayload) -> Result<(), AppError> {
        let mut tx = self.uow.begin_transaction()?;
        if let Some(sale_id) = payload.specific_bill_id {
            let payment_payload = RecordPaymentPayload {
                party_id: Some(payload.party_id),
                transaction_type: "SALE".to_string(),
                transaction_id: sale_id,
                settlement_date: payload.settlement_date,
                amount_paid: payload.amount,
                payment_mode: payload.payment_mode,
                notes: payload.notes,
            };
            self.payment_service
                .record_payment(&mut *tx, payment_payload)?;
        } else {
            let unpaid = tx.sales().list_unpaid_by_party_id(payload.party_id)?;
            let mut remaining = payload.amount;
            for bill in unpaid {
                if remaining <= 0.0 {
                    break;
                }
                let settle_amount = bill.balance_due.min(remaining);
                let payment_payload = RecordPaymentPayload {
                    party_id: Some(payload.party_id),
                    transaction_type: "SALE".to_string(),
                    transaction_id: bill.id,
                    settlement_date: payload.settlement_date.clone(),
                    amount_paid: settle_amount,
                    payment_mode: payload.payment_mode.clone(),
                    notes: payload.notes.clone(),
                };
                self.payment_service
                    .record_payment(&mut *tx, payment_payload)?;
                remaining -= settle_amount;
            }
        }
        tx.commit()?;
        Ok(())
    }

    fn get_party_ledger(&self, party_id: i64) -> Result<PartyLedgerView, AppError> {
        let mut tx = self.uow.begin_transaction()?;
        let party = tx
            .parties()
            .find_by_id(party_id)?
            .ok_or_else(|| AppError::NotFound {
                entity: "Party".to_string(),
                id: party_id.to_string(),
            })?;
        let sales = tx.sales().list_unpaid_by_party_id(party_id)?;
        let total_due: f64 = sales.iter().map(|s| s.balance_due).sum();
        Ok(PartyLedgerView {
            party,
            current_balance: total_due,
            entries: Vec::new(),
        })
    }
}
