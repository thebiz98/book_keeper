// tests/payment_service_test.rs
//! Test Suite: Payment Service (the single ledger / bill-money writer)
//! Test Cases: TC-PAY-001 through TC-PAY-007
//! Derived from book_keeper_LLD.md § 11.3. Runs the production services on real SQLite.

mod common;

use common::ledger::*;
use common::*;
use book_keeper::core::dtos::*;
use book_keeper::core::error::AppError;
use book_keeper::core::value_objects::*;
use book_keeper::data::contracts::unit_of_work::UnitOfWork;
use book_keeper::services::contracts::payment_service::PaymentService;
use book_keeper::services::contracts::sale_service::SaleService;
use book_keeper::services::r#impl::accounting_service_impl::ledger_cash_summary;
use book_keeper::services::r#impl::payment_service_impl::recompute_bill;

fn header(tx: &mut dyn book_keeper::data::contracts::unit_of_work::TransactionContext, id: i64) -> (f64, f64, PaymentStatus) {
    let s = tx.sales().find_transaction_by_id(id).unwrap().unwrap();
    (s.paid_amount, s.balance_due, s.payment_status)
}

#[test]
fn test_tc_pay_001_full_payment_clears_balance() {
    let db = TestDb::new();
    let mut tx = db.begin_transaction().unwrap();
    seed_sale_bill(&mut *tx, 1, None, 500.0, 250.0, DAY);
    assert_eq!(header(&mut *tx, 1), (250.0, 250.0, PaymentStatus::Partial));

    let id = pay(&mut *tx, TransactionType::Sale, 1, None, PaymentKind::Settlement, 250.0, DAY);
    assert!(id > 0);
    assert_eq!(header(&mut *tx, 1), (500.0, 0.0, PaymentStatus::Paid));
    let rows = tx.settlements().list_by_transaction(TransactionType::Sale, 1).unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[1].kind, PaymentKind::Settlement);
    assert_eq!(rows[1].amount_paid, 250.0);
}

#[test]
fn test_tc_pay_002_partial_payment_updates_status() {
    let db = TestDb::new();
    let mut tx = db.begin_transaction().unwrap();
    seed_sale_bill(&mut *tx, 2, None, 500.0, 0.0, DAY);
    assert_eq!(header(&mut *tx, 2), (0.0, 500.0, PaymentStatus::Unpaid));

    pay(&mut *tx, TransactionType::Sale, 2, None, PaymentKind::Settlement, 100.0, DAY);
    assert_eq!(header(&mut *tx, 2), (100.0, 400.0, PaymentStatus::Partial));
}

#[test]
fn test_tc_pay_003_overpayment_rejected_without_mutation() {
    let db = TestDb::new();
    let mut tx = db.begin_transaction().unwrap();
    seed_sale_bill(&mut *tx, 3, None, 500.0, 0.0, DAY);

    let err = PaymentServiceImpl
        .record_payment(
            &mut *tx,
            RecordPaymentPayload {
                party_id: None,
                transaction_type: TransactionType::Sale,
                transaction_id: 3,
                kind: PaymentKind::Settlement,
                return_id: None,
                settlement_date: DAY.to_string(),
                amount_paid: 600.0,
                payment_mode: None,
                notes: None,
            },
        )
        .unwrap_err();
    match err {
        AppError::ValidationError { message, .. } => {
            assert!(message.contains("Payment exceeds outstanding balance"), "{message}")
        }
        other => panic!("expected ValidationError, got {other:?}"),
    }
    assert_eq!(header(&mut *tx, 3), (0.0, 500.0, PaymentStatus::Unpaid));
    assert!(tx.settlements().list_by_transaction(TransactionType::Sale, 3).unwrap().is_empty());
}

#[test]
fn test_record_payment_rejects_zero_negative_and_refund_kinds() {
    let db = TestDb::new();
    let mut tx = db.begin_transaction().unwrap();
    seed_sale_bill(&mut *tx, 1, None, 100.0, 0.0, DAY);
    for (kind, amount) in [
        (PaymentKind::Settlement, 0.0),
        (PaymentKind::Settlement, -5.0),
        (PaymentKind::Settlement, 0.004), // rounds to 0.00
        (PaymentKind::Settlement, f64::NAN),
        (PaymentKind::RefundOut, 10.0),
        (PaymentKind::ReturnCredit, 10.0), // no return_id
    ] {
        let r = PaymentServiceImpl.record_payment(
            &mut *tx,
            RecordPaymentPayload {
                party_id: None,
                transaction_type: TransactionType::Sale,
                transaction_id: 1,
                kind,
                return_id: None,
                settlement_date: DAY.to_string(),
                amount_paid: amount,
                payment_mode: None,
                notes: None,
            },
        );
        assert!(matches!(r, Err(AppError::ValidationError { .. })), "{kind:?} {amount}");
    }
    assert!(tx.settlements().list_all().unwrap().is_empty());
}

#[test]
fn test_tc_pay_004_refund_is_ledger_row_only() {
    let db = TestDb::new();
    let mut tx = db.begin_transaction().unwrap();
    seed_sale_bill(&mut *tx, 1, None, 100.0, 100.0, DAY);
    seed_return(&mut *tx, 7, ReturnType::Customer, None, Some(1), 50.0, DAY);
    let before = header(&mut *tx, 1);

    let id = PaymentServiceImpl
        .record_refund(
            &mut *tx,
            RecordRefundPayload {
                party_id: None,
                transaction_type: TransactionType::Sale,
                transaction_id: Some(1),
                return_id: Some(7),
                refund_date: DAY.to_string(),
                amount: 50.0,
                payment_mode: Some("CASH".to_string()),
                notes: None,
            },
        )
        .unwrap();
    assert!(id > 0);
    let rows = tx.settlements().list_by_transaction(TransactionType::Sale, 1).unwrap();
    let refunds: Vec<_> = rows.iter().filter(|r| r.kind == PaymentKind::RefundOut).collect();
    assert_eq!(refunds.len(), 1);
    assert_eq!(refunds[0].amount_paid, 50.0);
    assert_eq!(refunds[0].return_id, Some(7));
    assert_eq!(header(&mut *tx, 1), before, "refund must not touch bill money fields");

    // Vendor side: RefundIn, and a non-positive refund is rejected.
    let r = PaymentServiceImpl.record_refund(
        &mut *tx,
        RecordRefundPayload {
            party_id: None,
            transaction_type: TransactionType::Purchase,
            transaction_id: None,
            return_id: Some(7),
            refund_date: DAY.to_string(),
            amount: 0.0,
            payment_mode: None,
            notes: None,
        },
    );
    assert!(matches!(r, Err(AppError::ValidationError { .. })));
}

#[test]
fn test_tc_pay_005_upfront_counted_exactly_once() {
    let db = TestDb::new();
    let inv = InventoryServiceImpl::new(FifoStrategy, db.clone());
    seed_party(&db, 1, "Cust", PartyType::Customer);
    seed_stocked_item(&db, &inv, 1, 10.0, 50.0, 100.0);
    let sales = SaleServiceImpl::new(inv, PaymentServiceImpl, db.clone());

    let r = sales
        .create_sale(sale_payload(Some(1), PaymentStatus::Partial, 40.0, &[(1, None, 1.0, 100.0)]))
        .unwrap();
    assert_eq!((r.total_amount, r.paid_amount, r.balance_due), (100.0, 40.0, 60.0));
    assert_eq!(r.payment_status, PaymentStatus::Partial);

    let mut tx = db.begin_read().unwrap();
    let h = tx.sales().find_transaction_by_id(r.sale_id).unwrap().unwrap();
    assert_eq!((h.paid_amount, h.balance_due, h.payment_status), (40.0, 60.0, PaymentStatus::Partial));
    let rows = tx.settlements().list_by_transaction(TransactionType::Sale, r.sale_id).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!((rows[0].kind, rows[0].amount_paid), (PaymentKind::Upfront, 40.0));
    let cash = ledger_cash_summary(&mut *tx, DAY, DAY).unwrap();
    assert_eq!(cash.sale_upfront, 40.0, "sales inflow is 40, not 80");
    assert_eq!(cash.cash_in, 40.0);
}

#[test]
fn test_tc_pay_006_return_credit_reduces_balance_not_paid() {
    let db = TestDb::new();
    let mut tx = db.begin_transaction().unwrap();
    seed_sale_bill(&mut *tx, 1, None, 100.0, 40.0, DAY);
    seed_return(&mut *tx, 9, ReturnType::Customer, None, Some(1), 30.0, DAY);

    PaymentServiceImpl
        .record_payment(
            &mut *tx,
            RecordPaymentPayload {
                party_id: None,
                transaction_type: TransactionType::Sale,
                transaction_id: 1,
                kind: PaymentKind::ReturnCredit,
                return_id: Some(9),
                settlement_date: DAY.to_string(),
                amount_paid: 30.0,
                payment_mode: None,
                notes: None,
            },
        )
        .unwrap();
    assert_eq!(header(&mut *tx, 1), (40.0, 30.0, PaymentStatus::Partial));
    let cash = ledger_cash_summary(&mut *tx, DAY, DAY).unwrap();
    assert_eq!(cash.cash_in, 40.0, "return credit is non-cash");
    assert_eq!(cash.net, 40.0);
}

#[test]
fn test_tc_pay_007_recompute_is_idempotent() {
    let db = TestDb::new();
    let mut tx = db.begin_transaction().unwrap();
    seed_sale_bill(&mut *tx, 1, None, 1000.0, 100.0, DAY);
    seed_return(&mut *tx, 3, ReturnType::Customer, None, Some(1), 50.0, DAY);
    pay(&mut *tx, TransactionType::Sale, 1, None, PaymentKind::Settlement, 200.0, DAY);
    PaymentServiceImpl
        .record_payment(
            &mut *tx,
            RecordPaymentPayload {
                party_id: None,
                transaction_type: TransactionType::Sale,
                transaction_id: 1,
                kind: PaymentKind::ReturnCredit,
                return_id: Some(3),
                settlement_date: DAY.to_string(),
                amount_paid: 50.0,
                payment_mode: None,
                notes: None,
            },
        )
        .unwrap();

    let first = header(&mut *tx, 1);
    assert_eq!(first, (300.0, 650.0, PaymentStatus::Partial)); // balance = total - paid - credits
    recompute_bill(&mut *tx, TransactionType::Sale, 1).unwrap();
    recompute_bill(&mut *tx, TransactionType::Sale, 1).unwrap();
    assert_eq!(header(&mut *tx, 1), first);
}

#[test]
fn test_payment_on_purchase_bill_and_missing_bill() {
    let db = TestDb::new();
    let mut tx = db.begin_transaction().unwrap();
    seed_purchase_bill(&mut *tx, 1, None, 300.0, 100.0, DAY);
    pay(&mut *tx, TransactionType::Purchase, 1, None, PaymentKind::Settlement, 200.0, DAY);
    let p = tx.purchases().find_transaction_by_id(1).unwrap().unwrap();
    assert_eq!((p.paid_amount, p.balance_due, p.payment_status), (300.0, 0.0, PaymentStatus::Paid));

    let r = PaymentServiceImpl.record_payment(
        &mut *tx,
        RecordPaymentPayload {
            party_id: None,
            transaction_type: TransactionType::Sale,
            transaction_id: 999,
            kind: PaymentKind::Settlement,
            return_id: None,
            settlement_date: DAY.to_string(),
            amount_paid: 1.0,
            payment_mode: None,
            notes: None,
        },
    );
    assert!(matches!(r, Err(AppError::NotFound { .. })));
}
