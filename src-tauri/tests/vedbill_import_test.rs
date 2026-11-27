// tests/vedbill_import_test.rs
//! Test Suite: VedBill backup -> inventory migration on real SQLite.
//! Synthetic backups are built at runtime (tests/common/vedbill_fixture.rs).
//! The real-backup check is opt-in:
//!   VEDBILL_BACKUP=<path to .vb> cargo test --test vedbill_import_test -- --ignored

mod common;

use std::sync::Arc;
use common::vedbill_fixture::{build_vb, product, VbItem};
use common::{FifoStrategy, InventoryServiceImpl, TestDb, VedbillImportServiceImpl};
use book_keeper::core::dtos::*;
use book_keeper::data::contracts::unit_of_work::UnitOfWork;
use book_keeper::data::contracts::search_indexer::ItemSearchIndexer;
use book_keeper::data::vedbill::ZipVedbillReader;
use book_keeper::services::contracts::vedbill_import_service::VedbillImportService;

type Svc = VedbillImportServiceImpl<InventoryServiceImpl<FifoStrategy>>;

fn setup() -> (TestDb, Svc) {
    let db = TestDb::new();
    let inv = InventoryServiceImpl::new(FifoStrategy, db.clone());
    let svc = Svc::new(db.clone(), inv, ZipVedbillReader).with_indexer(Arc::new(db.indexer()));
    (db, svc)
}

fn count(db: &TestDb, sql: &str) -> i64 {
    db.pool.get().unwrap().query_row(sql, [], |r| r.get(0)).unwrap()
}

fn item(db: &TestDb, name: &str) -> book_keeper::core::entities::Item {
    let mut r = db.begin_read().unwrap();
    r.items().find_by_name(name).unwrap().unwrap_or_else(|| panic!("item {name} missing"))
}

/// (transaction_date, quantity, unit_cost) of an item's lots in FIFO (id) order.
fn lots(db: &TestDb, item_id: i64) -> Vec<(String, f64, f64)> {
    let c = db.pool.get().unwrap();
    let mut st = c
        .prepare(
            "SELECT pt.transaction_date, pi.quantity, pi.unit_cost FROM purchase_items pi
             JOIN purchase_transactions pt ON pt.id = pi.purchase_transaction_id
             WHERE pi.item_id = ?1 AND pt.source = 'OPENING' ORDER BY pi.id",
        )
        .unwrap();
    st.query_map([item_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).unwrap().map(Result::unwrap).collect()
}

fn sample() -> std::path::PathBuf {
    let service = VbItem { item_type: "SERVICE", ..product("s", "BILTY CHARGE", 0.0, 0.0) };
    build_vb(
        &[
            product("g", "WHITE GLUE 20 ", 29.0, 13.0),
            product("d", "DOMS TULI NO 20/-", 14.0, 0.0),
            product("a", " LOOSE A4 SHEET", -106.0, 0.37),
            service,
            product("z", "ZERO STOCK", 0.0, 4.0),
        ],
        &[
            // out of order on purpose: the reader sorts by date
            ("g", "2026-11-12", 13.0, 30.0),
            ("g", "2026-10-28", 13.5, 2.0),
            ("d", "2026-11-21T11:34:44.832Z", 0.0, -1.0),
            ("a", "2026-11-19", 0.37, -14.0),
            ("a", "2026-11-19", 0.37, 0.0),
        ],
        8,
    )
}

#[test]
fn dry_run_reports_counts_and_warnings_without_writing() {
    let (db, svc) = setup();
    let rep = svc.validate_vedbill_import(sample().to_str().unwrap()).unwrap();
    assert_eq!(rep.valid_count, 5, "{rep:?}");
    assert!(rep.rows_with_errors.is_empty() && rep.header_issues.is_empty());
    assert_eq!(rep.lots_to_create, 3, "2 glue lots + 1 fallback lot");
    let has = |name: &str, needle: &str| rep.warnings.iter().any(|w| w.name == name && w.message.contains(needle));
    assert!(has("WHITE GLUE 20", "Name cleaned"));
    assert!(has("WHITE GLUE 20", "lots add up to 32"));
    assert!(has("DOMS TULI NO 20/-", "No stock lots"));
    assert!(has("LOOSE A4 SHEET", "Imported with stock 0"));
    assert!(has("BILTY CHARGE", "Service"));
    assert_eq!(count(&db, "SELECT COUNT(*) FROM items"), 0);
}

#[test]
fn execute_copies_lots_one_to_one_in_fifo_order() {
    let (db, svc) = setup();
    let sum = svc.execute_vedbill_import(sample().to_str().unwrap(), ConflictStrategy::Skip).unwrap();
    assert_eq!(sum, ImportSummary { created: 5, updated: 0, skipped: 0, stock_lots_created: 3 });

    let glue = item(&db, "WHITE GLUE 20");
    assert_eq!(glue.current_stock, 32.0);
    assert_eq!(glue.base_unit, "PCS");
    assert_eq!(glue.mrp, Some(12.0));
    assert_eq!(
        lots(&db, glue.id),
        vec![("2026-10-28".to_string(), 2.0, 13.5), ("2026-11-12".to_string(), 30.0, 13.0)]
    );
    assert_eq!(item(&db, "DOMS TULI NO 20/-").current_stock, 14.0);
    assert_eq!(item(&db, "LOOSE A4 SHEET").current_stock, 0.0);
    assert_eq!(item(&db, "BILTY CHARGE").current_stock, 0.0);
    assert_eq!(item(&db, "ZERO STOCK").current_stock, 0.0);

    // One OPENING header per lot date.
    assert_eq!(count(&db, "SELECT COUNT(*) FROM purchase_transactions WHERE source='OPENING'"), 3);
    // Stock cache invariant and FTS.
    assert_eq!(count(&db, "SELECT COUNT(*) FROM items i WHERE ABS(i.current_stock - COALESCE((SELECT SUM(remaining_quantity) FROM purchase_items p WHERE p.item_id=i.id),0)) > 0.000001"), 0);
    assert_eq!(db.indexer().search("GLUE").unwrap().len(), 1);
}

#[test]
fn rerun_with_skip_is_idempotent_and_add_stock_adds_lots() {
    let (db, svc) = setup();
    let vb = sample();
    svc.execute_vedbill_import(vb.to_str().unwrap(), ConflictStrategy::Skip).unwrap();
    let rep = svc.validate_vedbill_import(vb.to_str().unwrap()).unwrap();
    assert_eq!(rep.duplicates.len(), 5);
    assert_eq!(rep.lots_to_create, 0);
    let sum = svc.execute_vedbill_import(vb.to_str().unwrap(), ConflictStrategy::Skip).unwrap();
    assert_eq!((sum.created, sum.skipped, sum.stock_lots_created), (0, 5, 0));
    assert_eq!(count(&db, "SELECT COUNT(*) FROM items"), 5);

    let sum = svc.execute_vedbill_import(vb.to_str().unwrap(), ConflictStrategy::AddStock).unwrap();
    assert_eq!((sum.updated, sum.stock_lots_created), (2, 3));
    assert_eq!(item(&db, "WHITE GLUE 20").current_stock, 64.0);
}

#[test]
fn errors_roll_back_everything() {
    let (db, svc) = setup();
    let vb = build_vb(&[product("1", "Pen", 5.0, 1.0), product("2", " pen ", 5.0, 1.0)], &[], 8);
    let rep = svc.validate_vedbill_import(vb.to_str().unwrap()).unwrap();
    assert_eq!(rep.rows_with_errors.len(), 1);
    assert!(svc.execute_vedbill_import(vb.to_str().unwrap(), ConflictStrategy::Skip).is_err());
    assert_eq!(count(&db, "SELECT COUNT(*) FROM items"), 0);
    assert_eq!(count(&db, "SELECT COUNT(*) FROM purchase_transactions"), 0);
}

#[test]
fn bad_files_get_clear_messages() {
    let (_db, svc) = setup();
    let not_zip = common::temp_file("notzip", "vb");
    std::fs::write(&not_zip, b"hello").unwrap();
    let e = svc.validate_vedbill_import(not_zip.to_str().unwrap()).unwrap_err();
    assert!(e.to_string().contains("not a VedBill backup"), "{e}");

    let e = svc.validate_vedbill_import("C:/nope/backup.xlsx").unwrap_err();
    assert!(e.to_string().contains(".vb"), "{e}");

    let newer = build_vb(&[product("1", "Pen", 0.0, 1.0)], &[], 9);
    let rep = svc.validate_vedbill_import(newer.to_str().unwrap()).unwrap();
    assert!(rep.warnings.iter().any(|w| w.row == 0 && w.message.contains("schema version 9")));
}

/// Opt-in check against a real backup (not committed). Expectations match the
/// 2026-11-22 backup inspected while building this feature.
#[test]
#[ignore]
fn real_backup_from_env() {
    let Ok(path) = std::env::var("VEDBILL_BACKUP") else {
        eprintln!("VEDBILL_BACKUP not set; skipping");
        return;
    };
    let (db, svc) = setup();
    let rep = svc.validate_vedbill_import(&path).unwrap();
    eprintln!(
        "valid={} errors={} warnings={} lots={}",
        rep.valid_count,
        rep.rows_with_errors.len(),
        rep.warnings.len(),
        rep.lots_to_create
    );
    for w in &rep.warnings {
        eprintln!("  row {} {}: {}", w.row, w.name, w.message);
    }
    assert!(rep.rows_with_errors.is_empty(), "{:?}", rep.rows_with_errors);
    let sum = svc.execute_vedbill_import(&path, ConflictStrategy::Skip).unwrap();
    eprintln!("{sum:?}");
    assert_eq!(sum.created, rep.valid_count);
    if let Ok(true) = std::env::var("VEDBILL_EXPECT_20261122").map(|v| v == "1") {
        assert_eq!(sum.created, 201);
        let glue = item(&db, "WHITE GLUE 20");
        assert_eq!(glue.current_stock, 32.0);
        assert_eq!(lots(&db, glue.id).len(), 2);
        assert_eq!(item(&db, "LOOSE A4 SHEET").current_stock, 0.0);
        assert_eq!(item(&db, "DOMS TULI NO 20/-").current_stock, 14.0);
    }
    assert_eq!(count(&db, "SELECT COUNT(*) FROM items i WHERE ABS(i.current_stock - COALESCE((SELECT SUM(remaining_quantity) FROM purchase_items p WHERE p.item_id=i.id),0)) > 0.000001"), 0);
}

#[test]
fn vedbill_services_import_as_service_items_without_stock_or_components() {
    use book_keeper::core::value_objects::ItemType;
    let (db, svc) = setup();
    svc.execute_vedbill_import(sample().to_str().unwrap(), ConflictStrategy::Skip).unwrap();
    let bilty = item(&db, "BILTY CHARGE");
    assert_eq!(bilty.item_type, ItemType::Service);
    assert_eq!((bilty.current_stock, bilty.is_low_stock, bilty.service_cost), (0.0, false, None));
    assert_eq!(item(&db, "WHITE GLUE 20").item_type, ItemType::Physical);
    assert_eq!(count(&db, "SELECT COUNT(*) FROM service_components"), 0);
    assert_eq!(count(&db, "SELECT COUNT(*) FROM purchase_items WHERE item_id = (SELECT id FROM items WHERE name = 'BILTY CHARGE')"), 0);
}
