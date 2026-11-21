use crate::core::dtos::{RecordPaymentPayload, RecordRefundPayload};
use crate::core::entities::PaymentSettlement;
use crate::core::error::AppError;
use crate::core::value_objects::PaymentStatus;
use crate::data::contracts::unit_of_work::TransactionContext;
use crate::services::contracts::payment_service::PaymentService;

#[derive(Clone, Copy, Debug, Default)]
pub struct PaymentServiceImpl;

impl PaymentServiceImpl {
    pub fn new() -> Self {
        Self
    }
}

impl PaymentService for PaymentServiceImpl {
    fn record_payment(
        &self,
        uow: &mut dyn TransactionContext,
        payload: RecordPaymentPayload,
    ) -> Result<i64, AppError> {
        if payload.amount_paid <= 0.0 {
            return Err(AppError::ValidationError {
                field: "amount_paid".to_string(),
                message: format!("Payment amount must be positive, got {}", payload.amount_paid),
            });
        }

        let (current_balance, _) = if payload.transaction_type == "SALE" {
            let sale = uow
                .sales()
                .find_by_id(payload.transaction_id)?
                .ok_or_else(|| AppError::NotFound {
                    entity: "SaleTransaction".to_string(),
                    id: payload.transaction_id.to_string(),
                })?;
            (sale.balance_due, sale.payment_status)
        } else {
            let pur = uow
                .purchases()
                .find_by_id(payload.transaction_id)?
                .ok_or_else(|| AppError::NotFound {
                    entity: "PurchaseTransaction".to_string(),
                    id: payload.transaction_id.to_string(),
                })?;
            (pur.balance_due, pur.payment_status)
        };

        if payload.amount_paid > current_balance {
            return Err(AppError::ValidationError {
                field: "amount_paid".to_string(),
                message: "Payment exceeds outstanding balance".to_string(),
            });
        }

        let new_balance = current_balance - payload.amount_paid;
        let new_status = if new_balance <= 0.0 {
            PaymentStatus::Paid
        } else {
            PaymentStatus::Partial
        };

        if payload.transaction_type == "SALE" {
            uow.sales()
                .update_balance_due(payload.transaction_id, new_balance, new_status)?;
        } else {
            uow.purchases()
                .update_balance_due(payload.transaction_id, new_balance, new_status)?;
        }

        let settlement = PaymentSettlement {
            id: 0,
            settlement_date: payload.settlement_date,
            party_id: payload.party_id,
            transaction_type: payload.transaction_type,
            transaction_id: payload.transaction_id,
            amount_paid: payload.amount_paid,
            payment_mode: payload.payment_mode,
            notes: payload.notes,
            created_at: chrono::Utc::now().to_rfc3339(),
        };
        uow.settlements().create(&settlement)
    }

    fn record_refund(
        &self,
        uow: &mut dyn TransactionContext,
        payload: RecordRefundPayload,
    ) -> Result<i64, AppError> {
        let settlement = PaymentSettlement {
            id: 0,
            settlement_date: payload.refund_date,
            party_id: payload.party_id,
            transaction_type: format!("{}_REFUND", payload.transaction_type),
            transaction_id: payload.transaction_id.unwrap_or(0),
            amount_paid: -payload.amount.abs(),
            payment_mode: payload.payment_mode,
            notes: payload.notes,
            created_at: chrono::Utc::now().to_rfc3339(),
        };
        uow.settlements().create(&settlement)
    }

    fn reduce_transaction_balance(
        &self,
        uow: &mut dyn TransactionContext,
        transaction_type: &str,
        transaction_id: i64,
        reduction_amount: f64,
    ) -> Result<(), AppError> {
        if transaction_type == "SALE" {
            let sale = uow
                .sales()
                .find_by_id(transaction_id)?
                .ok_or_else(|| AppError::NotFound {
                    entity: "SaleTransaction".to_string(),
                    id: transaction_id.to_string(),
                })?;
            let new_balance = (sale.balance_due - reduction_amount).max(0.0);
            let new_status = if new_balance == 0.0 {
                PaymentStatus::Paid
            } else {
                PaymentStatus::Partial
            };
            uow.sales()
                .update_balance_due(transaction_id, new_balance, new_status)?;
        } else {
            let pur = uow
                .purchases()
                .find_by_id(transaction_id)?
                .ok_or_else(|| AppError::NotFound {
                    entity: "PurchaseTransaction".to_string(),
                    id: transaction_id.to_string(),
                })?;
            let new_balance = (pur.balance_due - reduction_amount).max(0.0);
            let new_status = if new_balance == 0.0 {
                PaymentStatus::Paid
            } else {
                PaymentStatus::Partial
            };
            uow.purchases()
                .update_balance_due(transaction_id, new_balance, new_status)?;
        }
        Ok(())
    }
}
