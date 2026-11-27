// tests/mechanical_guards_test.rs
//! Test Suite: v1.7 H7 mechanical half (FR-59, DD-97) on real SQLite: Set stock residue and the
//! low-stock rule (S9), stored rounding (L4), a failing reindex after commit (L5), the base-unit
//! lock (L6) and the caps on amounts (L7).

mod common;

use std::sync::Arc;

use book_keeper::core::dtos::*;
use book_keeper::core::entities::{Item, ItemBundle};
use book_keeper::core::error::AppError;
use book_keeper::core::money::{round2, MAX_AMOUNT};
use book_keeper::core::value_objects::*;
use book_keeper::data::contracts::search_indexer::ItemSearchIndexer;
use book_keeper::data::contracts::unit_of_work::UnitOfWork;
use book_keeper::services::contracts::inventory_service::InventoryService;
use book_keeper::services::contracts::item_service::ItemService;
use book_keeper::services::contracts::purchase_service::PurchaseService;
use book_keeper::services::contracts::sale_service::SaleService;
use common::ledger::*;
use common::*;

type Inv = InventoryServiceImpl<FifoStrategy>;

fn field_of<T: std::fmt::Debug>(r: Result<T, AppError>) -> String {
    match r {
        Err(AppError::ValidationError { field, .. }) => field,
        other => panic!("expected ValidationError, got {other:?}"),
    }
}

fn new_item(name: &str, unit: &str) -> CreateItemPayload {
    CreateItemPayload {
        item_type: None, service_cost: None, components: None, low_stock_threshold: Some(0.0),
        name: name.into(), category: None, barcode: None, base_unit: unit.into(), mrp: None,
        default_sale_price: 10.0, initial_stock: None, initial_cost: None, bundles: None,
    }
}

fn edit_item(name: &str, unit: &str) -> UpdateItemPayload {
    UpdateItemPayload {
        item_type: None, service_cost: None, components: None, low_stock_threshold: None,
        name: name.into(), category: None, barcode: None, base_unit: unit.into(), mrp: None,
        default_sale_price: 10.0,
    }
}

/// An indexer whose writes always fail, to prove a committed save never turns into an error (L5).
struct FailingIndexer(Arc<dyn ItemSearchIndexer>);
impl ItemSearchIndexer for FailingIndexer {
    fn search_with_limit(&self, q: &str, l: usize) -> Result<Vec<ItemSearchResult>, AppError> {
        self.0.search_with_limit(q, l)
    }
    fn search_page(&self, q: &str, c: Option<&str>, low: bool, o: usize, l: usize) -> Result<(Vec<ItemSearchResult>, usize), AppError> {
        self.0.search_page(q, c, low, o, l)
    }
    fn index_item_with_bundles(&self, _: &Item, _: &[ItemBundle]) -> Result<(), AppError> {
        Err(AppError::SearchIndexError("disk full".into()))
    }
    fn remove_item(&self, id: i64) -> Result<(), AppError> {
        self.0.remove_item(id)
    }
    fn indexed_item_count(&self) -> Result<usize, AppError> {
        self.0.indexed_item_count()
    }
    fn rebuild_index(&self) -> Result<(), AppError> {
        self.0.rebuild_index()
    }
}

#[test]
fn s9_set_stock_residue_is_snapped_and_never_flags_level_zero() {
    let db = TestDb::new();
    let inv: Inv = InventoryServiceImpl::new(FifoStrategy, db.clone());
    let items = ItemServiceImpl::new(db.clone(), Arc::new(db.indexer()), inv.clone());
    let id = items.create_item(new_item("Rope", "PCS")).unwrap();
    {
        let mut tx = db.begin_transaction().unwrap();
        // The cache holds a float residue, as after many fractional sales.
        tx.items().update_stock_cache(id, 1e-7).unwrap();
        tx.commit().unwrap();
    }
    let r = inv
        .set_item_stock(SetItemStockPayload {
            item_id: id, counted_quantity: 0.0, adjustment_date: DAY.into(), unit_cost: None, notes: None, mode: None,
        })
        .unwrap();
    assert!(r.adjustment_id.is_none());
    let mut rd = db.begin_read().unwrap();
    let item = rd.items().find_by_id(id).unwrap().unwrap();
    assert_eq!(item.current_stock, 0.0, "residue snapped to 0");
    assert!(!item.is_low_stock, "level 0 never flags");
    drop(rd);
    // A tiny negative residue never flags level 0 either (the SQL twin of the rule).
    let mut tx = db.begin_transaction().unwrap();
    tx.items().update_stock_cache(id, -1e-9).unwrap();
    tx.commit().unwrap();
    let mut rd = db.begin_read().unwrap();
    assert!(!rd.items().find_by_id(id).unwrap().unwrap().is_low_stock);
}

#[test]
fn l4_source_lot_total_is_stored_rounded() {
    let db = TestDb::new();
    let inv: Inv = InventoryServiceImpl::new(FifoStrategy, db.clone());
    seed_stocked_item(&db, &inv, 1, 0.0, 0.0, 10.0);
    let mut tx = db.begin_transaction().unwrap();
    let ids = inv.create_source_lots(&mut *tx, LotSource::Adjustment, DAY, None, &[(1, 3.0, Some(0.3335))]).unwrap();
    let lot = tx.purchases().find_purchase_item_by_id(ids[0]).unwrap().unwrap();
    assert_eq!(lot.total_amount, round2(3.0 * 0.3335));
    assert_eq!(lot.total_amount, 1.0);
}

#[test]
fn l5_a_failing_reindex_after_commit_never_fails_the_save() {
    let db = TestDb::new();
    let inv: Inv = InventoryServiceImpl::new(FifoStrategy, db.clone());
    let items = ItemServiceImpl::new(db.clone(), Arc::new(FailingIndexer(Arc::new(db.indexer()))), inv);
    let id = items.create_item(new_item("Rope", "PCS")).expect("create succeeds, the index is repaired later");
    items.update_item(id, edit_item("Rope 2", "PCS")).expect("update succeeds");
    let mut rd = db.begin_read().unwrap();
    assert_eq!(rd.items().find_by_id(id).unwrap().unwrap().name, "Rope 2");
}

#[test]
fn l6_base_unit_is_locked_once_the_item_has_stock_or_bills() {
    let db = TestDb::new();
    let inv: Inv = InventoryServiceImpl::new(FifoStrategy, db.clone());
    let items = ItemServiceImpl::new(db.clone(), Arc::new(db.indexer()), inv.clone());
    let fresh = items.create_item(new_item("Fresh", "PCS")).unwrap();
    items.update_item(fresh, edit_item("Fresh", "KG")).expect("a fresh item may change its unit");

    let mut with_stock = new_item("Stocked", "PCS");
    with_stock.initial_stock = Some(5.0);
    with_stock.initial_cost = Some(2.0);
    let stocked = items.create_item(with_stock).unwrap();
    assert_eq!(field_of(items.update_item(stocked, edit_item("Stocked", "KG"))), "base_unit");
    items.update_item(stocked, edit_item("Stocked renamed", "PCS")).expect("the same unit is fine");

    // A bill line also locks it, even when the stock is back to 0.
    let sold = items.create_item(new_item("Sold", "PCS")).unwrap();
    let purchases = PurchaseServiceImpl::new(inv.clone(), PaymentServiceImpl, db.clone());
    purchases.create_purchase(purchase_payload(None, PaymentStatus::Paid, 20.0, &[(sold, None, 10.0, 2.0)])).unwrap();
    let sales = SaleServiceImpl::new(inv, PaymentServiceImpl, db.clone());
    sales.create_sale(sale_payload(None, PaymentStatus::Paid, 100.0, &[(sold, None, 10.0, 10.0)])).unwrap();
    assert_eq!(field_of(items.update_item(sold, edit_item("Sold", "KG"))), "base_unit");
}

#[test]
fn l7_amounts_beyond_the_cap_are_refused_and_nothing_is_written() {
    let db = TestDb::new();
    let inv: Inv = InventoryServiceImpl::new(FifoStrategy, db.clone());
    seed_stocked_item(&db, &inv, 1, 10.0, 1.0, 10.0);
    let sales = SaleServiceImpl::new(inv.clone(), PaymentServiceImpl, db.clone());
    let purchases = PurchaseServiceImpl::new(inv, PaymentServiceImpl, db.clone());
    let before = ledger_row_count(&db);

    assert_eq!(field_of(sales.create_sale(sale_payload(None, PaymentStatus::Paid, 0.0, &[(1, None, 1e200, 1e200)]))), "unit_price");
    assert_eq!(field_of(sales.create_sale(sale_payload(None, PaymentStatus::Paid, 0.0, &[(1, None, 1e7, 1e7)]))), "unit_price");
    assert_eq!(field_of(sales.create_sale(sale_payload(None, PaymentStatus::Paid, 0.0, &[(1, None, 1.0, f64::INFINITY)]))), "unit_price");
    assert_eq!(field_of(purchases.create_purchase(purchase_payload(None, PaymentStatus::Paid, 0.0, &[(1, None, 1e200, 2.0)]))), "unit_cost");
    assert_eq!(field_of(purchases.create_purchase(purchase_payload(None, PaymentStatus::Paid, MAX_AMOUNT * 10.0, &[(1, None, 1.0, 2.0)]))), "paid_amount");
    assert_eq!(ledger_row_count(&db), before);
    let mut rd = db.begin_read().unwrap();
    assert_eq!(rd.items().find_by_id(1).unwrap().unwrap().current_stock, 10.0);
}

#[test]
fn l4_round2_vectors_match_the_shared_file() {
    let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/round2_vectors.json")).unwrap();
    let vectors: Vec<(f64, f64)> = serde_json::from_str(&text).unwrap();
    assert!(vectors.len() > 10);
    for (input, expected) in vectors {
        assert!((round2(input) - expected).abs() < 1e-9, "round2({input}) = {} expected {expected}", round2(input));
    }
}
