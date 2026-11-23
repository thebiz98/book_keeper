// tests/item_service_test.rs
//! Test Suite: Item & Bundle master data (validation, bundles, MRP, suggestions)
//! Test Cases: TC-BUN-001 through TC-BUN-009, TC-MRP-01 through TC-MRP-08
//! Derived from book_keeper_LLD.md section 6.6 (ItemService), DD-19, DD-21, DD-24, DD-28.

mod common;

use std::sync::Arc;
use common::ledger::sale_payload;
use common::*;
use book_keeper::core::dtos::*;
use book_keeper::core::error::AppError;
use book_keeper::core::value_objects::PaymentStatus;
use book_keeper::data::contracts::search_indexer::ItemSearchIndexer;
use book_keeper::data::contracts::unit_of_work::UnitOfWork;
use book_keeper::services::contracts::item_service::ItemService;
use book_keeper::services::contracts::sale_service::SaleService;

type Items = ItemServiceImpl<InventoryServiceImpl<FifoStrategy>>;

fn setup() -> (Items, InventoryServiceImpl<FifoStrategy>, TestDb) {
    let db = TestDb::new();
    let inv = InventoryServiceImpl::new(FifoStrategy, db.clone());
    let items = ItemServiceImpl::new(db.clone(), Arc::new(db.indexer()), inv.clone());
    (items, inv, db)
}

fn item_payload(name: &str) -> CreateItemPayload {
    CreateItemPayload { item_type: None,
        name: name.into(), category: None, barcode: None, base_unit: "PCS".into(), mrp: None,
        default_sale_price: 10.0, initial_stock: None, initial_cost: None, bundles: None,
    }
}

fn bundle_payload(item_id: i64, name: &str, units: f64) -> CreateBundlePayload {
    CreateBundlePayload {
        item_id: Some(item_id), bundle_name: name.into(), units_per_bundle: units,
        mrp: None, default_bundle_price: None, barcode: None,
    }
}

fn update_payload(name: &str) -> UpdateItemPayload {
    UpdateItemPayload { item_type: None,
        name: name.into(), category: None, barcode: None, base_unit: "PCS".into(), mrp: None,
        default_sale_price: 10.0,
    }
}

fn bundle_update(name: &str, units: f64) -> UpdateBundlePayload {
    UpdateBundlePayload {
        bundle_name: name.into(), units_per_bundle: units, mrp: None, default_bundle_price: None, barcode: None,
    }
}

fn field_of<T: std::fmt::Debug>(r: Result<T, AppError>) -> String {
    match r {
        Err(AppError::ValidationError { field, .. }) => field,
        other => panic!("expected ValidationError, got {other:?}"),
    }
}

fn stored_item(db: &TestDb, id: i64) -> book_keeper::core::entities::Item {
    db.begin_read().unwrap().items().find_by_id(id).unwrap().unwrap()
}

fn stored_bundles(db: &TestDb, item_id: i64) -> Vec<book_keeper::core::entities::ItemBundle> {
    db.begin_read().unwrap().bundles().find_by_item_id(item_id).unwrap()
}

// ------------------------------- bundles -------------------------------

#[test]
fn test_tc_bun_001_create_bundle_returns_full_bundle_and_indexes() {
    let (items, _, db) = setup();
    let id = items.create_item(item_payload("Pen")).unwrap();
    let mut p = bundle_payload(id, "  Box of 10 ", 10.0);
    p.default_bundle_price = Some(90.0);
    p.barcode = Some("BX-1".into());
    let b = items.create_bundle(p).unwrap();
    assert!(b.id > 0);
    assert_eq!((b.item_id, b.bundle_name.as_str(), b.units_per_bundle), (id, "Box of 10", 10.0));
    assert_eq!(b.default_bundle_price, Some(90.0));
    assert_eq!(stored_bundles(&db, id), vec![b.clone()]);
    // Search by the bundle barcode finds the parent and flags the bundle.
    let hits = db.indexer().search("BX-1").unwrap();
    assert_eq!(hits[0].item_id, id);
    assert_eq!(hits[0].matched_bundle_id, Some(b.id));
}

#[test]
fn test_tc_bun_002_update_bundle_any_field_stays_on_item() {
    let (items, _, db) = setup();
    let id = items.create_item(item_payload("Pen")).unwrap();
    let b = items.create_bundle(bundle_payload(id, "Box", 10.0)).unwrap();
    let up = items
        .update_bundle(b.id, UpdateBundlePayload {
            bundle_name: "Carton".into(), units_per_bundle: 24.0, mrp: Some(300.0),
            default_bundle_price: Some(250.0), barcode: Some("CT-9".into()),
        })
        .unwrap();
    assert_eq!((up.id, up.item_id, up.bundle_name.as_str(), up.units_per_bundle), (b.id, id, "Carton", 24.0));
    assert_eq!(stored_bundles(&db, id), vec![up]);
    assert_eq!(db.indexer().search("CT-9").unwrap()[0].matched_bundle_id, Some(b.id));
    // Missing bundle.
    assert!(matches!(items.update_bundle(999, bundle_update("X", 5.0)), Err(AppError::NotFound { .. })));
}

#[test]
fn test_tc_bun_003_delete_bundle_and_missing_is_not_found() {
    let (items, _, db) = setup();
    let id = items.create_item(item_payload("Pen")).unwrap();
    let mut p = bundle_payload(id, "Box", 10.0);
    p.barcode = Some("BX-7".into());
    let b = items.create_bundle(p).unwrap();
    items.delete_bundle(b.id).unwrap();
    assert!(stored_bundles(&db, id).is_empty());
    assert!(db.indexer().search("BX-7").unwrap().is_empty(), "bundle barcode row removed from the index");
    assert!(matches!(items.delete_bundle(b.id), Err(AppError::NotFound { .. })));
}

#[test]
fn test_tc_bun_004_multiplier_must_exceed_one() {
    let (items, _, _) = setup();
    let id = items.create_item(item_payload("Pen")).unwrap();
    for bad in [1.0, 0.5, 0.0, -3.0, f64::NAN, f64::INFINITY] {
        assert_eq!(field_of(items.create_bundle(bundle_payload(id, "Box", bad))), "units_per_bundle", "{bad}");
    }
    let b = items.create_bundle(bundle_payload(id, "Box", 1.5)).unwrap();
    assert_eq!(field_of(items.update_bundle(b.id, bundle_update("Box", 1.0))), "units_per_bundle");
    // Nested bundle in create_item.
    let mut p = item_payload("Nested");
    p.bundles = Some(vec![bundle_payload(0, "Bad", 1.0)]);
    assert_eq!(field_of(items.create_item(p)), "units_per_bundle");
    // Negative price / empty name.
    let mut neg = bundle_payload(id, "Neg", 5.0);
    neg.default_bundle_price = Some(-1.0);
    assert_eq!(field_of(items.create_bundle(neg)), "default_bundle_price");
    assert_eq!(field_of(items.create_bundle(bundle_payload(id, "   ", 5.0))), "bundle_name");
}

#[test]
fn test_tc_bun_005_duplicate_bundle_name_per_item() {
    let (items, _, _) = setup();
    let a = items.create_item(item_payload("Pen")).unwrap();
    let c = items.create_item(item_payload("Cap")).unwrap();
    let b = items.create_bundle(bundle_payload(a, "Box", 10.0)).unwrap();
    assert_eq!(field_of(items.create_bundle(bundle_payload(a, "box", 12.0))), "bundle_name");
    // Same name on another item is fine.
    items.create_bundle(bundle_payload(c, "Box", 10.0)).unwrap();
    // Renaming another bundle onto an existing name is rejected; keeping own name is fine.
    let b2 = items.create_bundle(bundle_payload(a, "Dozen", 12.0)).unwrap();
    assert_eq!(field_of(items.update_bundle(b2.id, bundle_update("BOX", 12.0))), "bundle_name");
    items.update_bundle(b.id, bundle_update("Box", 11.0)).unwrap();
    // Nested duplicates in create_item.
    let mut p = item_payload("Dup");
    p.bundles = Some(vec![bundle_payload(0, "Box", 10.0), bundle_payload(0, "box", 20.0)]);
    assert_eq!(field_of(items.create_item(p)), "bundle_name");
}

#[test]
fn test_tc_bun_006_barcode_collision_between_items_and_bundles() {
    let (items, _, db) = setup();
    let mut p = item_payload("Pen");
    p.barcode = Some("111".into());
    let pen = items.create_item(p).unwrap();
    // Bundle barcode == item barcode.
    let mut b = bundle_payload(pen, "Box", 10.0);
    b.barcode = Some("111".into());
    assert_eq!(field_of(items.create_bundle(b)), "barcode");
    let mut b = bundle_payload(pen, "Box", 10.0);
    b.barcode = Some("222".into());
    let bundle = items.create_bundle(b).unwrap();
    // Item barcode == bundle barcode, on create and update.
    let mut p = item_payload("Cap");
    p.barcode = Some("222".into());
    assert_eq!(field_of(items.create_item(p)), "barcode");
    let cap = items.create_item(item_payload("Cap")).unwrap();
    let mut u = update_payload("Cap");
    u.barcode = Some("222".into());
    assert_eq!(field_of(items.update_item(cap, u)), "barcode");
    // Bundle-to-bundle collision across items.
    let mut b = bundle_payload(cap, "Pack", 6.0);
    b.barcode = Some("222".into());
    assert_eq!(field_of(items.create_bundle(b)), "barcode");
    // Re-saving own barcode is fine.
    let mut u = update_payload("Pen");
    u.barcode = Some("111".into());
    items.update_item(pen, u).unwrap();
    items.update_bundle(bundle.id, UpdateBundlePayload { barcode: Some("222".into()), ..bundle_update("Box", 10.0) }).unwrap();
    // Nested bundle barcode collides with the new item's own barcode.
    let mut p = item_payload("Nest");
    p.barcode = Some("333".into());
    let mut nb = bundle_payload(0, "Box", 10.0);
    nb.barcode = Some("333".into());
    p.bundles = Some(vec![nb]);
    assert_eq!(field_of(items.create_item(p)), "barcode");
    assert!(db.begin_read().unwrap().items().find_by_name("Nest").unwrap().is_none(), "rolled back");
}

#[test]
fn test_tc_bun_007_suggestions_learn_case_insensitive_unrounded_active_only() {
    let (items, _, db) = setup();
    let ids: Vec<i64> = (1..=4).map(|i| items.create_item(item_payload(&format!("It{i}"))).unwrap()).collect();
    items.create_bundle(bundle_payload(ids[0], "Box", 10.0)).unwrap();
    items.create_bundle(bundle_payload(ids[1], "BOX", 10.0)).unwrap();
    items.create_bundle(bundle_payload(ids[2], "box", 10.0)).unwrap();
    items.create_bundle(bundle_payload(ids[0], "Half", 2.5)).unwrap();
    items.create_bundle(bundle_payload(ids[3], "Box", 12.0)).unwrap();
    let s = items.get_bundle_suggestions().unwrap();
    assert_eq!(s.len(), 3);
    assert_eq!((s[0].bundle_name.to_lowercase().as_str(), s[0].units_per_bundle, s[0].usage_count), ("box", 10.0, 3));
    assert!(s.iter().any(|x| x.bundle_name == "Half" && x.units_per_bundle == 2.5), "multiplier not rounded");
    // Archived items stop contributing.
    {
        let mut tx = db.begin_transaction().unwrap();
        let mut it = tx.items().find_by_id(ids[3]).unwrap().unwrap();
        it.is_active = false;
        tx.items().update(&it).unwrap();
        tx.commit().unwrap();
    }
    let s = items.get_bundle_suggestions().unwrap();
    assert!(!s.iter().any(|x| x.units_per_bundle == 12.0));
}

#[test]
fn test_tc_bun_008_delete_bundle_leaves_bill_history_unchanged() {
    let (items, _, db) = setup();
    let mut p = item_payload("Pen");
    p.initial_stock = Some(100.0);
    p.initial_cost = Some(5.0);
    let id = items.create_item(p).unwrap();
    let b = items.create_bundle(bundle_payload(id, "Box of 10", 10.0)).unwrap();
    let inv = InventoryServiceImpl::new(FifoStrategy, db.clone());
    let sales = SaleServiceImpl::new(inv, PaymentServiceImpl, db.clone());
    let r = sales
        .create_sale(sale_payload(None, PaymentStatus::Paid, 100.0, &[(id, Some(b.id), 2.0, 50.0)]))
        .unwrap();
    items.delete_bundle(b.id).unwrap();
    let d = sales.get_sale_details(r.sale_id).unwrap();
    assert_eq!(d.items[0].sale_item.pack_label.as_deref(), Some("Box of 10"));
    assert_eq!(d.items[0].sale_item.quantity, 20.0);
    assert_eq!(stored_item(&db, id).current_stock, 80.0);
}

#[test]
fn test_tc_bun_009_item_name_validation_and_uniqueness() {
    let (items, _, _) = setup();
    assert_eq!(field_of(items.create_item(item_payload("   "))), "name");
    let id = items.create_item(item_payload("  Pen  ")).unwrap();
    assert_eq!(field_of(items.create_item(item_payload("PEN"))), "name");
    let other = items.create_item(item_payload("Cap")).unwrap();
    assert_eq!(field_of(items.update_item(other, update_payload("pen"))), "name");
    items.update_item(id, update_payload("PEN")).unwrap(); // own name, new case
    let mut p = item_payload("Neg");
    p.default_sale_price = -1.0;
    assert_eq!(field_of(items.create_item(p)), "default_sale_price");
    let mut p = item_payload("Nan");
    p.default_sale_price = f64::NAN;
    assert_eq!(field_of(items.create_item(p)), "default_sale_price");
}

// --------------------------------- MRP ---------------------------------

#[test]
fn test_tc_mrp_01_null_mrp_accepted() {
    let (items, _, db) = setup();
    let id = items.create_item(item_payload("A")).unwrap();
    assert_eq!(stored_item(&db, id).mrp, None);
}

#[test]
fn test_tc_mrp_02_positive_mrp_stored() {
    let (items, _, db) = setup();
    let mut p = item_payload("A");
    p.mrp = Some(25.5);
    let id = items.create_item(p).unwrap();
    assert_eq!(stored_item(&db, id).mrp, Some(25.5));
}

#[test]
fn test_tc_mrp_03_zero_mrp_rejected_on_create_and_update() {
    let (items, _, _) = setup();
    let mut p = item_payload("A");
    p.mrp = Some(0.0);
    assert_eq!(field_of(items.create_item(p)), "mrp");
    let id = items.create_item(item_payload("B")).unwrap();
    let mut u = update_payload("B");
    u.mrp = Some(0.0);
    assert_eq!(field_of(items.update_item(id, u)), "mrp");
}

#[test]
fn test_tc_mrp_04_negative_mrp_rejected() {
    let (items, _, _) = setup();
    let mut p = item_payload("A");
    p.mrp = Some(-5.0);
    assert_eq!(field_of(items.create_item(p)), "mrp");
}

#[test]
fn test_tc_mrp_05_nan_and_infinite_mrp_rejected() {
    let (items, _, _) = setup();
    for bad in [f64::NAN, f64::INFINITY] {
        let mut p = item_payload("A");
        p.mrp = Some(bad);
        assert_eq!(field_of(items.create_item(p)), "mrp");
    }
}

#[test]
fn test_tc_mrp_06_update_can_clear_mrp() {
    let (items, _, db) = setup();
    let mut p = item_payload("A");
    p.mrp = Some(30.0);
    let id = items.create_item(p).unwrap();
    items.update_item(id, update_payload("A")).unwrap();
    assert_eq!(stored_item(&db, id).mrp, None);
}

#[test]
fn test_tc_mrp_07_bundle_mrp_validated() {
    let (items, _, _) = setup();
    let id = items.create_item(item_payload("A")).unwrap();
    for bad in [0.0, -1.0, f64::NAN] {
        let mut b = bundle_payload(id, "Box", 10.0);
        b.mrp = Some(bad);
        assert_eq!(field_of(items.create_bundle(b)), "mrp");
    }
    let mut b = bundle_payload(id, "Box", 10.0);
    b.mrp = Some(120.0);
    let made = items.create_bundle(b).unwrap();
    assert_eq!(made.mrp, Some(120.0));
    let cleared = items.update_bundle(made.id, bundle_update("Box", 10.0)).unwrap();
    assert_eq!(cleared.mrp, None);
}

#[test]
fn test_tc_mrp_08_update_item_never_touches_stock() {
    let (items, _, db) = setup();
    let mut p = item_payload("A");
    p.initial_stock = Some(12.0);
    p.initial_cost = Some(3.0);
    let id = items.create_item(p).unwrap();
    let mut u = update_payload("A2");
    u.mrp = Some(40.0);
    u.default_sale_price = 20.0;
    items.update_item(id, u).unwrap();
    let it = stored_item(&db, id);
    assert_eq!((it.current_stock, it.mrp, it.default_sale_price), (12.0, Some(40.0), 20.0));
}
