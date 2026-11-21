// tests/purchase_service_test.rs
//! Test Suite: Inward Purchase Service & Lot Cost Normalization
//! Test Cases: TC-PUR-001 through TC-PUR-005
//! Derived from book_keeper_LLD.md § 11.6. Runs the production services on real SQLite.

mod common;

use common::fixtures::*;
use common::ledger::*;
use common::stock::assert_stock_invariant;
use common::*;
use book_keeper::core::dtos::*;
use book_keeper::core::error::AppError;
use book_keeper::core::value_objects::*;
use book_keeper::data::contracts::unit_of_work::UnitOfWork;
use book_keeper::services::contracts::purchase_service::PurchaseService;

type Purchases = PurchaseServiceImpl<InventoryServiceImpl<FifoStrategy>, PaymentServiceImpl>;

fn setup() -> (Purchases, TestDb) {
    let db = TestDb::new();
    let inv = InventoryServiceImpl::new(FifoStrategy, db.clone());
    (PurchaseServiceImpl::new(inv, PaymentServiceImpl, db.clone()), db)
}

fn seed_item(db: &TestDb, id: i64, price: f64) {
    let mut tx = db.begin_transaction().unwrap();
    tx.items().create(&create_test_item(id, &format!("Item {id}"), None, None, 0.0, price)).unwrap();
    tx.commit().unwrap();
}

fn purchase_count(db: &TestDb) -> usize {
    let mut tx = db.begin_read().unwrap();
    tx.purchases().list_purchases(None, None, None, None, 0, 1000).unwrap().len()
}

#[test]
fn test_tc_pur_001_loose_base_unit_inward() {
    let (purchases, db) = setup();
    seed_item(&db, 1, 70.0);

    let r = purchases
        .create_purchase(purchase_payload(None, PaymentStatus::Paid, 500.0, &[(1, None, 50.0, 10.0)]))
        .unwrap();
    assert!(r.purchase_id > 0);
    assert_eq!(r.transaction_code, "PUR-1001");

    let mut tx = db.begin_read().unwrap();
    let pur = tx.purchases().find_transaction_by_id(r.purchase_id).unwrap().unwrap();
    assert_eq!(pur.source, LotSource::Purchase);
    assert_eq!(pur.total_amount, 500.0);
    assert_eq!((pur.paid_amount, pur.balance_due, pur.payment_status), (500.0, 0.0, PaymentStatus::Paid));
    let lots = tx.purchases().get_active_lots_for_item(1).unwrap();
    assert_eq!(lots.len(), 1);
    assert_eq!((lots[0].remaining_quantity, lots[0].unit_cost), (50.0, 10.0));
    assert_eq!(lots[0].item_name, "Item 1");
    let item = tx.items().find_by_id(1).unwrap().unwrap();
    assert_eq!(item.current_stock, 50.0);
    assert!(!item.is_low_stock);
    assert_stock_invariant(&mut *tx);
}

#[test]
fn test_tc_pur_002_packaging_bundle_inward_normalization() {
    let (purchases, db) = setup();
    seed_item(&db, 2, 15.0);
    {
        let mut tx = db.begin_transaction().unwrap();
        tx.bundles().create(&create_test_bundle(1, 2, "Carton", 500.0, Some(5000.0), None)).unwrap();
        tx.commit().unwrap();
    }
    purchases
        .create_purchase(purchase_payload(None, PaymentStatus::Paid, 8000.0, &[(2, Some(1), 2.0, 4000.0)]))
        .unwrap();

    let mut tx = db.begin_read().unwrap();
    let lots = tx.purchases().get_active_lots_for_item(2).unwrap();
    assert_eq!(lots.len(), 1);
    assert_eq!((lots[0].quantity, lots[0].unit_cost, lots[0].remaining_quantity), (1000.0, 8.0, 1000.0));
    assert_eq!(lots[0].pack_label.as_deref(), Some("Carton"));
    assert_eq!(lots[0].pack_quantity, 2.0);
    assert_eq!(tx.items().find_by_id(2).unwrap().unwrap().current_stock, 1000.0);
}

#[test]
fn test_tc_pur_003_partial_payment_supplier_purchase() {
    let (purchases, db) = setup();
    seed_party(&db, 5, "National Stationery Wholesalers", PartyType::Vendor);
    seed_item(&db, 3, 60.0);

    let r = purchases
        .create_purchase(purchase_payload(Some(5), PaymentStatus::Partial, 2000.0, &[(3, None, 100.0, 50.0)]))
        .unwrap();
    let mut tx = db.begin_read().unwrap();
    let pur = tx.purchases().find_transaction_by_id(r.purchase_id).unwrap().unwrap();
    assert_eq!(pur.total_amount, 5000.0);
    assert_eq!((pur.paid_amount, pur.balance_due, pur.payment_status), (2000.0, 3000.0, PaymentStatus::Partial));
    assert_eq!(pur.party_name.as_deref(), Some("National Stationery Wholesalers"));
    let rows = tx.settlements().list_by_transaction(TransactionType::Purchase, r.purchase_id).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!((rows[0].kind, rows[0].amount_paid), (PaymentKind::Upfront, 2000.0));
    // Vendor payables increased by the unpaid 3000.
    let open = tx.purchases().list_open_for_party(5).unwrap();
    assert_eq!(open.iter().map(|b| b.balance_due).sum::<f64>(), 3000.0);
}

#[test]
fn test_tc_pur_004_purchase_overpayment_rejected() {
    let (purchases, db) = setup();
    seed_item(&db, 1, 10.0);
    let err = purchases
        .create_purchase(purchase_payload(None, PaymentStatus::Paid, 1200.0, &[(1, None, 100.0, 10.0)]))
        .unwrap_err();
    assert!(matches!(err, AppError::ValidationError { .. }), "{err:?}");
    // Overpay is rejected for every status, not only PAID.
    seed_party(&db, 1, "V", PartyType::Vendor);
    let err = purchases
        .create_purchase(purchase_payload(Some(1), PaymentStatus::Partial, 1200.0, &[(1, None, 100.0, 10.0)]))
        .unwrap_err();
    assert!(matches!(err, AppError::ValidationError { .. }));
    assert_eq!(purchase_count(&db), 0);
    assert_eq!(ledger_row_count(&db), 0);
    let mut tx = db.begin_read().unwrap();
    assert_eq!(tx.items().find_by_id(1).unwrap().unwrap().current_stock, 0.0);
}

#[test]
fn test_tc_pur_005_supplier_invoice_ref_persisted() {
    let (purchases, db) = setup();
    seed_item(&db, 1, 10.0);
    let mut p = purchase_payload(None, PaymentStatus::Paid, 100.0, &[(1, None, 10.0, 10.0)]);
    p.supplier_invoice_ref = Some("  INV-77 ".to_string());
    p.notes = Some("dock 3".to_string());
    let r = purchases.create_purchase(p).unwrap();

    let hist = purchases.get_purchases_history(PurchaseHistoryFilter::default()).unwrap();
    assert_eq!(hist.items.len(), 1);
    assert_eq!(hist.items[0].supplier_invoice_ref.as_deref(), Some("INV-77"));
    assert_eq!(hist.items[0].notes.as_deref(), Some("dock 3"), "ref is not folded into notes (DD-25)");
    let d = purchases.get_purchase_details(r.purchase_id).unwrap();
    assert_eq!(d.purchase.supplier_invoice_ref.as_deref(), Some("INV-77"));
}

#[test]
fn test_purchase_status_party_and_validation_rules() {
    let (purchases, db) = setup();
    seed_party(&db, 1, "Vendor", PartyType::Vendor);
    seed_party(&db, 2, "Cust", PartyType::Customer);
    seed_item(&db, 1, 10.0);
    let line = [(1, None, 10.0, 10.0)]; // total 100

    let field = |e: AppError| match e {
        AppError::ValidationError { field, .. } => field,
        other => panic!("{other:?}"),
    };
    // Status / paid cross-check.
    for (status, paid, party) in [
        (PaymentStatus::Paid, 50.0, None),
        (PaymentStatus::Partial, 0.0, Some(1)),
        (PaymentStatus::Partial, 100.0, Some(1)),
        (PaymentStatus::Unpaid, 10.0, Some(1)),
    ] {
        let e = purchases.create_purchase(purchase_payload(party, status, paid, &line)).unwrap_err();
        assert_eq!(field(e), "paid_amount", "{status:?} {paid}");
    }
    // Credit needs a vendor; a customer is not a vendor.
    let e = purchases.create_purchase(purchase_payload(None, PaymentStatus::Unpaid, 0.0, &line)).unwrap_err();
    assert_eq!(field(e), "party_id");
    let e = purchases.create_purchase(purchase_payload(Some(2), PaymentStatus::Unpaid, 0.0, &line)).unwrap_err();
    assert_eq!(field(e), "party_id");
    // Bad numbers.
    for (qty, cost) in [(0.0, 10.0), (1.0, -1.0), (f64::NAN, 1.0)] {
        assert!(purchases.create_purchase(purchase_payload(None, PaymentStatus::Paid, 0.0, &[(1, None, qty, cost)])).is_err());
    }
    assert!(purchases.create_purchase(purchase_payload(None, PaymentStatus::Paid, 0.0, &[])).is_err());
    assert_eq!(purchase_count(&db), 0);

    // Fully unpaid works and leaves the header unpaid / no ledger row.
    let r = purchases.create_purchase(purchase_payload(Some(1), PaymentStatus::Unpaid, 0.0, &line)).unwrap();
    let mut tx = db.begin_read().unwrap();
    let h = tx.purchases().find_transaction_by_id(r.purchase_id).unwrap().unwrap();
    assert_eq!((h.paid_amount, h.balance_due, h.payment_status), (0.0, 100.0, PaymentStatus::Unpaid));
    assert!(tx.settlements().list_all().unwrap().is_empty());
}
