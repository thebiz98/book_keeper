use crate::core::dtos::{PartyDueSummary, PartyLedgerView, SettlePaymentPayload};
use crate::core::error::AppError;
use crate::core::value_objects::PartyType;

pub trait AccountingService: Send + Sync {
    /// Aggregates all outstanding dues for customers or vendors.
    fn get_party_dues_summary(
        &self,
        party_type: PartyType,
    ) -> Result<Vec<PartyDueSummary>, AppError>;

    /// Orchestrates bill settlement (specific bill or FIFO across oldest unpaid).
    fn settle_bill_payment(
        &self,
        payload: SettlePaymentPayload,
    ) -> Result<(), AppError>;

    /// Returns transaction-level ledger history for a party.
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
    ) -> Result<(), AppError> {
        (**self).settle_bill_payment(payload)
    }

    fn get_party_ledger(
        &self,
        party_id: i64,
    ) -> Result<PartyLedgerView, AppError> {
        (**self).get_party_ledger(party_id)
    }
}
