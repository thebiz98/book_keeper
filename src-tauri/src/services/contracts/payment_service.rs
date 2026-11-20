use crate::core::dtos::{RecordPaymentPayload, RecordRefundPayload};
use crate::core::error::AppError;
use crate::data::contracts::unit_of_work::TransactionContext;

pub trait PaymentService: Send + Sync {
    /// Records a payment against a specific sale or purchase transaction.
    fn record_payment(
        &self,
        uow: &mut dyn TransactionContext,
        payload: RecordPaymentPayload,
    ) -> Result<i64, AppError>;

    /// Records a cash refund (negative payment / outflow from drawer).
    fn record_refund(
        &self,
        uow: &mut dyn TransactionContext,
        payload: RecordRefundPayload,
    ) -> Result<i64, AppError>;

    /// Reduces balance_due on a transaction without creating a PaymentSettlement.
    fn reduce_transaction_balance(
        &self,
        uow: &mut dyn TransactionContext,
        transaction_type: &str,
        transaction_id: i64,
        reduction_amount: f64,
    ) -> Result<(), AppError>;
}
