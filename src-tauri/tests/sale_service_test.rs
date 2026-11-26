// tests/sale_service_test.rs
//! Test Suite: Sale Checkout Orchestration & Rapid Billing
//! Test Cases: TC-SALE-001 through TC-SALE-011
//! Derived from book_keeper_LLD.md § 11.5. Runs the production services on real SQLite.

mod common;

use common::fixtures::*;
use common::ledger::*;
use common::stock::assert_stock_invariant;
use common::*;
use book_keeper::core::error::AppError;
use book_keeper::core::value_objects::*;
use book_keeper::data::contracts::unit_of_work::UnitOfWork;
use book_keeper::services::contracts::sale_service::SaleService;

type Sales = SaleServiceImpl<InventoryServiceImpl<FifoStrategy>, PaymentServiceImpl>;

fn setup() -> (Sales, InventoryServiceImpl<FifoStrategy>, TestDb) {
    let db = TestDb::new();
    let inv = InventoryServiceImpl::new(FifoStrategy, db.clone());
    let sales = SaleServiceImpl::new(inv.clone(), PaymentServiceImpl, db.clone());
    (sales, inv, db)
}

fn stock(db: &TestDb, item_id: i64) -> f64 {
    let mut tx = db.begin_read().unwrap();
    tx.items().find_by_id(item_id).unwrap().unwrap().current_stock
}

fn sale_count(db: &TestDb) -> usize {
    let mut tx = db.begin_read().unwrap();
    tx.sales().list_sales(None, None, None, None, 0, 1000).unwrap().len()
}

fn field_of(err: AppError) -> String {
    match err {
        AppError::ValidationError { field, .. } => field,
        other => panic!("expected ValidationError, got {other:?}"),
    }
}

#[test]
fn test_tc_sale_001_full_cash_sale_paid() {
    let (sales, inv, db) = setup();
    seed_stocked_item(&db, &inv, 1, 20.0, 10.0, 15.0);

    let r = sales
        .create_sale(sale_payload(None, PaymentStatus::Paid, 30.0, &[(1, None, 2.0, 15.0)]))
        .unwrap();
    assert_eq!(r.payment_status, PaymentStatus::Paid);
    assert_eq!((r.total_amount, r.paid_amount, r.balance_due, r.change_amount), (30.0, 30.0, 0.0, 0.0));
    assert_eq!(stock(&db, 1), 18.0);

    let mut tx = db.begin_read().unwrap();
    let h = tx.sales().find_transaction_by_id(r.sale_id).unwrap().unwrap();
    assert_eq!((h.paid_amount, h.balance_due, h.payment_status), (30.0, 0.0, PaymentStatus::Paid));
    assert_eq!(h.transaction_code, "SAL-1001");
    assert_stock_invariant(&mut *tx);
}

#[test]
fn test_tc_sale_002_partial_payment_sale() {
    let (sales, inv, db) = setup();
    seed_party(&db, 1, "Sharma Ji", PartyType::Customer);
    seed_stocked_item(&db, &inv, 2, 10.0, 60.0, 100.0);

    // Credit needs a customer (walk-in PARTIAL is rejected, see TC-SALE-010).
    let r = sales
        .create_sale(sale_payload(Some(1), PaymentStatus::Partial, 40.0, &[(2, None, 1.0, 100.0)]))
        .unwrap();
    assert_eq!(r.payment_status, PaymentStatus::Partial);
    assert_eq!((r.total_amount, r.paid_amount, r.balance_due, r.change_amount), (100.0, 40.0, 60.0, 0.0));
}

#[test]
fn test_tc_sale_003_full_credit_sale_unpaid() {
    let (sales, inv, db) = setup();
    seed_party(&db, 10, "Sharma Ji", PartyType::Customer);
    seed_stocked_item(&db, &inv, 3, 50.0, 10.0, 20.0);

    let r = sales
        .create_sale(sale_payload(Some(10), PaymentStatus::Unpaid, 0.0, &[(3, None, 5.0, 20.0)]))
        .unwrap();
    assert_eq!(r.payment_status, PaymentStatus::Unpaid);
    assert_eq!(r.balance_due, 100.0);

    let mut tx = db.begin_read().unwrap();
    let open = tx.sales().list_open_for_party(10).unwrap();
    assert_eq!(open.len(), 1);
    assert_eq!(open[0].balance_due, 100.0);
    assert_eq!(open[0].party_name.as_deref(), Some("Sharma Ji"));
    assert!(tx.settlements().list_all().unwrap().is_empty(), "no upfront row for a credit sale");
}

#[test]
fn test_tc_sale_004_bundle_unit_sale_multiplier_conversion() {
    let (sales, inv, db) = setup();
    seed_stocked_item(&db, &inv, 4, 100.0, 5.0, 10.0);
    {
        let mut tx = db.begin_transaction().unwrap();
        tx.bundles().create(&create_test_bundle(1, 4, "Box of 10", 10.0, Some(90.0), None)).unwrap();
        tx.commit().unwrap();
    }
    let r = sales
        .create_sale(sale_payload(None, PaymentStatus::Paid, 180.0, &[(4, Some(1), 2.0, 90.0)]))
        .unwrap();
    assert_eq!(r.total_amount, 180.0);
    assert_eq!(stock(&db, 4), 80.0);

    let mut tx = db.begin_read().unwrap();
    let lines = tx.sales().list_items_by_transaction_id(r.sale_id).unwrap();
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].quantity, 20.0);
    assert_eq!(lines[0].unit_price, 9.0);
    assert_eq!(lines[0].pack_label.as_deref(), Some("Box of 10"));
    assert_eq!(lines[0].pack_quantity, 2.0);
    assert_eq!(lines[0].item_name, "Item 4", "name snapshot (DD-30)");
}

#[test]
fn test_tc_sale_005_low_stock_flag_transition() {
    let (sales, inv, db) = setup();
    seed_stocked_item(&db, &inv, 5, 6.0, 30.0, 45.0);
    sales.create_sale(sale_payload(None, PaymentStatus::Paid, 90.0, &[(5, None, 2.0, 45.0)])).unwrap();

    let mut tx = db.begin_read().unwrap();
    let item = tx.items().find_by_id(5).unwrap().unwrap();
    assert_eq!(item.current_stock, 4.0);
    assert!(item.is_low_stock);
    assert!(tx.items().list_catalog(None, true, 0, 10).unwrap().iter().any(|i| i.id == 5));
}

#[test]
fn test_tc_sale_006_empty_cart_validation() {
    let (sales, _inv, _db) = setup();
    let err = sales.create_sale(sale_payload(None, PaymentStatus::Paid, 0.0, &[])).unwrap_err();
    match err {
        AppError::ValidationError { field, message } => {
            assert_eq!(field, "items");
            assert!(message.contains("items cannot be empty"));
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn test_tc_sale_007_sequential_transaction_code_generation() {
    let (sales, _inv, db) = setup();
    let mut tx = db.begin_transaction().unwrap();
    tx.sales().create_transaction(&create_test_sale_tx(5, "SAL-1005", None, 100.0, 100.0, DAY)).unwrap();
    tx.commit().unwrap();
    assert_eq!(sales.get_next_sale_code().unwrap(), "SAL-1006");
}

#[test]
fn test_tc_sale_008_status_paid_mismatch_rejected() {
    let (sales, inv, db) = setup();
    seed_party(&db, 1, "Cust", PartyType::Customer);
    seed_stocked_item(&db, &inv, 1, 10.0, 10.0, 100.0);
    let line = [(1, None, 1.0, 100.0)];

    // PAID with paid < total; PARTIAL with paid = 0; PARTIAL with paid >= total;
    // UNPAID with paid > 0; negative / NaN paid.
    for (status, paid) in [
        (PaymentStatus::Paid, 99.0),
        (PaymentStatus::Partial, 0.0),
        (PaymentStatus::Partial, 100.0),
        (PaymentStatus::Unpaid, 1.0),
        (PaymentStatus::Paid, -5.0),
        (PaymentStatus::Paid, f64::NAN),
    ] {
        let err = sales.create_sale(sale_payload(Some(1), status, paid, &line)).unwrap_err();
        assert_eq!(field_of(err), "paid_amount", "{status:?} paid {paid}");
    }
    assert_eq!(sale_count(&db), 0);
    assert_eq!(ledger_row_count(&db), 0);
    assert_eq!(stock(&db, 1), 10.0);
}

#[test]
fn test_tc_sale_009_aggregate_stock_check_across_lines() {
    let (sales, inv, db) = setup();
    seed_stocked_item(&db, &inv, 1, 5.0, 10.0, 20.0);
    let err = sales
        .create_sale(sale_payload(None, PaymentStatus::Paid, 120.0, &[(1, None, 3.0, 20.0), (1, None, 3.0, 20.0)]))
        .unwrap_err();
    match err {
        AppError::InsufficientStockError { item_id, required, available, .. } => {
            assert_eq!(item_id, 1);
            assert_eq!((required, available), (6.0, 5.0));
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(sale_count(&db), 0);
    assert_eq!(stock(&db, 1), 5.0);
}

#[test]
fn test_tc_sale_010_credit_requires_customer() {
    let (sales, inv, db) = setup();
    seed_stocked_item(&db, &inv, 1, 10.0, 10.0, 100.0);
    for (status, paid) in [(PaymentStatus::Partial, 40.0), (PaymentStatus::Unpaid, 0.0)] {
        let err = sales.create_sale(sale_payload(None, status, paid, &[(1, None, 1.0, 100.0)])).unwrap_err();
        assert_eq!(field_of(err), "party_id");
    }
    assert_eq!(sale_count(&db), 0);
}

#[test]
fn test_tc_sale_011_change_on_overpaid_cash_sale() {
    let (sales, inv, db) = setup();
    seed_stocked_item(&db, &inv, 1, 10.0, 10.0, 95.0);
    let r = sales
        .create_sale(sale_payload(None, PaymentStatus::Paid, 100.0, &[(1, None, 1.0, 95.0)]))
        .unwrap();
    assert_eq!((r.total_amount, r.paid_amount, r.balance_due, r.change_amount), (95.0, 95.0, 0.0, 5.0));
    assert_eq!(r.payment_status, PaymentStatus::Paid);

    let mut tx = db.begin_read().unwrap();
    let h = tx.sales().find_transaction_by_id(r.sale_id).unwrap().unwrap();
    assert_eq!(h.paid_amount, 95.0);
    let rows = tx.settlements().list_by_transaction(TransactionType::Sale, r.sale_id).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!((rows[0].kind, rows[0].amount_paid), (PaymentKind::Upfront, 95.0));
}

#[test]
fn test_sale_validation_party_item_bundle_and_numbers() {
    let (sales, inv, db) = setup();
    seed_party(&db, 1, "Vendor", PartyType::Vendor);
    seed_party(&db, 2, "Cust", PartyType::Customer);
    seed_stocked_item(&db, &inv, 1, 10.0, 10.0, 10.0);
    seed_stocked_item(&db, &inv, 2, 10.0, 10.0, 10.0);
    {
        let mut tx = db.begin_transaction().unwrap();
        tx.bundles().create(&create_test_bundle(1, 2, "Pack", 5.0, None, None)).unwrap();
        let mut inactive = create_test_item(3, "Gone", None, None, 0.0, 1.0);
        inactive.is_active = false;
        tx.items().create(&inactive).unwrap();
        let mut p = create_test_party(3, "Dormant", PartyType::Customer, None);
        p.is_active = false;
        tx.parties().create(&p).unwrap();
        tx.commit().unwrap();
    }
    let ok = [(1, None, 1.0, 10.0)];
    // Vendor / inactive / unknown party.
    assert_eq!(field_of(sales.create_sale(sale_payload(Some(1), PaymentStatus::Paid, 10.0, &ok)).unwrap_err()), "party_id");
    // DD-90: a deleted party is a DeletedReference (Restore it first), not a plain validation error.
    assert!(matches!(
        sales.create_sale(sale_payload(Some(3), PaymentStatus::Paid, 10.0, &ok)),
        Err(AppError::DeletedReference { ref entity, id: 3, .. }) if entity == "PARTY"
    ));
    assert!(matches!(
        sales.create_sale(sale_payload(Some(77), PaymentStatus::Paid, 10.0, &ok)),
        Err(AppError::NotFound { .. })
    ));
    // Inactive / unknown item; bundle of another item.
    assert_eq!(field_of(sales.create_sale(sale_payload(None, PaymentStatus::Paid, 1.0, &[(3, None, 1.0, 1.0)])).unwrap_err()), "item_id");
    assert!(matches!(
        sales.create_sale(sale_payload(None, PaymentStatus::Paid, 1.0, &[(99, None, 1.0, 1.0)])),
        Err(AppError::NotFound { .. })
    ));
    assert_eq!(field_of(sales.create_sale(sale_payload(None, PaymentStatus::Paid, 10.0, &[(1, Some(1), 1.0, 10.0)])).unwrap_err()), "bundle_id");
    // Bad numbers and date.
    for (qty, price) in [(0.0, 10.0), (-1.0, 10.0), (f64::NAN, 10.0), (1.0, -1.0), (1.0, f64::INFINITY)] {
        assert!(matches!(
            sales.create_sale(sale_payload(None, PaymentStatus::Paid, 0.0, &[(1, None, qty, price)])),
            Err(AppError::ValidationError { .. })
        ), "{qty} {price}");
    }
    let mut bad_date = sale_payload(None, PaymentStatus::Paid, 10.0, &ok);
    bad_date.transaction_date = "2026/11/20".to_string();
    assert_eq!(field_of(sales.create_sale(bad_date).unwrap_err()), "transaction_date");
    assert_eq!(sale_count(&db), 0);
}

#[test]
fn test_sale_rounding_and_zero_total() {
    let (sales, inv, db) = setup();
    seed_stocked_item(&db, &inv, 1, 100.0, 1.0, 1.0);
    // 3 x 3.333 = 9.999 -> 10.00 (line total rounded to 2dp, DD-15).
    let r = sales
        .create_sale(sale_payload(None, PaymentStatus::Paid, 10.0, &[(1, None, 3.0, 3.333)]))
        .unwrap();
    assert_eq!((r.total_amount, r.paid_amount), (10.0, 10.0));

    // Zero-value bill: valid only as PAID with paid 0, and leaves a PAID header.
    let z = sales.create_sale(sale_payload(None, PaymentStatus::Paid, 0.0, &[(1, None, 1.0, 0.0)])).unwrap();
    assert_eq!((z.total_amount, z.balance_due, z.payment_status), (0.0, 0.0, PaymentStatus::Paid));
    assert_eq!(
        field_of(sales.create_sale(sale_payload(None, PaymentStatus::Paid, 5.0, &[(1, None, 1.0, 0.0)])).unwrap_err()),
        "paid_amount"
    );
    let mut tx = db.begin_read().unwrap();
    assert_eq!(tx.settlements().list_all().unwrap().len(), 1, "only the 10.00 sale moved money");
}
