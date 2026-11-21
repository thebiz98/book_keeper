// tests/payment_service_test.rs
//! Test Suite: Payment Service & Balance Decrement
//! Test Cases: TC-PAY-001 through TC-PAY-004
//! Derived from book_keeper_LLD.md § 11.3 and Spec Miner Report

mod common;

use common::fixtures::*;
use common::TestDb;
use book_keeper::core::dtos::*;
use book_keeper::core::entities::*;
use book_keeper::core::error::AppError;
use book_keeper::core::value_objects::*;
use book_keeper::data::contracts::unit_of_work::{TransactionContext, UnitOfWork};
use book_keeper::services::contracts::payment_service::PaymentService;

/// Test implementation of PaymentService matching LLD Section 6.4
pub struct PaymentServiceImpl;

impl PaymentService for PaymentServiceImpl {
    fn record_payment(
        &self,
        uow: &mut dyn TransactionContext,
        payload: RecordPaymentPayload,
    ) -> Result<i64, AppError> {
        let (current_balance, _current_status) = if payload.transaction_type == TransactionType::Sale {
            let sale = uow.sales().find_by_id(payload.transaction_id)?
                .ok_or_else(|| AppError::NotFound { entity: "SaleTransaction".to_string(), id: payload.transaction_id.to_string() })?;
            (sale.balance_due, sale.payment_status)
        } else {
            let pur = uow.purchases().find_by_id(payload.transaction_id)?
                .ok_or_else(|| AppError::NotFound { entity: "PurchaseTransaction".to_string(), id: payload.transaction_id.to_string() })?;
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

        if payload.transaction_type == TransactionType::Sale {
            uow.sales().update_balance_due(payload.transaction_id, new_balance, new_status)?;
        } else {
            uow.purchases().update_balance_due(payload.transaction_id, new_balance, new_status)?;
        }

        let settlement = PaymentSettlement {
            id: 0,
            settlement_date: payload.settlement_date,
            party_id: payload.party_id,
            kind: payload.kind,
            transaction_type: payload.transaction_type,
            transaction_id: Some(payload.transaction_id),
            return_id: None,
            amount_paid: payload.amount_paid,
            payment_mode: payload.payment_mode,
            notes: payload.notes,
            created_at: "2026-11-20T12:00:00Z".to_string(),
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
            kind: match payload.transaction_type {
                TransactionType::Sale => PaymentKind::RefundOut,
                TransactionType::Purchase => PaymentKind::RefundIn,
            },
            transaction_type: payload.transaction_type,
            transaction_id: payload.transaction_id,
            return_id: payload.return_id,
            amount_paid: payload.amount.abs(), // Ledger amounts are always positive (DD-01)
            payment_mode: payload.payment_mode,
            notes: payload.notes,
            created_at: "2026-11-20T12:00:00Z".to_string(),
        };

        uow.settlements().create(&settlement)
    }

    fn reduce_transaction_balance(
        &self,
        uow: &mut dyn TransactionContext,
        transaction_type: TransactionType,
        transaction_id: i64,
        reduction_amount: f64,
    ) -> Result<(), AppError> {
        if transaction_type == TransactionType::Sale {
            let sale = uow.sales().find_by_id(transaction_id)?
                .ok_or_else(|| AppError::NotFound { entity: "SaleTransaction".to_string(), id: transaction_id.to_string() })?;
            let new_balance = (sale.balance_due - reduction_amount).max(0.0);
            let new_status = if new_balance == 0.0 { PaymentStatus::Paid } else { PaymentStatus::Partial };
            uow.sales().update_balance_due(transaction_id, new_balance, new_status)?;
        } else {
            let pur = uow.purchases().find_by_id(transaction_id)?
                .ok_or_else(|| AppError::NotFound { entity: "PurchaseTransaction".to_string(), id: transaction_id.to_string() })?;
            let new_balance = (pur.balance_due - reduction_amount).max(0.0);
            let new_status = if new_balance == 0.0 { PaymentStatus::Paid } else { PaymentStatus::Partial };
            uow.purchases().update_balance_due(transaction_id, new_balance, new_status)?;
        }
        Ok(())
    }
}

#[test]
fn test_tc_pay_001_record_full_payment_clears_balance() {
    let uow = TestDb::new();
    let mut tx = uow.begin_transaction().expect("begin tx");

    // Setup: Sale with balance_due = 250.0, Partial
    let sale = create_test_sale_tx(1, "SAL-1001", None, 500.0, 250.0, "2026-11-20");
    tx.sales().create_transaction(&sale).expect("create sale");

    // Action: Pay 250.0
    let payment_service = PaymentServiceImpl;
    let payload = RecordPaymentPayload {
        party_id: None,
        transaction_type: TransactionType::Sale,
        kind: PaymentKind::Settlement,
        transaction_id: 1,
        settlement_date: "2026-11-20".to_string(),
        amount_paid: 250.0,
        payment_mode: Some("CASH".to_string()),
        notes: None,
    };
    let settlement_id = payment_service.record_payment(&mut *tx, payload).expect("record payment");

    assert!(settlement_id > 0);

    // Assert: balance_due becomes 0.0, status becomes Paid
    let updated_sale = tx.sales().find_by_id(1).expect("find sale").expect("sale exists");
    assert_eq!(updated_sale.balance_due, 0.0);
    assert_eq!(updated_sale.payment_status, PaymentStatus::Paid);

    // PaymentSettlement row exists
    let settlements = tx.settlements().list_by_transaction(TransactionType::Sale, 1).expect("list settlements");
    assert_eq!(settlements.len(), 1);
    assert_eq!(settlements[0].amount_paid, 250.0);
}

#[test]
fn test_tc_pay_002_record_partial_payment_updates_status() {
    let uow = TestDb::new();
    let mut tx = uow.begin_transaction().expect("begin tx");

    // Setup: Bill with balance_due = 500.0
    let sale = create_test_sale_tx(2, "SAL-1002", None, 500.0, 0.0, "2026-11-20");
    tx.sales().create_transaction(&sale).expect("create sale");

    // Action: Pay 100.0
    let payment_service = PaymentServiceImpl;
    let payload = RecordPaymentPayload {
        party_id: None,
        transaction_type: TransactionType::Sale,
        kind: PaymentKind::Settlement,
        transaction_id: 2,
        settlement_date: "2026-11-20".to_string(),
        amount_paid: 100.0,
        payment_mode: Some("CASH".to_string()),
        notes: None,
    };
    let settlement_id = payment_service.record_payment(&mut *tx, payload).expect("record partial payment");
    assert!(settlement_id > 0);

    // Assert: balance_due becomes 400.0, payment_status is Partial
    let updated_sale = tx.sales().find_by_id(2).expect("find sale").expect("sale exists");
    assert_eq!(updated_sale.balance_due, 400.0);
    assert_eq!(updated_sale.payment_status, PaymentStatus::Partial);
}

#[test]
fn test_tc_pay_003_overpayment_rejected() {
    let uow = TestDb::new();
    let mut tx = uow.begin_transaction().expect("begin tx");

    // Setup: Bill with balance_due = 500.0
    let sale = create_test_sale_tx(3, "SAL-1003", None, 500.0, 0.0, "2026-11-20");
    tx.sales().create_transaction(&sale).expect("create sale");

    // Action: Attempt to pay 600.0
    let payment_service = PaymentServiceImpl;
    let payload = RecordPaymentPayload {
        party_id: None,
        transaction_type: TransactionType::Sale,
        kind: PaymentKind::Settlement,
        transaction_id: 3,
        settlement_date: "2026-11-20".to_string(),
        amount_paid: 600.0,
        payment_mode: Some("CASH".to_string()),
        notes: None,
    };
    let err = payment_service.record_payment(&mut *tx, payload).unwrap_err();

    // Assert: ValidationError("Payment exceeds outstanding balance")
    match err {
        AppError::ValidationError { message, .. } => {
            assert!(message.contains("Payment exceeds outstanding balance"), "Error message must indicate overpayment: {}", message);
        }
        other => panic!("Expected ValidationError, got {:?}", other),
    }

    // Zero mutations: balance remains 500.0
    let unmodified_sale = tx.sales().find_by_id(3).expect("find sale").expect("sale exists");
    assert_eq!(unmodified_sale.balance_due, 500.0);
    let settlements = tx.settlements().list_by_transaction(TransactionType::Sale, 3).expect("list settlements");
    assert!(settlements.is_empty(), "No settlements should be logged");
}

#[test]
fn test_tc_pay_004_record_refund_creates_outflow_settlement() {
    let uow = TestDb::new();
    let mut tx = uow.begin_transaction().expect("begin tx");

    // Action: Record 50.0 cash refund
    let payment_service = PaymentServiceImpl;
    let payload = RecordRefundPayload {
        party_id: None,
        transaction_type: TransactionType::Sale,
        transaction_id: Some(1),
        return_id: None,
        refund_date: "2026-11-20".to_string(),
        amount: 50.0,
        payment_mode: Some("CASH".to_string()),
        notes: Some("Customer return refund".to_string()),
    };
    let settlement_id = payment_service.record_refund(&mut *tx, payload).expect("record refund");
    assert!(settlement_id > 0);

    // Assert: REFUND_OUT row with a positive amount (DD-01: ledger amounts are always > 0)
    let settlements = tx.settlements().list_by_transaction(TransactionType::Sale, 1).expect("list settlements");
    assert_eq!(settlements.len(), 1);
    assert_eq!(settlements[0].kind, PaymentKind::RefundOut);
    assert_eq!(settlements[0].amount_paid, 50.0, "Refund rows carry a positive amount; direction comes from kind");
}
