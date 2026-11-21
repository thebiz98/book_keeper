// tests/accounting_service_test.rs
//! Test Suite: Accounting & Dues Settlement (Khata) & Cash Drawer
//! Test Cases: TC-DUES-001 through TC-DUES-003
//! Derived from book_keeper_LLD.md § 11.8 and Spec Miner Report

mod common;

use common::fixtures::*;
use common::*;
use book_keeper::core::dtos::*;
use book_keeper::core::value_objects::*;
use book_keeper::data::contracts::unit_of_work::UnitOfWork;
use book_keeper::services::contracts::accounting_service::AccountingService;

fn setup_accounting_service() -> (AccountingServiceImpl<PaymentServiceImpl>, TestDb) {
    let uow = TestDb::new();
    let payment_service = PaymentServiceImpl;
    let accounting_service = AccountingServiceImpl::new(payment_service, uow.clone());
    (accounting_service, uow)
}

#[test]
fn test_tc_dues_001_full_due_settlement() {
    let (accounting_service, uow) = setup_accounting_service();
    let mut tx = uow.begin_transaction().expect("begin tx");

    let party = create_test_party(1, "Ramesh General Store", PartyType::Customer, Some("9811122233"));
    tx.parties().create(&party).expect("create party");

    let sale = create_test_sale_tx(1, "SAL-1001", Some(1), 500.0, 250.0, "2026-11-20");
    tx.sales().create_transaction(&sale).expect("create sale");
    tx.commit().expect("commit");

    let payload = SettlePaymentPayload {
        party_id: 1,
        amount: 250.0,
        settlement_date: "2026-11-20".to_string(),
        payment_mode: Some("CASH".to_string()),
        specific_bill_id: Some(1),
        notes: Some("Cleared final balance".to_string()),
    };
    accounting_service.settle_bill_payment(payload).expect("settle bill");

    let mut check_tx = uow.begin_transaction().expect("check tx");
    let updated_sale = check_tx.sales().find_by_id(1).expect("find").expect("exists");
    assert_eq!(updated_sale.balance_due, 0.0);
    assert_eq!(updated_sale.payment_status, PaymentStatus::Paid);

    let settlements = check_tx.settlements().list_by_transaction(TransactionType::Sale, 1).expect("settlements");
    assert_eq!(settlements.len(), 1);
    assert_eq!(settlements[0].amount_paid, 250.0);
}

#[test]
fn test_tc_dues_002_incremental_partial_settlement() {
    let (accounting_service, uow) = setup_accounting_service();
    let mut tx = uow.begin_transaction().expect("begin tx");

    let party = create_test_party(2, "Anand Stationery", PartyType::Customer, Some("9822233344"));
    tx.parties().create(&party).expect("create party");

    let sale = create_test_sale_tx(2, "SAL-1002", Some(2), 500.0, 0.0, "2026-11-20");
    tx.sales().create_transaction(&sale).expect("create sale");
    tx.commit().expect("commit");

    let payload = SettlePaymentPayload {
        party_id: 2,
        amount: 200.0,
        settlement_date: "2026-11-20".to_string(),
        payment_mode: Some("UPI".to_string()),
        specific_bill_id: Some(2),
        notes: Some("First installment".to_string()),
    };
    accounting_service.settle_bill_payment(payload).expect("partial settle");

    let mut check_tx = uow.begin_transaction().expect("check tx");
    let updated_sale = check_tx.sales().find_by_id(2).expect("find").expect("exists");
    assert_eq!(updated_sale.balance_due, 300.0);
    assert_eq!(updated_sale.payment_status, PaymentStatus::Partial);
}

#[test]
fn test_tc_dues_003_daily_cash_drawer_settlement_reconciliation() {
    let (accounting_service, uow) = setup_accounting_service();
    let mut tx = uow.begin_transaction().expect("begin tx");

    let party = create_test_party(3, "Vikas Book Depot", PartyType::Customer, None);
    tx.parties().create(&party).expect("create party");

    let sale_cash = create_test_sale_tx(10, "SAL-1010", None, 1000.0, 1000.0, "2026-11-20");
    tx.sales().create_transaction(&sale_cash).expect("create cash sale");

    let credit_bill = create_test_sale_tx(11, "SAL-1011", Some(3), 400.0, 0.0, "2026-11-19");
    tx.sales().create_transaction(&credit_bill).expect("create credit bill");
    tx.commit().expect("commit");

    let payload = SettlePaymentPayload {
        party_id: 3,
        amount: 400.0,
        settlement_date: "2026-11-20".to_string(),
        payment_mode: Some("CASH".to_string()),
        specific_bill_id: Some(11),
        notes: Some("Khata settlement".to_string()),
    };
    accounting_service.settle_bill_payment(payload).expect("settle");

    let drawer_total = accounting_service.calculate_live_cash_drawer("2026-11-20").expect("calculate drawer");
    assert_eq!(drawer_total, 1400.0, "Cash drawer must reconcile: ₹1000 (upfront) + ₹400 (settled) = ₹1400");
}
