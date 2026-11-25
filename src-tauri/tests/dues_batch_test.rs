// tests/dues_batch_test.rs
//! Test Suite: v1.2 B3 batch settlement (D9) and settlement correction (D8).
//! Runs the production services on real SQLite.

mod common;

use common::fixtures::*;
use common::invariants::assert_ledger_invariant;
use common::ledger::*;
use common::*;
use book_keeper::core::dtos::*;
use book_keeper::core::error::AppError;
use book_keeper::core::value_objects::*;
use book_keeper::data::contracts::unit_of_work::UnitOfWork;
use book_keeper::services::contracts::accounting_service::AccountingService;

fn setup() -> (AccountingServiceImpl<PaymentServiceImpl>, TestDb) {
    let db = TestDb::new();
    (AccountingServiceImpl::new(PaymentServiceImpl, db.clone()), db)
}

fn sale_header(db: &TestDb, id: i64) -> (f64, f64, PaymentStatus) {
    let mut tx = db.begin_read().unwrap();
    let s = tx.sales().find_transaction_by_id(id).unwrap().unwrap();
    (s.paid_amount, s.balance_due, s.payment_status)
}

fn batch(party: i64, t: TransactionType, allocs: &[(i64, f64)]) -> SettleBatchPayload {
    SettleBatchPayload {
        party_id: party,
        transaction_type: t,
        settlement_date: DAY.to_string(),
        payment_mode: Some("CASH".to_string()),
        notes: None,
        allocations: allocs
            .iter()
            .map(|(id, a)| SettleAllocation { transaction_id: *id, amount_paid: *a })
            .collect(),
    }
}

fn field_of<T: std::fmt::Debug>(r: Result<T, AppError>) -> String {
    match r {
        Err(AppError::ValidationError { field, .. }) => field,
        other => panic!("expected ValidationError, got {other:?}"),
    }
}

/// Customer 1 with sales 1 (due 300), 2 (due 400, 100 upfront on 500) and 3 (due 200).
fn seed_three(db: &TestDb) {
    seed_party(db, 1, "Cust", PartyType::Customer);
    let mut tx = db.begin_transaction().unwrap();
    seed_sale_bill(&mut *tx, 1, Some(1), 300.0, 0.0, DAY);
    seed_sale_bill(&mut *tx, 2, Some(1), 500.0, 100.0, DAY);
    seed_sale_bill(&mut *tx, 3, Some(1), 200.0, 0.0, DAY);
    tx.commit().unwrap();
}

#[test]
fn batch_settles_several_bills_and_moves_cash() {
    let (acc, db) = setup();
    seed_three(&db);
    let before = acc.calculate_cash_in_hand(DAY).unwrap();
    let res = acc
        .settle_bills_batch(batch(1, TransactionType::Sale, &[(1, 300.0), (2, 150.0)]))
        .unwrap();
    assert_eq!(res.bills_touched, vec![1, 2]);
    assert_eq!(res.settlement_ids.len(), 2);
    assert_eq!(sale_header(&db, 1), (300.0, 0.0, PaymentStatus::Paid));
    assert_eq!(sale_header(&db, 2), (250.0, 250.0, PaymentStatus::Partial));
    assert_eq!(sale_header(&db, 3).1, 200.0);
    assert_eq!(acc.calculate_cash_in_hand(DAY).unwrap(), before + 450.0);
    assert_ledger_invariant(&db);
}

#[test]
fn batch_is_atomic_when_one_row_is_bad() {
    let (acc, db) = setup();
    seed_three(&db);
    let rows = ledger_row_count(&db);
    // Third row exceeds its balance: nothing at all may be written.
    let r = acc.settle_bills_batch(batch(1, TransactionType::Sale, &[(1, 100.0), (2, 100.0), (3, 999.0)]));
    assert_eq!(field_of(r), "allocations[2].amount_paid");
    assert_eq!(ledger_row_count(&db), rows);
    assert_eq!(sale_header(&db, 1), (0.0, 300.0, PaymentStatus::Unpaid));
    assert_ledger_invariant(&db);
}

#[test]
fn batch_rejects_bad_shapes() {
    let (acc, db) = setup();
    seed_three(&db);
    seed_party(&db, 2, "Other", PartyType::Customer);
    seed_party(&db, 9, "Vendor", PartyType::Vendor);
    let mut tx = db.begin_transaction().unwrap();
    seed_purchase_bill(&mut *tx, 1, Some(9), 100.0, 0.0, DAY);
    // A non-PURCHASE-source purchase (opening stock) must never be settleable.
    let mut opening = create_test_purchase_tx(2, "OPN-1", Some(9), 100.0, 0.0, DAY);
    opening.source = LotSource::Opening;
    tx.purchases().create_transaction(&opening).unwrap();
    tx.commit().unwrap();
    let rows = ledger_row_count(&db);

    assert_eq!(field_of(acc.settle_bills_batch(batch(1, TransactionType::Sale, &[]))), "allocations");
    let many: Vec<(i64, f64)> = (0..201).map(|i| (i + 1, 1.0)).collect();
    assert_eq!(field_of(acc.settle_bills_batch(batch(1, TransactionType::Sale, &many))), "allocations");
    assert_eq!(
        field_of(acc.settle_bills_batch(batch(1, TransactionType::Sale, &[(1, 10.0), (1, 10.0)]))),
        "allocations[1].transaction_id"
    );
    assert_eq!(
        field_of(acc.settle_bills_batch(batch(1, TransactionType::Sale, &[(1, 0.0)]))),
        "allocations[0].amount_paid"
    );
    assert_eq!(
        field_of(acc.settle_bills_batch(batch(1, TransactionType::Sale, &[(1, f64::NAN)]))),
        "allocations[0].amount_paid"
    );
    // Wrong party owns the bill.
    assert_eq!(
        field_of(acc.settle_bills_batch(batch(2, TransactionType::Sale, &[(1, 10.0)]))),
        "allocations[0].transaction_id"
    );
    // Party type mismatch.
    assert_eq!(field_of(acc.settle_bills_batch(batch(9, TransactionType::Sale, &[(1, 10.0)]))), "transaction_type");
    // Bad date.
    let mut bad_date = batch(1, TransactionType::Sale, &[(1, 10.0)]);
    bad_date.settlement_date = "not-a-date".to_string();
    assert!(matches!(acc.settle_bills_batch(bad_date), Err(AppError::ValidationError { .. })));
    // Unknown party / bill.
    assert!(matches!(
        acc.settle_bills_batch(batch(77, TransactionType::Sale, &[(1, 10.0)])),
        Err(AppError::NotFound { .. })
    ));
    assert!(matches!(
        acc.settle_bills_batch(batch(1, TransactionType::Sale, &[(99, 10.0)])),
        Err(AppError::NotFound { .. })
    ));
    // Non-PURCHASE source purchase is invisible; the real purchase bill still works.
    assert!(matches!(
        acc.settle_bills_batch(batch(9, TransactionType::Purchase, &[(2, 10.0)])),
        Err(AppError::NotFound { .. })
    ));
    assert_eq!(ledger_row_count(&db), rows);
    acc.settle_bills_batch(batch(9, TransactionType::Purchase, &[(1, 40.0)])).unwrap();
    assert_ledger_invariant(&db);
}

#[test]
fn fifo_wrapper_matches_the_batch_path() {
    let (acc, db) = setup();
    seed_three(&db);
    // FIFO: oldest-first by (date, id): bill 1 (300), then bill 2 (200 of 400).
    let res = acc.settle_bill_payment(settle_payload(1, TransactionType::Sale, None, 500.0)).unwrap();
    assert_eq!(res.bills_touched, vec![1, 2]);
    assert_eq!(sale_header(&db, 1), (300.0, 0.0, PaymentStatus::Paid));
    assert_eq!(sale_header(&db, 2), (300.0, 200.0, PaymentStatus::Partial));
    assert_eq!(sale_header(&db, 3).2, PaymentStatus::Unpaid);
    assert_ledger_invariant(&db);
}

fn first_settlement(db: &TestDb, bill: i64) -> i64 {
    let mut tx = db.begin_read().unwrap();
    tx.settlements()
        .list_by_transaction(TransactionType::Sale, bill)
        .unwrap()
        .into_iter()
        .find(|r| r.kind == PaymentKind::Settlement)
        .unwrap()
        .id
}

fn upd(amount: f64) -> UpdateSettlementPayload {
    UpdateSettlementPayload {
        amount_paid: amount,
        settlement_date: DAY.to_string(),
        payment_mode: Some("UPI".to_string()),
        notes: Some("fixed".to_string()),
    }
}

#[test]
fn update_settlement_within_the_cap() {
    let (acc, db) = setup();
    seed_three(&db);
    acc.settle_bills_batch(batch(1, TransactionType::Sale, &[(1, 100.0)])).unwrap();
    let id = first_settlement(&db, 1);
    let drawer = acc.calculate_cash_in_hand(DAY).unwrap();

    // Up to old + balance (= 300), down to 50.
    let row = acc.update_settlement(id, upd(300.0)).unwrap();
    assert_eq!(row.amount_paid, 300.0);
    assert_eq!(row.payment_mode.as_deref(), Some("UPI"));
    assert_eq!(sale_header(&db, 1), (300.0, 0.0, PaymentStatus::Paid));
    assert_eq!(acc.calculate_cash_in_hand(DAY).unwrap(), drawer + 200.0);
    acc.update_settlement(id, upd(50.0)).unwrap();
    assert_eq!(sale_header(&db, 1), (50.0, 250.0, PaymentStatus::Partial));
    assert_eq!(acc.calculate_cash_in_hand(DAY).unwrap(), drawer - 50.0);

    assert_eq!(field_of(acc.update_settlement(id, upd(300.01))), "amount_paid");
    assert_eq!(field_of(acc.update_settlement(id, upd(0.0))), "amount_paid");
    let mut bad = upd(10.0);
    bad.settlement_date = "2026-13-40".to_string();
    assert!(matches!(acc.update_settlement(id, bad), Err(AppError::ValidationError { .. })));
    assert!(matches!(acc.update_settlement(9999, upd(10.0)), Err(AppError::NotFound { .. })));
    assert_eq!(sale_header(&db, 1), (50.0, 250.0, PaymentStatus::Partial));
    assert_ledger_invariant(&db);
}

#[test]
fn update_and_delete_only_touch_settlement_rows() {
    let (acc, db) = setup();
    seed_three(&db);
    // Bill 2 carries an UPFRONT row of 100.
    let mut tx = db.begin_read().unwrap();
    let upfront = tx
        .settlements()
        .list_by_transaction(TransactionType::Sale, 2)
        .unwrap()
        .into_iter()
        .find(|r| r.kind == PaymentKind::Upfront)
        .unwrap()
        .id;
    drop(tx);
    assert_eq!(field_of(acc.update_settlement(upfront, upd(10.0))), "kind");
    assert_eq!(field_of(acc.delete_settlement(upfront)), "kind");
    assert_eq!(sale_header(&db, 2), (100.0, 400.0, PaymentStatus::Partial));
    assert_ledger_invariant(&db);
}

#[test]
fn delete_settlement_restores_balance_and_status() {
    let (acc, db) = setup();
    seed_three(&db);
    acc.settle_bills_batch(batch(1, TransactionType::Sale, &[(1, 300.0), (3, 50.0)])).unwrap();
    assert_eq!(sale_header(&db, 1).2, PaymentStatus::Paid);
    let drawer = acc.calculate_cash_in_hand(DAY).unwrap();
    let id = first_settlement(&db, 1);
    let out = acc.delete_settlement(id).unwrap();
    assert_eq!(out.removed_amount, 300.0);
    assert_eq!(out.transaction_id, 1);
    assert_eq!(sale_header(&db, 1), (0.0, 300.0, PaymentStatus::Unpaid));
    assert_eq!(acc.calculate_cash_in_hand(DAY).unwrap(), drawer - 300.0);
    assert!(matches!(acc.delete_settlement(id), Err(AppError::NotFound { .. })));
    assert_ledger_invariant(&db);
}
