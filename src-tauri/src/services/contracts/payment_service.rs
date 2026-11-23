use crate::core::dtos::{RecordPaymentPayload, RecordRefundPayload};
use crate::core::error::AppError;
use crate::core::value_objects::{PaymentKind, TransactionType};
use crate::data::contracts::unit_of_work::TransactionContext;

/// The only writer of the payment ledger and of the cached bill money fields
/// (`paid_amount`, `balance_due`, `payment_status`). See DD-01 and LLD §6.4.
pub trait PaymentService: Send + Sync {
    /// Balance-reducing entry (kind Upfront | Settlement | ReturnCredit) against one bill:
    /// rounds the amount, rejects `<= 0` or more than the balance, inserts the ledger row and
    /// recomputes the bill header from the ledger. Returns the settlement id.
    fn record_payment(
        &self,
        uow: &mut dyn TransactionContext,
        payload: RecordPaymentPayload,
    ) -> Result<i64, AppError>;

    /// Cash refund ledger row (RefundOut for a Sale return, RefundIn for a Purchase return).
    /// Never changes any bill balance.
    fn record_refund(
        &self,
        uow: &mut dyn TransactionContext,
        payload: RecordRefundPayload,
    ) -> Result<i64, AppError>;

    /// FR-20: deletes the bill's ledger rows of `kinds`, recomputes the bill header from what is
    /// left and returns the cash removed (Σ UPFRONT + SETTLEMENT among the deleted rows).
    fn delete_bill_ledger(
        &self,
        uow: &mut dyn TransactionContext,
        tx_type: TransactionType,
        tx_id: i64,
        kinds: &[PaymentKind],
    ) -> Result<f64, AppError>;

    /// FR-20: re-points the bill's UPFRONT and SETTLEMENT rows to another party.
    fn repoint_bill_party(
        &self,
        uow: &mut dyn TransactionContext,
        tx_type: TransactionType,
        tx_id: i64,
        party_id: Option<i64>,
    ) -> Result<(), AppError>;

    /// FR-20: rebuilds the bill's cached money fields from its ledger (DD-01).
    fn recompute(&self, uow: &mut dyn TransactionContext, tx_type: TransactionType, tx_id: i64) -> Result<(), AppError>;

    /// FR-20: deletes every ledger row booked by a return and recomputes each bill that had a
    /// `RETURN_CREDIT`. Returns `(cash removed, credit removed)`.
    fn delete_return_ledger(&self, uow: &mut dyn TransactionContext, return_id: i64) -> Result<(f64, f64), AppError>;
}

impl<T: PaymentService + ?Sized> PaymentService for std::sync::Arc<T> {
    fn record_payment(
        &self,
        uow: &mut dyn TransactionContext,
        payload: RecordPaymentPayload,
    ) -> Result<i64, AppError> {
        (**self).record_payment(uow, payload)
    }

    fn record_refund(
        &self,
        uow: &mut dyn TransactionContext,
        payload: RecordRefundPayload,
    ) -> Result<i64, AppError> {
        (**self).record_refund(uow, payload)
    }

    fn delete_bill_ledger(
        &self,
        uow: &mut dyn TransactionContext,
        tx_type: TransactionType,
        tx_id: i64,
        kinds: &[PaymentKind],
    ) -> Result<f64, AppError> {
        (**self).delete_bill_ledger(uow, tx_type, tx_id, kinds)
    }

    fn repoint_bill_party(
        &self,
        uow: &mut dyn TransactionContext,
        tx_type: TransactionType,
        tx_id: i64,
        party_id: Option<i64>,
    ) -> Result<(), AppError> {
        (**self).repoint_bill_party(uow, tx_type, tx_id, party_id)
    }

    fn recompute(&self, uow: &mut dyn TransactionContext, tx_type: TransactionType, tx_id: i64) -> Result<(), AppError> {
        (**self).recompute(uow, tx_type, tx_id)
    }

    fn delete_return_ledger(&self, uow: &mut dyn TransactionContext, return_id: i64) -> Result<(f64, f64), AppError> {
        (**self).delete_return_ledger(uow, return_id)
    }
}
