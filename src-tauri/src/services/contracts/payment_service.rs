use crate::core::dtos::{RecordPaymentPayload, RecordRefundPayload};
use crate::core::error::AppError;
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
}
