use crate::core::dtos::{PartyDueSummary, PartyLedgerView, SettlePaymentPayload, SettlementResult};
use crate::core::error::AppError;
use crate::core::value_objects::{PartyType, TransactionType};
use crate::data::contracts::unit_of_work::TransactionContext;

pub trait AccountingService: Send + Sync {
    /// Aggregates all outstanding dues for customers or vendors.
    fn get_party_dues_summary(
        &self,
        party_type: PartyType,
    ) -> Result<Vec<PartyDueSummary>, AppError>;

    /// Cash settlement (kind Settlement) in its own transaction: one specific bill, or FIFO
    /// across the party's oldest open bills. Works for both sides (SALE/CUSTOMER, PURCHASE/VENDOR).
    fn settle_bill_payment(
        &self,
        payload: SettlePaymentPayload,
    ) -> Result<SettlementResult, AppError>;

    /// Applies a return's value as RETURN_CREDIT inside the caller's transaction.
    /// `target_bill = Some(id)` credits that bill only; `None` credits the party's oldest open
    /// bills (FIFO); no party credits nothing. Returns the amount actually credited (<= value);
    /// the caller pays the remainder as cash.
    #[allow(clippy::too_many_arguments)]
    fn apply_return_credit(
        &self,
        uow: &mut dyn TransactionContext,
        tx_type: TransactionType,
        party_id: Option<i64>,
        target_bill: Option<i64>,
        return_id: i64,
        date: &str,
        value: f64,
    ) -> Result<f64, AppError>;

    /// Chronological ledger history for a party (bills, ledger rows, returns, running balance).
    fn get_party_ledger(
        &self,
        party_id: i64,
    ) -> Result<PartyLedgerView, AppError>;
}

impl<T: AccountingService + ?Sized> AccountingService for std::sync::Arc<T> {
    fn get_party_dues_summary(
        &self,
        party_type: PartyType,
    ) -> Result<Vec<PartyDueSummary>, AppError> {
        (**self).get_party_dues_summary(party_type)
    }

    fn settle_bill_payment(
        &self,
        payload: SettlePaymentPayload,
    ) -> Result<SettlementResult, AppError> {
        (**self).settle_bill_payment(payload)
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
        (**self).apply_return_credit(uow, tx_type, party_id, target_bill, return_id, date, value)
    }

    fn get_party_ledger(
        &self,
        party_id: i64,
    ) -> Result<PartyLedgerView, AppError> {
        (**self).get_party_ledger(party_id)
    }
}
