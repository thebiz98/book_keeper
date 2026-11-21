use crate::core::dtos::{RecordPaymentPayload, RecordRefundPayload};
use crate::core::error::AppError;
use crate::core::value_objects::TransactionType;
use crate::data::contracts::unit_of_work::TransactionContext;

pub trait PaymentService: Send + Sync {
    /// Records a payment against a specific sale or purchase transaction.
    fn record_payment(
        &self,
        uow: &mut dyn TransactionContext,
        payload: RecordPaymentPayload,
    ) -> Result<i64, AppError>;

    /// Records a cash refund ledger row (REFUND_OUT / REFUND_IN, positive amount).
    fn record_refund(
        &self,
        uow: &mut dyn TransactionContext,
        payload: RecordRefundPayload,
    ) -> Result<i64, AppError>;

    /// Reduces balance_due on a transaction without creating a PaymentSettlement.
    fn reduce_transaction_balance(
        &self,
        uow: &mut dyn TransactionContext,
        transaction_type: TransactionType,
        transaction_id: i64,
        reduction_amount: f64,
    ) -> Result<(), AppError>;
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

    fn reduce_transaction_balance(
        &self,
        uow: &mut dyn TransactionContext,
        transaction_type: TransactionType,
        transaction_id: i64,
        reduction_amount: f64,
    ) -> Result<(), AppError> {
        (**self).reduce_transaction_balance(uow, transaction_type, transaction_id, reduction_amount)
    }
}
