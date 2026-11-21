// tests/accounting_service_test.rs
//! Test Suite: Accounting & Dues Settlement (Khata), return credit, party ledger, cash drawer
//! Test Cases: TC-DUES-001 through TC-DUES-007
//! Derived from book_keeper_LLD.md § 11.8. Runs the production services on real SQLite.

mod common;

use common::fixtures::*;
use common::ledger::*;
use common::*;
use book_keeper::core::dtos::*;
use book_keeper::core::error::AppError;
use book_keeper::core::value_objects::*;
use book_keeper::data::contracts::unit_of_work::UnitOfWork;
use book_keeper::services::contracts::accounting_service::AccountingService;
use book_keeper::services::contracts::payment_service::PaymentService;

fn setup() -> (AccountingServiceImpl<PaymentServiceImpl>, TestDb) {
    let db = TestDb::new();
    (AccountingServiceImpl::new(PaymentServiceImpl, db.clone()), db)
}

fn sale_header(db: &TestDb, id: i64) -> (f64, f64, PaymentStatus) {
    let mut tx = db.begin_read().unwrap();
    let s = tx.sales().find_transaction_by_id(id).unwrap().unwrap();
    (s.paid_amount, s.balance_due, s.payment_status)
}

fn pur_header(db: &TestDb, id: i64) -> (f64, f64, PaymentStatus) {
    let mut tx = db.begin_read().unwrap();
    let s = tx.purchases().find_transaction_by_id(id).unwrap().unwrap();
    (s.paid_amount, s.balance_due, s.payment_status)
}

fn expect_validation(r: Result<SettlementResult, AppError>) {
    assert!(matches!(r, Err(AppError::ValidationError { .. })), "expected ValidationError, got {r:?}");
}

#[test]
fn test_tc_dues_001_full_due_settlement() {
    let (acc, db) = setup();
    seed_party(&db, 1, "Ramesh General Store", PartyType::Customer);
    let mut tx = db.begin_transaction().unwrap();
    seed_sale_bill(&mut *tx, 1, Some(1), 500.0, 250.0, DAY);
    tx.commit().unwrap();
    assert_eq!(sale_header(&db, 1).2, PaymentStatus::Partial);

    let res = acc.settle_bill_payment(settle_payload(1, TransactionType::Sale, Some(1), 250.0)).unwrap();
    assert_eq!(res.bills_touched, vec![1]);
    assert_eq!(res.settlement_ids.len(), 1);
    assert_eq!(sale_header(&db, 1), (500.0, 0.0, PaymentStatus::Paid));
    let mut r = db.begin_read().unwrap();
    let rows = r.settlements().list_by_transaction(TransactionType::Sale, 1).unwrap();
    assert_eq!(rows.iter().filter(|x| x.kind == PaymentKind::Settlement).count(), 1);
}

#[test]
fn test_tc_dues_002_incremental_partial_settlement() {
    let (acc, db) = setup();
    seed_party(&db, 2, "Anand Stationery", PartyType::Customer);
    let mut tx = db.begin_transaction().unwrap();
    seed_sale_bill(&mut *tx, 2, Some(2), 500.0, 0.0, DAY);
    tx.commit().unwrap();

    acc.settle_bill_payment(settle_payload(2, TransactionType::Sale, Some(2), 200.0)).unwrap();
    assert_eq!(sale_header(&db, 2), (200.0, 300.0, PaymentStatus::Partial));
}

#[test]
fn test_tc_dues_003_daily_cash_drawer_reconciliation() {
    let (acc, db) = setup();
    seed_party(&db, 3, "Vikas Book Depot", PartyType::Customer);
    let mut tx = db.begin_transaction().unwrap();
    seed_sale_bill(&mut *tx, 10, None, 1000.0, 1000.0, DAY); // cash sale today
    seed_sale_bill(&mut *tx, 11, Some(3), 400.0, 0.0, "2026-11-19"); // older credit bill
    tx.commit().unwrap();

    acc.settle_bill_payment(settle_payload(3, TransactionType::Sale, Some(11), 400.0)).unwrap();
    assert_eq!(acc.calculate_live_cash_drawer(DAY).unwrap(), 1400.0, "upfront 1000 + settled 400, each once");

    let s = acc.ledger_cash_summary(DAY, DAY).unwrap();
    assert_eq!((s.sale_upfront, s.sale_settlement, s.cash_in, s.cash_out, s.net), (1000.0, 400.0, 1400.0, 0.0, 1400.0));
    // The 19th has nothing in the ledger (the bill itself moved no cash).
    assert_eq!(acc.calculate_live_cash_drawer("2026-11-19").unwrap(), 0.0);
}

#[test]
fn test_cash_drawer_uses_full_direction_table() {
    let (acc, db) = setup();
    seed_party(&db, 1, "Vendor", PartyType::Vendor);
    let mut tx = db.begin_transaction().unwrap();
    seed_sale_bill(&mut *tx, 1, None, 500.0, 500.0, DAY); // in 500
    seed_purchase_bill(&mut *tx, 1, Some(1), 300.0, 120.0, DAY); // out 120
    pay(&mut *tx, TransactionType::Purchase, 1, Some(1), PaymentKind::Settlement, 80.0, DAY); // out 80
    seed_return(&mut *tx, 1, ReturnType::Customer, None, Some(1), 60.0, DAY);
    seed_return(&mut *tx, 2, ReturnType::Vendor, Some(1), Some(1), 25.0, DAY);
    let refund = |tx_type, amount, ret| RecordRefundPayload {
        party_id: None,
        transaction_type: tx_type,
        transaction_id: None,
        return_id: Some(ret),
        refund_date: DAY.to_string(),
        amount,
        payment_mode: None,
        notes: None,
    };
    PaymentServiceImpl.record_refund(&mut *tx, refund(TransactionType::Sale, 60.0, 1)).unwrap(); // out 60
    PaymentServiceImpl.record_refund(&mut *tx, refund(TransactionType::Purchase, 25.0, 2)).unwrap(); // in 25
    tx.commit().unwrap();

    let s = acc.ledger_cash_summary(DAY, DAY).unwrap();
    assert_eq!(s.cash_in, 525.0);
    assert_eq!(s.cash_out, 260.0);
    assert_eq!(s.net, 265.0);
    assert_eq!(acc.calculate_live_cash_drawer(DAY).unwrap(), 265.0);
}

#[test]
fn test_tc_dues_004_fifo_settlement_across_bills() {
    let (acc, db) = setup();
    seed_party(&db, 1, "Cust", PartyType::Customer);
    let mut tx = db.begin_transaction().unwrap();
    seed_sale_bill(&mut *tx, 2, Some(1), 400.0, 0.0, "2026-11-12"); // newer, lower id order irrelevant
    seed_sale_bill(&mut *tx, 1, Some(1), 300.0, 0.0, "2026-11-10"); // oldest by date
    tx.commit().unwrap();

    let res = acc.settle_bill_payment(settle_payload(1, TransactionType::Sale, None, 500.0)).unwrap();
    assert_eq!(res.bills_touched, vec![1, 2], "oldest transaction_date first");
    assert_eq!(res.settlement_ids.len(), 2);
    assert_eq!(sale_header(&db, 1), (300.0, 0.0, PaymentStatus::Paid));
    assert_eq!(sale_header(&db, 2), (200.0, 200.0, PaymentStatus::Partial));
}

#[test]
fn test_tc_dues_005_fifo_over_settlement_rejected_up_front() {
    let (acc, db) = setup();
    seed_party(&db, 1, "Cust", PartyType::Customer);
    let mut tx = db.begin_transaction().unwrap();
    seed_sale_bill(&mut *tx, 1, Some(1), 300.0, 0.0, "2026-11-10");
    seed_sale_bill(&mut *tx, 2, Some(1), 400.0, 0.0, "2026-11-12");
    tx.commit().unwrap();

    expect_validation(acc.settle_bill_payment(settle_payload(1, TransactionType::Sale, None, 800.0)));
    assert_eq!(ledger_row_count(&db), 0);
    assert_eq!(sale_header(&db, 1).1, 300.0);
    assert_eq!(sale_header(&db, 2).1, 400.0);
}

#[test]
fn test_tc_dues_006_specific_bill_over_settlement_rejected() {
    let (acc, db) = setup();
    seed_party(&db, 1, "Cust", PartyType::Customer);
    let mut tx = db.begin_transaction().unwrap();
    seed_sale_bill(&mut *tx, 1, Some(1), 100.0, 0.0, DAY);
    tx.commit().unwrap();

    expect_validation(acc.settle_bill_payment(settle_payload(1, TransactionType::Sale, Some(1), 150.0)));
    assert_eq!(ledger_row_count(&db), 0);
    assert_eq!(sale_header(&db, 1), (0.0, 100.0, PaymentStatus::Unpaid));
}

#[test]
fn test_tc_dues_007_party_type_mismatch() {
    let (acc, db) = setup();
    seed_party(&db, 1, "Vendor", PartyType::Vendor);
    seed_party(&db, 2, "Cust", PartyType::Customer);
    let mut tx = db.begin_transaction().unwrap();
    seed_sale_bill(&mut *tx, 1, Some(2), 100.0, 0.0, DAY);
    seed_purchase_bill(&mut *tx, 1, Some(1), 100.0, 0.0, DAY);
    tx.commit().unwrap();

    expect_validation(acc.settle_bill_payment(settle_payload(1, TransactionType::Sale, None, 10.0)));
    expect_validation(acc.settle_bill_payment(settle_payload(2, TransactionType::Purchase, None, 10.0)));
    assert_eq!(ledger_row_count(&db), 0);
}

#[test]
fn test_settle_validates_amount_date_party_and_bill_ownership() {
    let (acc, db) = setup();
    seed_party(&db, 1, "A", PartyType::Customer);
    seed_party(&db, 2, "B", PartyType::Customer);
    let mut tx = db.begin_transaction().unwrap();
    seed_sale_bill(&mut *tx, 1, Some(1), 100.0, 0.0, DAY);
    tx.commit().unwrap();

    expect_validation(acc.settle_bill_payment(settle_payload(1, TransactionType::Sale, None, 0.0)));
    expect_validation(acc.settle_bill_payment(settle_payload(1, TransactionType::Sale, None, f64::NAN)));
    let mut bad_date = settle_payload(1, TransactionType::Sale, Some(1), 10.0);
    bad_date.settlement_date = "20-11-2026".to_string();
    expect_validation(acc.settle_bill_payment(bad_date));
    // Bill 1 belongs to party 1, not 2.
    expect_validation(acc.settle_bill_payment(settle_payload(2, TransactionType::Sale, Some(1), 10.0)));
    assert!(matches!(
        acc.settle_bill_payment(settle_payload(99, TransactionType::Sale, None, 10.0)),
        Err(AppError::NotFound { .. })
    ));
    assert!(matches!(
        acc.settle_bill_payment(settle_payload(1, TransactionType::Sale, Some(99), 10.0)),
        Err(AppError::NotFound { .. })
    ));
    assert_eq!(ledger_row_count(&db), 0);
}

#[test]
fn test_vendor_side_specific_and_fifo_settlement() {
    let (acc, db) = setup();
    seed_party(&db, 1, "Vendor", PartyType::Vendor);
    seed_party(&db, 2, "Other", PartyType::Vendor);
    let mut tx = db.begin_transaction().unwrap();
    seed_purchase_bill(&mut *tx, 1, Some(1), 500.0, 100.0, "2026-11-10");
    seed_purchase_bill(&mut *tx, 2, Some(1), 300.0, 0.0, "2026-11-11");
    seed_purchase_bill(&mut *tx, 3, Some(2), 300.0, 0.0, "2026-11-11");
    // A non-PURCHASE lot header is never a payable bill.
    let mut adj = create_test_purchase_tx(4, "ADJ-1001", Some(1), 0.0, 0.0, "2026-11-01");
    adj.source = LotSource::Adjustment;
    tx.purchases().create_transaction(&adj).unwrap();
    tx.commit().unwrap();

    // A bill owned by another party is rejected; a non-PURCHASE header is not a payable bill.
    expect_validation(acc.settle_bill_payment(settle_payload(2, TransactionType::Purchase, Some(1), 10.0)));
    assert!(matches!(
        acc.settle_bill_payment(settle_payload(1, TransactionType::Purchase, Some(4), 0.01)),
        Err(AppError::NotFound { .. })
    ));
    acc.settle_bill_payment(settle_payload(1, TransactionType::Purchase, Some(2), 100.0)).unwrap();
    assert_eq!(pur_header(&db, 2), (100.0, 200.0, PaymentStatus::Partial));

    // Lump sum: outstanding is 400 (bill 1) + 200 (bill 2) = 600; 700 rejected, 450 spans both.
    expect_validation(acc.settle_bill_payment(settle_payload(1, TransactionType::Purchase, None, 700.0)));
    let res = acc.settle_bill_payment(settle_payload(1, TransactionType::Purchase, None, 450.0)).unwrap();
    assert_eq!(res.bills_touched, vec![1, 2]);
    assert_eq!(pur_header(&db, 1), (500.0, 0.0, PaymentStatus::Paid));
    assert_eq!(pur_header(&db, 2), (150.0, 150.0, PaymentStatus::Partial));
    // Settlement is cash out for purchases.
    assert_eq!(acc.calculate_live_cash_drawer(DAY).unwrap(), -550.0);
}

#[test]
fn test_apply_return_credit_linked_and_unlinked() {
    let (acc, db) = setup();
    seed_party(&db, 1, "Cust", PartyType::Customer);
    let mut tx = db.begin_transaction().unwrap();
    seed_sale_bill(&mut *tx, 1, Some(1), 100.0, 40.0, "2026-11-10"); // due 60
    seed_sale_bill(&mut *tx, 2, Some(1), 200.0, 0.0, "2026-11-11"); // due 200
    seed_return(&mut *tx, 1, ReturnType::Customer, Some(1), Some(1), 90.0, DAY);
    seed_return(&mut *tx, 2, ReturnType::Customer, Some(1), None, 500.0, DAY);
    seed_return(&mut *tx, 3, ReturnType::Customer, None, None, 50.0, DAY);

    // Linked: capped at the bill's own balance; leftover is for the caller to pay in cash.
    let c = acc
        .apply_return_credit(&mut *tx, TransactionType::Sale, Some(1), Some(1), 1, DAY, 90.0)
        .unwrap();
    assert_eq!(c, 60.0);
    let b1 = tx.sales().find_transaction_by_id(1).unwrap().unwrap();
    assert_eq!((b1.paid_amount, b1.balance_due, b1.payment_status), (40.0, 0.0, PaymentStatus::Paid));
    let b2 = tx.sales().find_transaction_by_id(2).unwrap().unwrap();
    assert_eq!(b2.balance_due, 200.0, "a linked return never touches the party's other bills");

    // Unlinked with a party: oldest open bills first; value above dues is not an error.
    let c = acc
        .apply_return_credit(&mut *tx, TransactionType::Sale, Some(1), None, 2, DAY, 500.0)
        .unwrap();
    assert_eq!(c, 200.0);
    assert_eq!(tx.sales().find_transaction_by_id(2).unwrap().unwrap().payment_status, PaymentStatus::Paid);

    // Walk-in: nothing credited.
    assert_eq!(acc.apply_return_credit(&mut *tx, TransactionType::Sale, None, None, 3, DAY, 50.0).unwrap(), 0.0);
    let credits: Vec<_> = tx
        .settlements()
        .list_all()
        .unwrap()
        .into_iter()
        .filter(|r| r.kind == PaymentKind::ReturnCredit)
        .collect();
    assert_eq!(credits.len(), 2);
    assert!(credits.iter().all(|r| r.return_id.is_some()));
}

#[test]
fn test_apply_return_credit_vendor_side_fifo() {
    let (acc, db) = setup();
    seed_party(&db, 1, "Vendor", PartyType::Vendor);
    let mut tx = db.begin_transaction().unwrap();
    seed_purchase_bill(&mut *tx, 1, Some(1), 100.0, 0.0, "2026-11-10");
    seed_purchase_bill(&mut *tx, 2, Some(1), 100.0, 0.0, "2026-11-11");
    seed_return(&mut *tx, 1, ReturnType::Vendor, Some(1), None, 150.0, DAY);
    let c = acc
        .apply_return_credit(&mut *tx, TransactionType::Purchase, Some(1), None, 1, DAY, 150.0)
        .unwrap();
    assert_eq!(c, 150.0);
    let p1 = tx.purchases().find_transaction_by_id(1).unwrap().unwrap();
    let p2 = tx.purchases().find_transaction_by_id(2).unwrap().unwrap();
    assert_eq!((p1.balance_due, p1.payment_status), (0.0, PaymentStatus::Paid));
    assert_eq!((p2.balance_due, p2.payment_status), (50.0, PaymentStatus::Partial));
    assert_eq!(p2.paid_amount, 0.0, "credit is not paid cash");
}

#[test]
fn test_party_dues_summary_both_sides() {
    let (acc, db) = setup();
    seed_party(&db, 1, "Cust", PartyType::Customer);
    seed_party(&db, 2, "Vendor", PartyType::Vendor);
    let mut tx = db.begin_transaction().unwrap();
    seed_sale_bill(&mut *tx, 1, Some(1), 100.0, 30.0, "2026-11-10");
    seed_sale_bill(&mut *tx, 2, Some(1), 50.0, 0.0, "2026-11-12");
    seed_sale_bill(&mut *tx, 3, Some(1), 20.0, 20.0, "2026-11-13"); // settled: not counted
    seed_purchase_bill(&mut *tx, 1, Some(2), 400.0, 150.0, "2026-11-05");
    tx.commit().unwrap();
    acc.settle_bill_payment(settle_payload(1, TransactionType::Sale, Some(2), 10.0)).unwrap();

    let c = acc.get_party_dues_summary(PartyType::Customer).unwrap();
    assert_eq!(c.len(), 1);
    assert_eq!((c[0].party_id, c[0].total_due, c[0].unpaid_bills_count), (1, 110.0, 2));
    assert_eq!(c[0].oldest_bill_date.as_deref(), Some("2026-11-10"));
    let v = acc.get_party_dues_summary(PartyType::Vendor).unwrap();
    assert_eq!(v.len(), 1);
    assert_eq!((v[0].party_id, v[0].total_due, v[0].unpaid_bills_count), (2, 250.0, 1));
}

#[test]
fn test_party_ledger_is_chronological_with_running_balance() {
    let (acc, db) = setup();
    seed_party(&db, 1, "Cust", PartyType::Customer);
    let mut tx = db.begin_transaction().unwrap();
    seed_sale_bill(&mut *tx, 1, Some(1), 100.0, 40.0, "2026-11-10"); // +100 -40
    seed_sale_bill(&mut *tx, 2, Some(1), 200.0, 0.0, "2026-11-12"); // +200
    seed_return(&mut *tx, 1, ReturnType::Customer, Some(1), Some(1), 25.0, "2026-11-13");
    tx.commit().unwrap();
    {
        let mut tx = db.begin_transaction().unwrap();
        // credit 25 on bill 1 (linked return) then cash refund of 5 above its balance
        acc.apply_return_credit(&mut *tx, TransactionType::Sale, Some(1), Some(1), 1, "2026-11-13", 25.0).unwrap();
        tx.commit().unwrap();
    }
    let mut dated = settle_payload(1, TransactionType::Sale, Some(2), 50.0);
    dated.settlement_date = "2026-11-14".to_string();
    acc.settle_bill_payment(dated).unwrap();

    let l = acc.get_party_ledger(1).unwrap();
    assert_eq!(l.party.id, 1);
    assert_eq!(l.bills.len(), 2);
    assert_eq!(l.payments.len(), 3); // upfront, return credit, settlement
    assert_eq!(l.returns.len(), 1);
    // 100-40=60; +200=260; return (info) 260; credit 25 -> 235; settle 50 -> 185
    let seq: Vec<(&str, &str, f64)> = l
        .entries
        .iter()
        .map(|e| (e.date.as_str(), e.transaction_type.as_str(), e.running_balance))
        .collect();
    assert_eq!(
        seq,
        vec![
            ("2026-11-10", "SALE", 100.0),
            ("2026-11-10", "UPFRONT", 60.0),
            ("2026-11-12", "SALE", 260.0),
            ("2026-11-13", "RETURN", 260.0),
            ("2026-11-13", "RETURN_CREDIT", 235.0),
            ("2026-11-14", "SETTLEMENT", 185.0),
        ]
    );
    assert_eq!(l.current_balance, 185.0);
    assert_eq!(l.entries.last().unwrap().running_balance, l.current_balance);
    assert!(matches!(acc.get_party_ledger(99), Err(AppError::NotFound { .. })));
}

#[test]
fn test_vendor_party_ledger() {
    let (acc, db) = setup();
    seed_party(&db, 1, "Vendor", PartyType::Vendor);
    let mut tx = db.begin_transaction().unwrap();
    seed_purchase_bill(&mut *tx, 1, Some(1), 500.0, 200.0, "2026-11-10");
    tx.commit().unwrap();
    let l = acc.get_party_ledger(1).unwrap();
    assert_eq!(l.bills.len(), 1);
    assert_eq!(l.bills[0].transaction_type, TransactionType::Purchase);
    assert_eq!(l.current_balance, 300.0);
    assert_eq!(l.entries.last().unwrap().running_balance, 300.0);
}
