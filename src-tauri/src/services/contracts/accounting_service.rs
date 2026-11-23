use crate::core::dtos::{
    DeleteSettlementResult, PartyDueSummary, PartyLedgerView, SettleAllocation, SettleBatchPayload, SettlePaymentPayload,
    SettlementResult, UpdateSettlementPayload,
};
use crate::core::entities::PaymentSettlement;
use crate::core::error::AppError;
use crate::core::value_objects::{PartyType, TransactionType};
use crate::data::contracts::unit_of_work::TransactionContext;

pub trait AccountingService: Send + Sync {
    /// Outstanding dues per party (customers: sales; vendors: PURCHASE-source purchases) in one
    /// grouped query: open-bill count, earliest bill date, Σ invoiced / paid / balance. Not
    /// paginated (the screen totals the whole list).
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

    /// DD-56: the FIFO split a lump-sum `settle_bill_payment` of `amount` would book, oldest bill
    /// first. Same validation and allocation code as the command; writes nothing.
    fn preview_settlement_allocation(
        &self,
        party_id: i64,
        transaction_type: TransactionType,
        amount: f64,
    ) -> Result<Vec<SettleAllocation>, AppError>;

    /// D9: the one SETTLEMENT core. Several bills in one transaction, all or nothing.
    fn settle_bills_batch(&self, payload: SettleBatchPayload) -> Result<SettlementResult, AppError>;

    /// D8: edits a SETTLEMENT row (amount, date, mode, notes); other kinds are rejected.
    fn update_settlement(
        &self,
        settlement_id: i64,
        payload: UpdateSettlementPayload,
    ) -> Result<PaymentSettlement, AppError>;

    /// D8: deletes a SETTLEMENT row and recomputes its bill; other kinds are rejected.
    fn delete_settlement(&self, settlement_id: i64) -> Result<DeleteSettlementResult, AppError>;

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

    fn preview_settlement_allocation(
        &self,
        party_id: i64,
        transaction_type: TransactionType,
        amount: f64,
    ) -> Result<Vec<SettleAllocation>, AppError> {
        (**self).preview_settlement_allocation(party_id, transaction_type, amount)
    }

    fn settle_bills_batch(&self, payload: SettleBatchPayload) -> Result<SettlementResult, AppError> {
        (**self).settle_bills_batch(payload)
    }

    fn update_settlement(
        &self,
        settlement_id: i64,
        payload: UpdateSettlementPayload,
    ) -> Result<PaymentSettlement, AppError> {
        (**self).update_settlement(settlement_id, payload)
    }

    fn delete_settlement(&self, settlement_id: i64) -> Result<DeleteSettlementResult, AppError> {
        (**self).delete_settlement(settlement_id)
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
