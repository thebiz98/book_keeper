// tests/item_import_service_test.rs
//! Test Suite: ItemImportService (FR-11 v1) on real SQLite.
//! Test Cases: TC-IMP-001 .. TC-IMP-004 plus CSV/XLSX paths, header rules, row numbers,
//! duplicates, barcode collisions and the three conflict strategies.
//!
//! XLSX coverage uses the small committed fixture `tests/fixtures/items_template_a.xlsx`
//! (no XLSX writer crate is available); every other case is generated as CSV at runtime.

mod common;

use std::sync::Arc;
use common::{FifoStrategy, InventoryServiceImpl, ItemImportServiceImpl, TestDb};
use book_keeper::core::dtos::*;
use book_keeper::core::error::AppError;
use book_keeper::data::contracts::unit_of_work::UnitOfWork;
use book_keeper::data::contracts::search_indexer::ItemSearchIndexer;
use book_keeper::services::contracts::item_import_service::ItemImportService;

const HEADER: &str = "Item Name,Category,Barcode,Base Unit,MRP,Sale Price,Purchase Cost,Opening Stock,Packaging Bundle,Units Per Bundle";

fn setup() -> (TestDb, ItemImportServiceImpl<InventoryServiceImpl<FifoStrategy>>) {
    let db = TestDb::new();
    let inv = InventoryServiceImpl::new(FifoStrategy, db.clone());
    let svc = ItemImportServiceImpl::new(db.clone(), inv).with_indexer(Arc::new(db.indexer()));
    (db, svc)
}

fn csv_file(body: &str) -> std::path::PathBuf {
    let p = std::env::temp_dir().join(format!("bk_import_{}.csv", uuid::Uuid::new_v4()));
    std::fs::write(&p, format!("{HEADER}\n{body}")).unwrap();
    p
}

fn raw_csv(content: &str) -> std::path::PathBuf {
    let p = std::env::temp_dir().join(format!("bk_import_{}.csv", uuid::Uuid::new_v4()));
    std::fs::write(&p, content).unwrap();
    p
}

fn s(p: &std::path::Path) -> &str {
    p.to_str().unwrap()
}

fn fixture_xlsx() -> String {
    format!("{}/tests/fixtures/items_template_a.xlsx", env!("CARGO_MANIFEST_DIR"))
}

fn count(db: &TestDb, sql: &str) -> i64 {
    db.pool.get().unwrap().query_row(sql, [], |r| r.get(0)).unwrap()
}

fn item(db: &TestDb, name: &str) -> book_keeper::core::entities::Item {
    let mut r = db.begin_read().unwrap();
    r.items().find_by_name(name).unwrap().unwrap()
}

#[test]
fn tc_imp_001_dry_run_reports_row_errors_and_writes_nothing() {
    let (db, svc) = setup();
    let f = csv_file(
        "Pen,Stat,,PCS,12,10,6,5,,\n\
         ,Stat,,PCS,12,10,6,5,,\n\
         Ink,Stat,,PCS,12,abc,6,5,,\n\
         pen,Stat,,PCS,12,10,6,5,,\n",
    );
    let rep = svc.validate_items_import(s(&f)).unwrap();
    assert_eq!(rep.valid_count, 1);
    let find = |row: usize, field: &str| rep.rows_with_errors.iter().any(|e| e.row == row && e.field == field);
    assert!(find(3, "Item Name"), "{:?}", rep.rows_with_errors);
    assert!(find(4, "Sale Price"));
    assert!(find(5, "Item Name"), "duplicate within file");
    assert_eq!(count(&db, "SELECT COUNT(*) FROM items"), 0);
    assert_eq!(count(&db, "SELECT COUNT(*) FROM purchase_transactions"), 0);
}

#[test]
fn tc_imp_002_execute_xlsx_creates_items_bundle_and_single_opn_header() {
    let (db, svc) = setup();
    let rep = svc.validate_items_import(&fixture_xlsx()).unwrap();
    assert_eq!((rep.valid_count, rep.rows_with_errors.len()), (3, 0), "{rep:?}");

    let sum = svc.execute_items_import(&fixture_xlsx(), ConflictStrategy::Skip).unwrap();
    assert_eq!(sum, ImportSummary { created: 3, updated: 0, skipped: 0, stock_lots_created: 2 });

    assert_eq!(count(&db, "SELECT COUNT(*) FROM items"), 3);
    assert_eq!(count(&db, "SELECT COUNT(*) FROM purchase_transactions WHERE source='OPENING'"), 1);
    assert_eq!(count(&db, "SELECT COUNT(*) FROM purchase_transactions WHERE source='OPENING' AND transaction_code LIKE 'OPN-%'"), 1);
    assert_eq!(count(&db, "SELECT COUNT(*) FROM purchase_items"), 2);
    let pen = item(&db, "Pen Blue");
    assert_eq!(pen.current_stock, 100.0);
    assert_eq!(pen.barcode.as_deref(), Some("8901234567890"));
    assert_eq!(pen.mrp, Some(12.0));
    assert_eq!(item(&db, "Rubber").current_stock, 0.0);
    let cost: f64 = db.pool.get().unwrap()
        .query_row("SELECT unit_cost FROM purchase_items WHERE item_id=?1", [pen.id], |r| r.get(0)).unwrap();
    assert_eq!(cost, 6.0);
    assert_eq!(count(&db, "SELECT COUNT(*) FROM item_bundles WHERE bundle_name='Box' AND units_per_bundle=10"), 1);
    // Cache invariant and FTS
    assert_eq!(count(&db, "SELECT COUNT(*) FROM items i WHERE ABS(i.current_stock - COALESCE((SELECT SUM(remaining_quantity) FROM purchase_items p WHERE p.item_id=i.id),0)) > 0.000001"), 0);
    let hits = db.indexer().search("Notebook").unwrap();
    assert_eq!(hits.len(), 1);
}

#[test]
fn tc_imp_003_conflict_strategies() {
    let file = "Pen,Office,BC1,PCS,20,15,7,10,,\n";
    let seed = |svc: &ItemImportServiceImpl<InventoryServiceImpl<FifoStrategy>>| {
        let f = csv_file("Pen,Old,,PCS,12,10,6,5,,\n");
        svc.execute_items_import(s(&f), ConflictStrategy::Skip).unwrap();
    };

    // SKIP: untouched
    let (db, svc) = setup();
    seed(&svc);
    let sum = svc.execute_items_import(s(&csv_file(file)), ConflictStrategy::Skip).unwrap();
    assert_eq!((sum.created, sum.updated, sum.skipped, sum.stock_lots_created), (0, 0, 1, 0));
    let p = item(&db, "Pen");
    assert_eq!((p.category.as_deref(), p.default_sale_price, p.current_stock), (Some("Old"), 10.0, 5.0));

    // dry run reports the duplicate (case-insensitive)
    let rep = svc.validate_items_import(s(&csv_file("PEN,Office,,PCS,20,15,7,10,,\n"))).unwrap();
    assert_eq!(rep.duplicates.len(), 1);
    assert_eq!(rep.duplicates[0].existing_item_id, p.id);
    assert_eq!(rep.duplicates[0].row, 2);

    // OVERWRITE: master fields only, stock untouched; barcode of the same item is no clash
    let sum = svc.execute_items_import(s(&csv_file(file)), ConflictStrategy::Overwrite).unwrap();
    assert_eq!((sum.updated, sum.stock_lots_created), (1, 0));
    let p = item(&db, "Pen");
    assert_eq!((p.category.as_deref(), p.barcode.as_deref(), p.mrp, p.default_sale_price, p.current_stock),
               (Some("Office"), Some("BC1"), Some(20.0), 15.0, 5.0));

    // ADD_STOCK: master untouched, new OPENING lot at Purchase Cost
    let sum = svc.execute_items_import(s(&csv_file("Pen,Zzz,,PCS,99,99,7,10,,\n")), ConflictStrategy::AddStock).unwrap();
    assert_eq!((sum.updated, sum.stock_lots_created), (1, 1));
    let p = item(&db, "Pen");
    assert_eq!((p.category.as_deref(), p.default_sale_price, p.current_stock), (Some("Office"), 15.0, 15.0));
    assert_eq!(count(&db, "SELECT COUNT(*) FROM purchase_items WHERE unit_cost=7 AND quantity=10"), 1);
    assert_eq!(count(&db, "SELECT COUNT(*) FROM purchase_transactions WHERE source='OPENING'"), 2);
}

#[test]
fn tc_imp_004_all_or_nothing() {
    let (db, svc) = setup();
    let f = csv_file("Good,,,PCS,,10,5,10,,\nBad,,,PCS,,-3,5,10,,\n");
    let err = svc.execute_items_import(s(&f), ConflictStrategy::Skip).unwrap_err();
    assert!(matches!(err, AppError::ValidationError { .. }), "{err:?}");
    assert_eq!(count(&db, "SELECT COUNT(*) FROM items"), 0);
    assert_eq!(count(&db, "SELECT COUNT(*) FROM purchase_transactions"), 0);
}

#[test]
fn header_rules() {
    let (_db, svc) = setup();
    let unknown = raw_csv("Item Name,Base Unit,Sale Price,Purchase Cost,Colour\nA,PCS,1,1,red\n");
    let e = svc.validate_items_import(s(&unknown)).unwrap_err();
    assert!(e.to_string().contains("Colour"), "{e}");
    let missing = raw_csv("Item Name,Base Unit,Sale Price\nA,PCS,1\n");
    let e = svc.validate_items_import(s(&missing)).unwrap_err();
    assert!(e.to_string().contains("Purchase Cost"), "{e}");
    // case-insensitive + trimmed + BOM accepted
    let ok = raw_csv("\u{feff} ITEM NAME , base unit,SALE PRICE,purchase cost\nA,PCS,1,1\n");
    assert_eq!(svc.validate_items_import(s(&ok)).unwrap().valid_count, 1);
    let bad_ext = std::env::temp_dir().join("x.txt");
    std::fs::write(&bad_ext, "a").unwrap();
    assert!(svc.validate_items_import(s(&bad_ext)).is_err());
}

#[test]
fn row_validation_rules() {
    let (_db, svc) = setup();
    let f = csv_file(
        "A,,,PCS,0,1,1,,,\n\
         B,,,PCS,,1,1,,Box,\n\
         C,,,PCS,,1,1,,,6\n\
         D,,,PCS,,1,1,,Box,1\n\
         E,,,,,1,1,,,\n\
         F,,,PCS,,inf,1,,,\n\
         G,,,PCS,,1,,,,\n\
         H,,,PCS,,1,1,-2,,\n\
         I,,,PCS,,1,1,1,Box,12\n",
    );
    let rep = svc.validate_items_import(s(&f)).unwrap();
    assert_eq!(rep.valid_count, 1);
    let at = |row: usize, field: &str| rep.rows_with_errors.iter().any(|e| e.row == row && e.field == field);
    assert!(at(2, "MRP") && at(3, "Units Per Bundle") && at(4, "Packaging Bundle") && at(5, "Units Per Bundle"));
    assert!(at(6, "Base Unit") && at(7, "Sale Price") && at(8, "Purchase Cost") && at(9, "Opening Stock"));
}

#[test]
fn barcode_collisions_vs_file_and_db() {
    let (db, svc) = setup();
    svc.execute_items_import(s(&csv_file("Existing,,BC9,PCS,,1,1,,,\n")), ConflictStrategy::Skip).unwrap();
    let f = csv_file("N1,,BC9,PCS,,1,1,,,\nN2,,BC5,PCS,,1,1,,,\nN3,,BC5,PCS,,1,1,,,\n");
    let rep = svc.validate_items_import(s(&f)).unwrap();
    assert_eq!(rep.valid_count, 1);
    assert!(rep.rows_with_errors.iter().any(|e| e.row == 2 && e.field == "Barcode"));
    assert!(rep.rows_with_errors.iter().any(|e| e.row == 4 && e.field == "Barcode"));
    assert!(svc.execute_items_import(s(&f), ConflictStrategy::Skip).is_err());
    assert_eq!(count(&db, "SELECT COUNT(*) FROM items"), 1);
}
