// tests/schema_test.rs
//! Schema foundation (F1/T1): migration runner, connection pragmas, CHECK/UNIQUE enforcement,
//! enum TEXT literals (DD-23) and DB path resolution (DD-20).

mod common;

use std::path::{Path, PathBuf};

use book_keeper::app_state::{resolve_db_path, DEFAULT_DB_FILE};
use book_keeper::core::error::AppError;
use book_keeper::core::value_objects::*;
use book_keeper::data::sqlite::connection::create_pool;
use book_keeper::data::sqlite::migrations::{current_version, latest_version, run_migrations};
use book_keeper::data::sqlite::{enum_from_sql, enum_to_sql};
use rusqlite::Connection;

/// Each database lives in its own unique folder, so the `backups/` folder written next to it
/// by the pre-migration backup (DD-29) is private to the test.
fn temp_path(tag: &str) -> PathBuf {
    common::sweep_stale_temp();
    let dir = std::env::temp_dir().join(format!("bk_schema_{tag}_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    dir.join("shop.db")
}

fn cleanup(path: &Path) {
    for ext in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{}", path.display(), ext));
    }
    if let Some(dir) = path.parent() {
        if dir.file_name().is_some_and(|n| n.to_string_lossy().starts_with("bk_schema_")) {
            let _ = std::fs::remove_dir_all(dir);
        }
    }
}

/// Fresh migrated database, returned with its pool.
fn fresh() -> (PathBuf, book_keeper::data::sqlite::connection::SqlitePool) {
    let path = temp_path("fresh");
    let pool = create_pool(&path).expect("pool");
    run_migrations(&pool.get().expect("conn")).expect("migrate");
    (path, pool)
}

// ---------------------------------------------------------------- runner

#[test]
fn schema_version_created_on_fresh_db_and_runner_is_idempotent() {
    let (path, pool) = fresh();
    let conn = pool.get().unwrap();
    assert_eq!(current_version(&conn).unwrap(), latest_version());
    let uv: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap();
    assert_eq!(uv, latest_version());
    let rows: i64 = conn.query_row("SELECT COUNT(*) FROM schema_version", [], |r| r.get(0)).unwrap();
    assert_eq!(rows, latest_version());
    let name: String = conn
        .query_row("SELECT name FROM schema_version WHERE version = 1", [], |r| r.get(0))
        .unwrap();
    assert_eq!(name, "aligned_schema");

    // Second run on the same connection and on a re-opened pool: no-op, no duplicate rows.
    run_migrations(&conn).expect("second run");
    drop(conn);
    drop(pool);
    let pool2 = create_pool(&path).unwrap();
    run_migrations(&pool2.get().unwrap()).expect("reopen run");
    let rows: i64 = pool2
        .get()
        .unwrap()
        .query_row("SELECT COUNT(*) FROM schema_version", [], |r| r.get(0))
        .unwrap();
    assert_eq!(rows, latest_version());
    // Single seeded settings row survives re-runs.
    let settings: i64 = pool2.get().unwrap().query_row("SELECT COUNT(*) FROM shop_settings", [], |r| r.get(0)).unwrap();
    assert_eq!(settings, 1);
    drop(pool2);
    cleanup(&path);
}

#[test]
fn migration_leaves_foreign_keys_on_for_the_migrating_connection() {
    let (path, pool) = fresh();
    let conn = pool.get().unwrap();
    let fk: i64 = conn.query_row("PRAGMA foreign_keys", [], |r| r.get(0)).unwrap();
    assert_eq!(fk, 1);
    drop(conn);
    drop(pool);
    cleanup(&path);
}

#[test]
fn legacy_pre_alignment_db_is_rejected_and_untouched() {
    let path = temp_path("legacy");
    {
        // Mimics the old v1 layout: user_version = 1, payment_settlements without `kind`.
        let c = Connection::open(&path).unwrap();
        c.execute_batch(
            "CREATE TABLE payment_settlements (id INTEGER PRIMARY KEY, transaction_type TEXT NOT NULL, \
             transaction_id INTEGER NOT NULL, amount_paid REAL NOT NULL);
             PRAGMA user_version = 1;",
        )
        .unwrap();
    }
    let pool = create_pool(&path).unwrap();
    let err = run_migrations(&pool.get().unwrap()).expect_err("legacy must be rejected");
    assert!(matches!(err, AppError::MigrationError(_)), "got {err:?}");
    if let AppError::MigrationError(msg) = err {
        assert!(msg.contains("pre-release"), "message should explain the legacy DB: {msg}");
    }
    // Untouched: no schema_version table was created, no `kind` column appeared.
    let conn = pool.get().unwrap();
    let n: i64 = conn
        .query_row("SELECT COUNT(*) FROM sqlite_master WHERE name = 'schema_version'", [], |r| r.get(0))
        .unwrap();
    assert_eq!(n, 0);
    let cols: i64 = conn
        .query_row("SELECT COUNT(*) FROM pragma_table_info('payment_settlements') WHERE name = 'kind'", [], |r| r.get(0))
        .unwrap();
    assert_eq!(cols, 0);
    drop(conn);
    drop(pool);
    cleanup(&path);
}

#[test]
fn database_newer_than_the_code_is_refused() {
    let (path, pool) = fresh();
    {
        let conn = pool.get().unwrap();
        conn.execute("INSERT INTO schema_version (version, name) VALUES (?1, 'from_the_future')", [latest_version() + 1])
            .unwrap();
    }
    let err = run_migrations(&pool.get().unwrap()).expect_err("newer DB must be refused");
    assert!(matches!(err, AppError::MigrationError(ref m) if m.contains("newer")), "got {err:?}");
    drop(pool);
    cleanup(&path);
}

// ---------------------------------------------------------------- connection

#[test]
fn every_pooled_connection_has_fk_wal_and_pragmas() {
    let (path, pool) = fresh();
    // Hold several connections at once so the pool hands out distinct ones.
    let conns: Vec<_> = (0..3).map(|_| pool.get().unwrap()).collect();
    for c in &conns {
        let fk: i64 = c.query_row("PRAGMA foreign_keys", [], |r| r.get(0)).unwrap();
        let jm: String = c.query_row("PRAGMA journal_mode", [], |r| r.get(0)).unwrap();
        let sync: i64 = c.query_row("PRAGMA synchronous", [], |r| r.get(0)).unwrap();
        let busy: i64 = c.query_row("PRAGMA busy_timeout", [], |r| r.get(0)).unwrap();
        let temp: i64 = c.query_row("PRAGMA temp_store", [], |r| r.get(0)).unwrap();
        assert_eq!(fk, 1, "foreign_keys must be ON");
        assert_eq!(jm.to_lowercase(), "wal");
        assert_eq!(sync, 1, "synchronous NORMAL");
        assert_eq!(busy, 5000);
        assert_eq!(temp, 2, "temp_store MEMORY");
    }
    // FK enforcement is actually live: a dangling FK insert is rejected.
    let err = conns[0].execute(
        "INSERT INTO sale_items (sale_transaction_id, item_id, item_name, pack_quantity, quantity, unit_price, total_amount) \
         VALUES (999, 999, 'x', 1, 1, 1, 1)",
        [],
    );
    assert!(err.is_err());
    drop(conns);
    drop(pool);
    cleanup(&path);
}

// ---------------------------------------------------------------- CHECK / UNIQUE

fn insert_item(c: &Connection, name: &str) -> rusqlite::Result<usize> {
    c.execute("INSERT INTO items (name) VALUES (?1)", [name])
}

#[test]
fn zero_or_negative_ledger_amount_is_rejected() {
    let (path, pool) = fresh();
    let c = pool.get().unwrap();
    for amount in [0.0_f64, -5.0] {
        let r = c.execute(
            "INSERT INTO payment_settlements (settlement_date, kind, transaction_type, transaction_id, amount_paid) \
             VALUES ('2026-11-21', 'SETTLEMENT', 'SALE', 1, ?1)",
            [amount],
        );
        assert!(r.is_err(), "amount_paid {amount} must violate CHECK (amount_paid > 0)");
    }
    drop(c);
    drop(pool);
    cleanup(&path);
}

#[test]
fn ledger_requires_transaction_unless_refund() {
    let (path, pool) = fresh();
    let c = pool.get().unwrap();
    let no_tx = |kind: &str| {
        c.execute(
            "INSERT INTO payment_settlements (settlement_date, kind, transaction_type, amount_paid) \
             VALUES ('2026-11-21', ?1, 'SALE', 10)",
            [kind],
        )
    };
    assert!(no_tx("UPFRONT").is_err());
    assert!(no_tx("SETTLEMENT").is_err());
    assert!(no_tx("RETURN_CREDIT").is_err());
    assert!(no_tx("REFUND_OUT").is_ok(), "walk-in refunds carry no bill");
    assert!(no_tx("REFUND_IN").is_ok());
    assert!(no_tx("BOGUS").is_err(), "unknown kind rejected");
    drop(c);
    drop(pool);
    cleanup(&path);
}

#[test]
fn bundle_multiplier_must_exceed_one_and_names_are_unique_per_item() {
    let (path, pool) = fresh();
    let c = pool.get().unwrap();
    insert_item(&c, "Pen").unwrap();
    let ins = |name: &str, units: f64| {
        c.execute(
            "INSERT INTO item_bundles (item_id, bundle_name, units_per_bundle) VALUES (1, ?1, ?2)",
            rusqlite::params![name, units],
        )
    };
    assert!(ins("Single", 1.0).is_err(), "units_per_bundle = 1 rejected");
    assert!(ins("Neg", 0.0).is_err());
    assert!(ins("Box of 10", 10.0).is_ok());
    assert!(ins("Box of 10", 12.0).is_err(), "UNIQUE(item_id, bundle_name)");
    drop(c);
    drop(pool);
    cleanup(&path);
}

#[test]
fn item_name_is_unique_case_insensitively() {
    let (path, pool) = fresh();
    let c = pool.get().unwrap();
    insert_item(&c, "Apsara Pencil").unwrap();
    assert!(insert_item(&c, "APSARA PENCIL").is_err());
    assert!(insert_item(&c, "apsara pencil").is_err());
    assert!(insert_item(&c, "Apsara Pencil 2").is_ok());
    drop(c);
    drop(pool);
    cleanup(&path);
}

#[test]
fn lot_return_adjustment_and_enum_checks() {
    let (path, pool) = fresh();
    let c = pool.get().unwrap();
    insert_item(&c, "Pen").unwrap();
    c.execute(
        "INSERT INTO purchase_transactions (transaction_code, transaction_date, payment_status, total_amount) \
         VALUES ('PUR-1', '2026-11-21', 'PAID', 0)",
        [],
    )
    .unwrap();
    let lot = |qty: f64, rem: f64| {
        c.execute(
            "INSERT INTO purchase_items (purchase_transaction_id, item_id, item_name, pack_quantity, quantity, unit_cost, total_amount, remaining_quantity) \
             VALUES (1, 1, 'Pen', ?1, ?1, 1, 1, ?2)",
            rusqlite::params![qty, rem],
        )
    };
    assert!(lot(10.0, 11.0).is_err(), "remaining > quantity rejected");
    assert!(lot(10.0, -1.0).is_err(), "negative remaining rejected");
    assert!(lot(10.0, 10.0).is_ok());

    // quantity <> 0 and reason enum on adjustments
    let adj = |qty: f64, reason: &str| {
        c.execute(
            "INSERT INTO stock_adjustments (adjustment_date, item_id, quantity, reason) VALUES ('2026-11-21', 1, ?1, ?2)",
            rusqlite::params![qty, reason],
        )
    };
    assert!(adj(0.0, "DAMAGED").is_err());
    assert!(adj(-1.0, "DamageWriteOff").is_err(), "legacy PascalCase reason rejected");
    assert!(adj(-1.0, "DAMAGED").is_ok());
    assert!(adj(2.0, "UNRECORDED_STOCK").is_err(), "needs unit_cost");
    let adj_cost = |qty: f64, reason: &str| {
        c.execute(
            "INSERT INTO stock_adjustments (adjustment_date, item_id, quantity, unit_cost, reason) VALUES ('2026-11-21', 1, ?1, 3, ?2)",
            rusqlite::params![qty, reason],
        )
    };
    assert!(adj_cost(2.0, "UNRECORDED_STOCK").is_ok());
    assert!(adj_cost(-2.0, "UNRECORDED_STOCK").is_err(), "positive only");

    // sale allocation returned_quantity bounds
    c.execute(
        "INSERT INTO sale_transactions (transaction_code, transaction_date, payment_status, total_amount) VALUES ('SAL-1','2026-11-21','PAID',1)",
        [],
    )
    .unwrap();
    c.execute(
        "INSERT INTO sale_items (sale_transaction_id, item_id, item_name, pack_quantity, quantity, unit_price, total_amount) VALUES (1,1,'Pen',1,1,1,1)",
        [],
    )
    .unwrap();
    let alloc = |ret: f64| {
        c.execute(
            "INSERT INTO sale_stock_allocations (sale_item_id, purchase_item_id, quantity_depleted, returned_quantity, unit_cost) VALUES (1,1,2,?1,1)",
            [ret],
        )
    };
    assert!(alloc(3.0).is_err(), "returned_quantity > quantity_depleted rejected");
    assert!(alloc(-1.0).is_err());
    assert!(alloc(2.0).is_ok());

    // vendor replacement is forbidden
    let ret = |t: &str, s: &str, code: &str| {
        c.execute(
            "INSERT INTO return_transactions (return_code, return_date, return_type, settlement_type, total_amount) VALUES (?1,'2026-11-21',?2,?3,0)",
            rusqlite::params![code, t, s],
        )
    };
    assert!(ret("VENDOR", "REPLACEMENT", "R1").is_err());
    assert!(ret("CUSTOMER", "REPLACEMENT", "R2").is_ok());
    assert!(ret("VENDOR", "REFUND", "R3").is_ok());
    assert!(ret("CustomerReturn", "REFUND", "R4").is_err(), "legacy return_type rejected");

    // outflow source enum
    let out = |st: &str| {
        c.execute(
            "INSERT INTO stock_outflow_allocations (source_type, source_id, purchase_item_id, quantity_depleted, unit_cost) VALUES (?1,1,1,1,1)",
            [st],
        )
    };
    assert!(out("ADJUSTMENT").is_ok());
    assert!(out("SALE").is_err());
    drop(c);
    drop(pool);
    cleanup(&path);
}

#[test]
fn legacy_adjustment_allocation_table_is_gone_and_bills_have_no_bundle_column() {
    let (path, pool) = fresh();
    let c = pool.get().unwrap();
    let has_table = |t: &str| -> i64 {
        c.query_row("SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name = ?1", [t], |r| r.get(0)).unwrap()
    };
    assert_eq!(has_table("adjustment_stock_allocations"), 0);
    assert_eq!(has_table("stock_outflow_allocations"), 1);
    assert_eq!(has_table("item_search_fts"), 1);
    for t in ["sale_items", "purchase_items"] {
        let n: i64 = c
            .query_row(
                &format!("SELECT COUNT(*) FROM pragma_table_info('{t}') WHERE name IN ('bundle_id','bundle_quantity')"),
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 0, "{t} must not reference bundles (DD-28)");
    }
    drop(c);
    drop(pool);
    cleanup(&path);
}

// ---------------------------------------------------------------- enums (DD-23)

fn lit<T: serde::Serialize>(v: T) -> String {
    enum_to_sql(&v).unwrap()
}

#[test]
fn enum_literals_are_screaming_snake_case_and_round_trip() {
    assert_eq!(lit(PartyType::Customer), "CUSTOMER");
    assert_eq!(lit(PaymentStatus::Partial), "PARTIAL");
    assert_eq!(lit(StockCondition::DamagedScrap), "DAMAGED_SCRAP");
    assert_eq!(lit(AdjustmentReason::CountCorrection), "COUNT_CORRECTION");
    assert_eq!(lit(AdjustmentReason::TheftLost), "THEFT_LOST");
    assert_eq!(lit(AdjustmentReason::UnrecordedStock), "UNRECORDED_STOCK");
    assert_eq!(lit(SettlementType::Replacement), "REPLACEMENT");
    assert_eq!(lit(ReturnType::Vendor), "VENDOR");
    assert_eq!(lit(PaymentKind::ReturnCredit), "RETURN_CREDIT");
    assert_eq!(lit(PaymentKind::RefundOut), "REFUND_OUT");
    assert_eq!(lit(TransactionType::Purchase), "PURCHASE");
    assert_eq!(lit(LotSource::Opening), "OPENING");
    assert_eq!(lit(OutflowSourceType::VendorReturn), "VENDOR_RETURN");
    assert_eq!(lit(ReturnLineType::ReplacementIssued), "REPLACEMENT_ISSUED");
    assert_eq!(lit(AllocationMethod::Fifo), "FIFO");
    assert_eq!(lit(ItemType::Physical), "PHYSICAL");
    assert_eq!(lit(ItemType::Service), "SERVICE");
    assert!(enum_from_sql::<ItemType>("Physical").is_err());

    assert_eq!(enum_from_sql::<PaymentKind>("REFUND_IN").unwrap(), PaymentKind::RefundIn);
    assert_eq!(enum_from_sql::<StockCondition>("RESTOCKABLE").unwrap(), StockCondition::Restockable);
    assert_eq!(enum_from_sql::<OutflowSourceType>("ADJUSTMENT").unwrap(), OutflowSourceType::Adjustment);

    // Legacy / unknown literals are errors, never silently mapped.
    assert!(enum_from_sql::<PaymentStatus>("Paid").is_err());
    assert!(enum_from_sql::<SettlementType>("CashRefund").is_err());
    assert!(enum_from_sql::<AdjustmentReason>("DamageWriteOff").is_err());
    assert!(enum_from_sql::<ReturnType>("CustomerReturn").is_err());
}

#[test]
fn ipc_json_uses_the_same_literals() {
    assert_eq!(serde_json::to_string(&PaymentKind::RefundOut).unwrap(), "\"REFUND_OUT\"");
    let back: ReturnLineType = serde_json::from_str("\"REPLACEMENT_ISSUED\"").unwrap();
    assert_eq!(back, ReturnLineType::ReplacementIssued);
}

// ---------------------------------------------------------------- DB path (DD-20)

#[test]
fn db_path_defaults_to_shop_db_and_env_override_wins() {
    let dir = Path::new("/data/app");
    assert_eq!(resolve_db_path(dir, None), dir.join(DEFAULT_DB_FILE));
    assert_eq!(DEFAULT_DB_FILE, "shop.db");
    assert_eq!(resolve_db_path(dir, Some("")), dir.join("shop.db"), "blank override ignored");
    assert_eq!(resolve_db_path(dir, Some("   ")), dir.join("shop.db"));
    assert_eq!(resolve_db_path(dir, Some("/tmp/x/custom.db")), PathBuf::from("/tmp/x/custom.db"));
}

// ---------------------------------------------------------------- frozen fixtures (DD-29)

/// Exact DDL delta of each shipped migration (v1 = the whole aligned schema). Never edited.
const V1: &str = include_str!("fixtures/schema/v1.sql");
const V2: &str = include_str!("fixtures/schema/v2.sql");
const V3: &str = include_str!("fixtures/schema/v3.sql");
const V4: &str = include_str!("fixtures/schema/v4.sql");
const V5: &str = include_str!("fixtures/schema/v5.sql");
#[allow(dead_code)]
const V6: &str = include_str!("fixtures/schema/v6.sql");
#[allow(dead_code)]
const V7: &str = include_str!("fixtures/schema/v7.sql");
#[allow(dead_code)]
const V8: &str = include_str!("fixtures/schema/v8.sql");
#[allow(dead_code)]
const V9: &str = include_str!("fixtures/schema/v9.sql");
const V10: &str = include_str!("fixtures/schema/v10.sql");
const V11: &str = include_str!("fixtures/schema/v11.sql");
const V12: &str = include_str!("fixtures/schema/v12.sql");
/// The shipped v8 fixture is a DDL delta without its `schema_version` row (never edited); chains that
/// go past v8 record it with this step.
const V8_RECORD: &str = "INSERT INTO schema_version (version, name) VALUES (8, 'bill_correction'); PRAGMA user_version = 8;";

/// Builds a database at the version reached by replaying `steps` (fixtures 1..N).
fn db_from_fixtures(tag: &str, steps: &[&str]) -> (PathBuf, Connection) {
    let path = temp_path(tag);
    let conn = Connection::open(&path).expect("open");
    for step in steps {
        conn.execute_batch(step).expect("fixture");
    }
    (path, conn)
}

#[test]
fn fresh_db_has_nullable_backup_directory_column() {
    let (path, pool) = fresh();
    let conn = pool.get().unwrap();
    let notnull: i64 = conn
        .query_row("SELECT \"notnull\" FROM pragma_table_info('shop_settings') WHERE name = 'backup_directory'", [], |r| r.get(0))
        .unwrap();
    assert_eq!(notnull, 0);
    drop(conn);
    drop(pool);
    cleanup(&path);
}

#[test]
fn v1_database_upgrades_to_add_backup_directory() {
    let (path, conn) = db_from_fixtures("v1up", &[V1]);
    run_migrations(&conn).expect("upgrade");
    assert_eq!(current_version(&conn).unwrap(), latest_version());
    let v: Option<String> = conn.query_row("SELECT backup_directory FROM shop_settings WHERE id = 1", [], |r| r.get(0)).unwrap();
    assert!(v.is_none());
    let cols: i64 = conn
        .query_row("SELECT COUNT(*) FROM pragma_table_info('items') WHERE name = 'item_type'", [], |r| r.get(0))
        .unwrap();
    assert_eq!(cols, 1);
    drop(conn);
    cleanup(&path);
}

#[test]
fn fresh_db_items_have_item_type_physical_default() {
    let (path, pool) = fresh();
    let conn = pool.get().unwrap();
    assert!(latest_version() >= 3);
    let (notnull, dflt): (i64, String) = conn
        .query_row(
            "SELECT \"notnull\", dflt_value FROM pragma_table_info('items') WHERE name = 'item_type'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(notnull, 1);
    assert_eq!(dflt, "'PHYSICAL'");
    conn.execute("INSERT INTO items (name) VALUES ('A')", []).unwrap();
    let t: String = conn.query_row("SELECT item_type FROM items WHERE name = 'A'", [], |r| r.get(0)).unwrap();
    assert_eq!(t, "PHYSICAL");
    conn.execute("INSERT INTO items (name, item_type) VALUES ('B', 'SERVICE')", []).unwrap();
    let bad = conn.execute("INSERT INTO items (name, item_type) VALUES ('C', 'GOODS')", []);
    assert!(bad.is_err(), "CHECK must reject unknown item types");
    drop(conn);
    drop(pool);
    cleanup(&path);
}

#[test]
fn v2_database_upgrades_to_v3_and_existing_items_become_physical() {
    let (path, conn) = db_from_fixtures("v2up", &[V1, V2]);
    conn.execute_batch(
        "INSERT INTO items (name, category, base_unit, default_sale_price, current_stock) VALUES ('Pen','Pens','PCS',10,7),('Paper',NULL,'SHEET',1,0);
         UPDATE shop_settings SET backup_directory = 'D:\\bk';",
    )
    .unwrap();
    run_migrations(&conn).expect("upgrade");
    assert!(current_version(&conn).unwrap() >= 3);
    let types: Vec<String> = conn
        .prepare("SELECT item_type FROM items ORDER BY id")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert_eq!(types, vec!["PHYSICAL", "PHYSICAL"]);
    let stock: Vec<f64> = conn
        .prepare("SELECT current_stock FROM items ORDER BY id")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert_eq!(stock, vec![7.0, 0.0]);
    let dir: Option<String> = conn.query_row("SELECT backup_directory FROM shop_settings WHERE id = 1", [], |r| r.get(0)).unwrap();
    assert_eq!(dir.as_deref(), Some("D:\\bk"));
    let name: String = conn.query_row("SELECT name FROM schema_version WHERE version = 3", [], |r| r.get(0)).unwrap();
    assert_eq!(name, "item_type");
    let fk: i64 = conn.prepare("PRAGMA foreign_key_check").unwrap().query_map([], |_| Ok(())).unwrap().count() as i64;
    assert_eq!(fk, 0);
    drop(conn);
    cleanup(&path);
}

// ---------------------------------------------------------------- pre-migration backup (DD-29)

fn premigration_files(db: &Path) -> Vec<PathBuf> {
    let dir = db.parent().unwrap().join("backups");
    let mut v: Vec<PathBuf> = std::fs::read_dir(&dir)
        .map(|rd| rd.filter_map(Result::ok).map(|e| e.path()).collect())
        .unwrap_or_default();
    v.retain(|p| p.file_name().is_some_and(|n| n.to_string_lossy().starts_with("book_keeper_premigration_")));
    v
}

#[test]
fn upgrade_takes_exactly_one_premigration_backup_at_the_old_version() {
    let (path, conn) = db_from_fixtures("premig", &[V1, V2]);
    conn.execute("INSERT INTO items (name) VALUES ('Pen')", []).unwrap();
    run_migrations(&conn).expect("upgrade");
    let files = premigration_files(&path);
    assert_eq!(files.len(), 1, "{files:?}");
    let name = files[0].file_name().unwrap().to_string_lossy().to_string();
    assert!(name.starts_with("book_keeper_premigration_v2_to_v"), "{name}");
    assert!(name.ends_with(".db"));
    let bk = Connection::open(&files[0]).unwrap();
    let max: i64 = bk.query_row("SELECT MAX(version) FROM schema_version", [], |r| r.get(0)).unwrap();
    assert_eq!(max, 2);
    let items: i64 = bk.query_row("SELECT COUNT(*) FROM items", [], |r| r.get(0)).unwrap();
    assert_eq!(items, 1);
    let has_col: i64 = bk
        .query_row("SELECT COUNT(*) FROM pragma_table_info('items') WHERE name = 'item_type'", [], |r| r.get(0))
        .unwrap();
    assert_eq!(has_col, 0, "backup holds the pre-upgrade schema");
    drop(bk);
    drop(conn);
    cleanup(&path);
}

#[test]
fn fresh_db_and_already_latest_db_create_no_premigration_backup() {
    let (path, pool) = fresh();
    assert!(premigration_files(&path).is_empty(), "fresh creation must not back up");
    run_migrations(&pool.get().unwrap()).unwrap();
    assert!(premigration_files(&path).is_empty(), "already-latest must not back up");
    drop(pool);
    cleanup(&path);
}

#[test]
fn in_memory_database_upgrade_skips_the_backup() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(V1).unwrap();
    conn.execute_batch(V2).unwrap();
    run_migrations(&conn).expect("in-memory upgrade works without a backup");
    assert_eq!(current_version(&conn).unwrap(), latest_version());
}

#[test]
fn failed_premigration_backup_applies_nothing() {
    let (path, conn) = db_from_fixtures("premigfail", &[V1, V2]);
    // A file where the `backups` folder must go makes the snapshot impossible.
    std::fs::write(path.parent().unwrap().join("backups"), b"not a folder").unwrap();
    let err = run_migrations(&conn).expect_err("must abort");
    assert!(matches!(err, AppError::MigrationError(_)), "{err:?}");
    assert_eq!(current_version(&conn).unwrap(), 2, "nothing was applied");
    drop(conn);
    cleanup(&path);
}

#[test]
fn fresh_db_v4_has_threshold_default_check_and_party_fts() {
    let (path, pool) = fresh();
    let conn = pool.get().unwrap();
    assert!(latest_version() >= 4);
    conn.execute("INSERT INTO items (name) VALUES ('X')", []).unwrap();
    let t: f64 = conn.query_row("SELECT low_stock_threshold FROM items WHERE name='X'", [], |r| r.get(0)).unwrap();
    assert_eq!(t, 5.0);
    assert!(conn.execute("UPDATE items SET low_stock_threshold = -1", []).is_err());
    let n: i64 = conn.query_row("SELECT COUNT(*) FROM sqlite_master WHERE name='party_search_fts'", [], |r| r.get(0)).unwrap();
    assert_eq!(n, 1);
    drop(conn);
    drop(pool);
    cleanup(&path);
}

#[test]
fn v3_database_upgrades_to_v4_recomputes_flag_and_indexes_parties() {
    let (path, conn) = db_from_fixtures("v3up", &[V1, V2, V3]);
    conn.execute_batch(
        "INSERT INTO items (name, current_stock, is_low_stock, item_type) VALUES
           ('Pen', 3, 1, 'PHYSICAL'), ('Ink', 10, 0, 'PHYSICAL'), ('Print', 0, 1, 'SERVICE'), ('Paper', 5, 0, 'PHYSICAL');
         INSERT INTO parties (id, name, phone, party_type, is_active) VALUES
           (1, 'Ramesh Kumar', '98765-43210', 'CUSTOMER', 1),
           (2, 'Old Vendor', NULL, 'VENDOR', 0),
           (3, 'Sita Traders', '+91 99887 66554', 'VENDOR', 1);",
    )
    .unwrap();
    run_migrations(&conn).expect("upgrade");
    assert!(current_version(&conn).unwrap() >= 4);
    let thr: Vec<f64> = conn.prepare("SELECT low_stock_threshold FROM items ORDER BY id").unwrap()
        .query_map([], |r| r.get(0)).unwrap().map(Result::unwrap).collect();
    assert_eq!(thr, vec![5.0; 4]);
    let flags: Vec<i64> = conn.prepare("SELECT is_low_stock FROM items ORDER BY id").unwrap()
        .query_map([], |r| r.get(0)).unwrap().map(Result::unwrap).collect();
    assert_eq!(flags, vec![1, 0, 0, 0]);
    let n: i64 = conn.query_row("SELECT COUNT(*) FROM party_search_fts", [], |r| r.get(0)).unwrap();
    assert_eq!(n, 2);
    let id: i64 = conn.query_row("SELECT party_id FROM party_search_fts WHERE party_search_fts MATCH '\"ram\"*'", [], |r| r.get(0)).unwrap();
    assert_eq!(id, 1);
    let d: String = conn.query_row("SELECT phone_digits FROM party_search_fts WHERE party_id = 1", [], |r| r.get(0)).unwrap();
    assert_eq!(d, "9876543210");
    let id: i64 = conn.query_row("SELECT party_id FROM party_search_fts WHERE party_search_fts MATCH '\"9199\"*'", [], |r| r.get(0)).unwrap();
    assert_eq!(id, 3);
    run_migrations(&conn).expect("second run is a no-op");
    drop(conn);
    cleanup(&path);
}

#[test]
fn v4_database_upgrades_stock_adjustments_to_accept_unrecorded_stock() {
    let (path, conn) = db_from_fixtures("v4up", &[V1, V2, V3, V4]);
    conn.execute_batch(
        "INSERT INTO items (id, name) VALUES (1, 'Pen');
         INSERT INTO purchase_transactions (id, transaction_code, transaction_date, payment_status, total_amount)
           VALUES (1, 'PUR-1001', '2026-11-01', 'PAID', 0);
         INSERT INTO purchase_items (id, purchase_transaction_id, item_id, item_name, pack_quantity, quantity, unit_cost, total_amount, remaining_quantity)
           VALUES (1, 1, 1, 'Pen', 10, 10, 5, 50, 7);
         INSERT INTO stock_adjustments (id, adjustment_date, item_id, quantity, unit_cost, reason, notes, created_at)
           VALUES (7, '2026-11-01', 1, -3, NULL, 'DAMAGED', 'broken', '2026-11-01T10:00:00Z');
         INSERT INTO stock_adjustments (id, adjustment_date, item_id, quantity, unit_cost, reason, notes, created_at)
           VALUES (9, '2026-11-02', 1, 2, 5.0, 'COUNT_CORRECTION', NULL, '2026-11-02T10:00:00Z');
         INSERT INTO stock_outflow_allocations (source_type, source_id, purchase_item_id, quantity_depleted, unit_cost)
           VALUES ('ADJUSTMENT', 7, 1, 3, 5);
         UPDATE sqlite_sequence SET seq = 20 WHERE name = 'stock_adjustments';",
    )
    .unwrap();
    type Row = (i64, String, i64, f64, Option<f64>, String, Option<String>, String);
    let read = |c: &Connection| -> Vec<Row> {
        c.prepare("SELECT id, adjustment_date, item_id, quantity, unit_cost, reason, notes, created_at FROM stock_adjustments ORDER BY id")
            .unwrap()
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?, r.get(7)?)))
            .unwrap()
            .map(Result::unwrap)
            .collect()
    };
    let before = read(&conn);
    run_migrations(&conn).expect("upgrade");
    assert!(current_version(&conn).unwrap() >= 5);
    let uv: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap();
    assert_eq!(uv, latest_version());
    assert_eq!(read(&conn), before, "rows identical");
    let joined: i64 = conn
        .query_row("SELECT COUNT(*) FROM stock_outflow_allocations o JOIN stock_adjustments j ON j.id = o.source_id", [], |r| r.get(0))
        .unwrap();
    assert_eq!(joined, 1);
    let seq: i64 = conn.query_row("SELECT seq FROM sqlite_sequence WHERE name = 'stock_adjustments'", [], |r| r.get(0)).unwrap();
    assert_eq!(seq, 20);
    let stray: i64 = conn.query_row("SELECT COUNT(*) FROM sqlite_sequence WHERE name = 'stock_adjustments_v5'", [], |r| r.get(0)).unwrap();
    assert_eq!(stray, 0);
    conn.execute("INSERT INTO stock_adjustments (adjustment_date, item_id, quantity, reason) VALUES ('2026-11-03', 1, -1, 'DAMAGED')", []).unwrap();
    let new_id: i64 = conn.query_row("SELECT MAX(id) FROM stock_adjustments", [], |r| r.get(0)).unwrap();
    assert_eq!(new_id, 21);
    let idx: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND tbl_name='stock_adjustments' AND name IN ('idx_adjustments_item','idx_adjustments_date')",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(idx, 2);
    let ins = |qty: f64, cost: Option<f64>, reason: &str| {
        conn.execute(
            "INSERT INTO stock_adjustments (adjustment_date, item_id, quantity, unit_cost, reason) VALUES ('2026-11-03', 1, ?1, ?2, ?3)",
            rusqlite::params![qty, cost, reason],
        )
    };
    assert!(ins(4.0, Some(6.0), "UNRECORDED_STOCK").is_ok());
    assert!(ins(-1.0, Some(6.0), "UNRECORDED_STOCK").is_err());
    assert!(ins(4.0, None, "UNRECORDED_STOCK").is_err());
    assert!(ins(4.0, Some(6.0), "BOGUS").is_err());
    let fk: i64 = conn.query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |r| r.get(0)).unwrap();
    assert_eq!(fk, 0);
    let ic: String = conn.query_row("PRAGMA integrity_check", [], |r| r.get(0)).unwrap();
    assert_eq!(ic, "ok");
    drop(conn);
    cleanup(&path);
}

#[test]
fn v5_database_with_data_upgrades_to_v6_expenses_and_seeds_code_sequences() {
    let (path, conn) = db_from_fixtures("v5up", &[V1, V2, V3, V4, V5]);
    conn.execute_batch(
        "INSERT INTO items (id, name) VALUES (1, 'Pen');
         INSERT INTO purchase_transactions (id, transaction_code, transaction_date, payment_status, total_amount)
           VALUES (1, 'PUR-1007', '2026-11-01', 'PAID', 50), (2, 'OPN-0003', '2026-11-01', 'PAID', 0),
                  (3, 'PUR-C1', '2026-11-01', 'PAID', 0);
         INSERT INTO sale_transactions (id, transaction_code, transaction_date, payment_status, total_amount)
           VALUES (1, 'SAL-1004', '2026-11-02', 'PAID', 20);",
    )
    .unwrap();
    run_migrations(&conn).expect("upgrade");
    assert!(current_version(&conn).unwrap() >= 6);
    let seq = |p: &str| -> i64 {
        conn.query_row("SELECT last_value FROM code_sequences WHERE prefix = ?1", [p], |r| r.get(0)).unwrap()
    };
    assert_eq!(seq("PUR-"), 1007, "non-numeric PUR-C1 ignored");
    assert_eq!(seq("OPN-"), 3);
    assert_eq!(seq("SAL-"), 1004);
    assert_eq!(seq("ADJ-"), 0);
    assert_eq!(seq("EXP-"), 0);
    let pur: i64 = conn.query_row("SELECT COUNT(*) FROM purchase_transactions", [], |r| r.get(0)).unwrap();
    assert_eq!(pur, 3);
    for t in ["expense_types", "expenses"] {
        let n: i64 = conn.query_row(&format!("SELECT COUNT(*) FROM {t}"), [], |r| r.get(0)).unwrap();
        assert_eq!(n, 0);
    }
    let fts: i64 = conn.query_row("SELECT COUNT(*) FROM sqlite_master WHERE name='expense_type_search_fts'", [], |r| r.get(0)).unwrap();
    assert_eq!(fts, 1);
    let fk: i64 = conn.query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |r| r.get(0)).unwrap();
    assert_eq!(fk, 0);
    let before = current_version(&conn).unwrap();
    run_migrations(&conn).unwrap();
    assert_eq!(current_version(&conn).unwrap(), before);
    drop(conn);
    cleanup(&path);
}

#[test]
fn expense_checks() {
    let (path, pool) = fresh();
    let conn = pool.get().unwrap();
    assert!(latest_version() >= 6);
    conn.execute("INSERT INTO expense_types (id, name) VALUES (1, 'Printer Ink')", []).unwrap();
    assert!(conn.execute("INSERT INTO expense_types (name) VALUES ('printer ink')", []).is_err(), "NOCASE unique");
    assert!(conn.execute("INSERT INTO expense_types (name) VALUES ('   ')", []).is_err(), "blank");
    let ins = |code: &str, type_id: i64, amount: f64| {
        conn.execute(
            "INSERT INTO expenses (expense_code, expense_date, expense_type_id, expense_type_name, amount)
             VALUES (?1, '2026-11-01', ?2, 'Printer Ink', ?3)",
            rusqlite::params![code, type_id, amount],
        )
    };
    assert!(ins("EXP-1001", 1, 0.0).is_err());
    assert!(ins("EXP-1001", 1, -1.0).is_err());
    assert!(ins("EXP-1001", 99, 5.0).is_err(), "dangling type (FK)");
    assert!(ins("EXP-1001", 1, 5.0).is_ok());
    assert!(ins("EXP-1001", 1, 6.0).is_err(), "duplicate code");
    drop(conn);
    drop(pool);
    cleanup(&path);
}

#[test]
fn v6_database_upgrades_to_v7_with_items_subtotal_backfill() {
    let (path, conn) = db_from_fixtures("v6up", &[V1, V2, V3, V4, V5, V6]);
    conn.execute_batch(
        "INSERT INTO sale_transactions (id, transaction_code, transaction_date, payment_status, total_amount, paid_amount)
           VALUES (1, 'SAL-1001', '2026-11-01', 'PAID', 150, 150);
         INSERT INTO purchase_transactions (id, transaction_code, transaction_date, payment_status, total_amount, source)
           VALUES (1, 'PUR-1001', '2026-11-01', 'UNPAID', 200, 'PURCHASE'), (2, 'OPN-0001', '2026-11-01', 'PAID', 0, 'OPENING');",
    )
    .unwrap();
    run_migrations(&conn).expect("upgrade");
    assert!(current_version(&conn).unwrap() >= 7);
    let sale: (f64, f64, f64) = conn
        .query_row("SELECT items_subtotal, charges_total, discounts_total FROM sale_transactions WHERE id=1", [], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?))
        })
        .unwrap();
    assert_eq!(sale, (150.0, 0.0, 0.0));
    let sub = |id: i64| -> f64 {
        conn.query_row("SELECT items_subtotal FROM purchase_transactions WHERE id=?1", [id], |r| r.get(0)).unwrap()
    };
    assert_eq!(sub(1), 200.0);
    assert_eq!(sub(2), 0.0);
    for t in ["charge_types", "bill_charges", "charge_type_search_fts"] {
        let n: i64 = conn.query_row("SELECT COUNT(*) FROM sqlite_master WHERE name = ?1", [t], |r| r.get(0)).unwrap();
        assert_eq!(n, 1, "{t}");
    }
    let before = current_version(&conn).unwrap();
    run_migrations(&conn).unwrap();
    assert_eq!(current_version(&conn).unwrap(), before);
    drop(conn);
    cleanup(&path);
}

#[test]
fn charge_tables_enforce_checks() {
    let (path, pool) = fresh();
    let conn = pool.get().unwrap();
    assert!(latest_version() >= 7);
    assert!(conn.execute("INSERT INTO charge_types (name, effect) VALUES ('X', 'BAD')", []).is_err(), "effect");
    assert!(
        conn.execute("INSERT INTO charge_types (name, effect, applies_to) VALUES ('X', 'CHARGE', 'NOPE')", []).is_err()
    );
    assert!(
        conn.execute("INSERT INTO charge_types (name, effect, default_amount) VALUES ('X', 'CHARGE', 0)", []).is_err()
    );
    conn.execute("INSERT INTO charge_types (id, name, effect) VALUES (1, 'Transport', 'CHARGE')", []).unwrap();
    assert!(conn.execute("INSERT INTO charge_types (name, effect) VALUES ('transport', 'CHARGE')", []).is_err());
    let ins = |amount: f64| {
        conn.execute(
            "INSERT INTO bill_charges (transaction_type, transaction_id, charge_type_id, charge_name, effect, amount)
             VALUES ('SALE', 1, 1, 'Transport', 'CHARGE', ?1)",
            [amount],
        )
    };
    assert!(ins(0.0).is_err());
    assert!(ins(5.0).is_ok());
    drop(conn);
    drop(pool);
    cleanup(&path);
}

#[test]
fn v7_database_upgrades_to_v8_keeping_bills() {
    let (path, conn) = db_from_fixtures("v7up", &[V1, V2, V3, V4, V5, V6, V7]);
    conn.execute_batch(
        "INSERT INTO sale_transactions (id, transaction_code, transaction_date, payment_status, total_amount, paid_amount)
           VALUES (1, 'SAL-1001', '2026-11-01', 'PAID', 150, 150);
         INSERT INTO purchase_transactions (id, transaction_code, transaction_date, payment_status, total_amount, source)
           VALUES (1, 'PUR-1001', '2026-11-01', 'UNPAID', 200, 'PURCHASE');",
    )
    .unwrap();
    run_migrations(&conn).expect("upgrade");
    assert!(current_version(&conn).unwrap() >= 8);
    for t in ["sale_transactions", "purchase_transactions"] {
        let n: i64 = conn.query_row(&format!("SELECT COUNT(*) FROM {t} WHERE updated_at IS NULL"), [], |r| r.get(0)).unwrap();
        assert_eq!(n, 1, "{t} row kept with NULL updated_at");
    }
    let idx: i64 = conn
        .query_row("SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name='idx_return_tx_original'", [], |r| r.get(0))
        .unwrap();
    assert_eq!(idx, 1);
    let fk: i64 = conn.query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |r| r.get(0)).unwrap();
    assert_eq!(fk, 0);
    let before = current_version(&conn).unwrap();
    run_migrations(&conn).unwrap();
    assert_eq!(current_version(&conn).unwrap(), before);
    drop(conn);
    cleanup(&path);
}

#[test]
fn v8_database_upgrades_to_v9_backfilling_line_dates_and_adding_indexes() {
    let (path, conn) = db_from_fixtures("v8up", &[V1, V2, V3, V4, V5, V6, V7, V8, V8_RECORD]);
    conn.execute_batch(
        "INSERT INTO items (id, name, base_unit, mrp, default_sale_price) VALUES (1, 'Rice', 'kg', 0, 10);
         INSERT INTO sale_transactions (id, transaction_code, transaction_date, payment_status, total_amount, paid_amount)
           VALUES (1, 'SAL-1001', '2026-11-01', 'PAID', 20, 20), (2, 'SAL-1002', '2026-11-05', 'PAID', 10, 10);
         INSERT INTO sale_items (id, sale_transaction_id, item_id, item_name, pack_quantity, quantity, unit_price, total_amount)
           VALUES (1, 1, 1, 'Rice', 1, 1, 10, 10), (2, 1, 1, 'Rice', 1, 1, 10, 10), (3, 2, 1, 'Rice', 1, 1, 10, 10);
         INSERT INTO purchase_transactions (id, transaction_code, transaction_date, payment_status, total_amount, source)
           VALUES (1, 'PUR-1001', '2026-10-01', 'PAID', 50, 'PURCHASE'), (2, 'OPN-1001', '2026-10-02', 'PAID', 0, 'OPENING');
         INSERT INTO purchase_items (id, purchase_transaction_id, item_id, item_name, pack_quantity, quantity, unit_cost, total_amount, remaining_quantity)
           VALUES (1, 1, 1, 'Rice', 1, 10, 5, 50, 10), (2, 2, 1, 'Rice', 1, 4, 5, 20, 4);",
    )
    .unwrap();
    // Pre-migration backup (file DB) is taken by run_migrations; it must not break the upgrade.
    run_migrations(&conn).expect("upgrade");
    assert!(current_version(&conn).unwrap() >= 9);
    let dates = |sql: &str| -> Vec<String> {
        let mut st = conn.prepare(sql).unwrap();
        st.query_map([], |r| r.get::<_, String>(0)).unwrap().map(|x| x.unwrap()).collect()
    };
    assert_eq!(dates("SELECT transaction_date FROM sale_items ORDER BY id"), ["2026-11-01", "2026-11-01", "2026-11-05"]);
    assert_eq!(dates("SELECT transaction_date FROM purchase_items ORDER BY id"), ["2026-10-01", "2026-10-02"]);
    for (t, idx) in [("sale_items", "idx_sale_items_item_date"), ("purchase_items", "idx_purchase_items_item_date")] {
        let n: i64 = conn
            .query_row("SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name=?1 AND tbl_name=?2", [idx, t], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 1, "{idx}");
    }
    let fk: i64 = conn.query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |r| r.get(0)).unwrap();
    assert_eq!(fk, 0);
    let before = current_version(&conn).unwrap();
    run_migrations(&conn).unwrap();
    assert_eq!(current_version(&conn).unwrap(), before);
    drop(conn);
    cleanup(&path);
}

#[test]
fn frozen_v9_fixture_matches_the_migrated_schema() {
    let (p1, from_fixtures) = db_from_fixtures("v9fx", &[V1, V2, V3, V4, V5, V6, V7, V8, V8_RECORD, V9]);
    let (p2, migrated) = db_from_fixtures("v9mig", &[V1]);
    run_migrations(&migrated).unwrap();
    let shape = |c: &Connection| -> Vec<String> {
        let mut st = c
            .prepare("SELECT name FROM pragma_table_info('sale_items') UNION ALL SELECT name FROM pragma_table_info('purchase_items') UNION ALL SELECT name FROM sqlite_master WHERE type='index' AND name LIKE 'idx_%_items_item_date' ORDER BY 1")
            .unwrap();
        // Columns added by later migrations (v12 `service_cost_total`) are checked by their own frozen test.
        st.query_map([], |r| r.get::<_, String>(0))
            .unwrap()
            .map(|x| x.unwrap())
            .filter(|n| n != "service_cost_total")
            .collect()
    };
    assert_eq!(shape(&from_fixtures), shape(&migrated));
    drop(from_fixtures);
    drop(migrated);
    cleanup(&p1);
    cleanup(&p2);
}

#[test]
fn v9_database_upgrades_to_v10_adding_opening_cash() {
    let (path, conn) = db_from_fixtures("v9up", &[V1, V2, V3, V4, V5, V6, V7, V8, V8_RECORD, V9]);
    conn.execute_batch("UPDATE shop_settings SET shop_name = 'Kept Store';").unwrap();
    run_migrations(&conn).expect("upgrade");
    assert!(current_version(&conn).unwrap() >= 10);
    let (name, cash): (String, f64) = conn
        .query_row("SELECT shop_name, opening_cash FROM shop_settings WHERE id = 1", [], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap();
    assert_eq!((name.as_str(), cash), ("Kept Store", 0.0));
    conn.execute("UPDATE shop_settings SET opening_cash = 250.5 WHERE id = 1", []).unwrap();
    assert!(conn.execute("UPDATE shop_settings SET opening_cash = -1 WHERE id = 1", []).is_err(), "CHECK rejects negatives");
    let before = current_version(&conn).unwrap();
    run_migrations(&conn).unwrap();
    assert_eq!(current_version(&conn).unwrap(), before);
    drop(conn);
    cleanup(&path);
}

#[test]
fn frozen_v10_fixture_matches_the_migrated_schema() {
    let (p1, from_fixtures) = db_from_fixtures("v10fx", &[V1, V2, V3, V4, V5, V6, V7, V8, V8_RECORD, V9, V10]);
    let (p2, migrated) = db_from_fixtures("v10mig", &[V1]);
    run_migrations(&migrated).unwrap();
    let shape = |c: &Connection| -> Vec<(String, String, i64, Option<String>)> {
        let mut st = c
            .prepare("SELECT name, type, \"notnull\", dflt_value FROM pragma_table_info('shop_settings') ORDER BY name")
            .unwrap();
        st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))).unwrap().map(|x| x.unwrap()).collect()
    };
    assert_eq!(shape(&from_fixtures), shape(&migrated));
    drop(from_fixtures);
    drop(migrated);
    cleanup(&p1);
    cleanup(&p2);
}

const V1_TO_V10: [&str; 11] = [V1, V2, V3, V4, V5, V6, V7, V8, V8_RECORD, V9, V10];

#[test]
fn v10_database_upgrades_to_v11_adding_the_opening_bill_flag() {
    let (path, conn) = db_from_fixtures("v10up", &V1_TO_V10);
    conn.execute_batch(
        "INSERT INTO parties (name, party_type) VALUES ('Ram', 'CUSTOMER'), ('Shyam', 'VENDOR');
         INSERT INTO sale_transactions (transaction_code, transaction_date, party_id, payment_status, total_amount, paid_amount, balance_due, created_at)
           VALUES ('SAL-1001', '2026-01-01', 1, 'UNPAID', 10, 0, 10, 'x');
         INSERT INTO purchase_transactions (transaction_code, transaction_date, party_id, payment_status, total_amount, paid_amount, balance_due, created_at)
           VALUES ('PUR-1001', '2026-01-01', 2, 'UNPAID', 10, 0, 10, 'x');",
    )
    .unwrap();
    run_migrations(&conn).expect("upgrade");
    assert!(current_version(&conn).unwrap() >= 11);
    let flags: (i64, i64) = conn
        .query_row(
            "SELECT (SELECT is_opening_balance FROM sale_transactions), (SELECT is_opening_balance FROM purchase_transactions)",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(flags, (0, 0), "existing bills default to not-opening");
    let ob_sale = |code: &str| {
        conn.execute(
            "INSERT INTO sale_transactions (transaction_code, transaction_date, party_id, payment_status, total_amount, paid_amount, balance_due, created_at, is_opening_balance)
             VALUES (?1, '2026-01-01', 1, 'UNPAID', 5, 0, 5, 'x', 1)",
            [code],
        )
    };
    ob_sale("OB-0001").unwrap();
    assert!(ob_sale("OB-0002").is_err(), "one opening bill per customer");
    assert!(
        conn.execute("UPDATE sale_transactions SET is_opening_balance = 2 WHERE id = 1", []).is_err(),
        "CHECK rejects other flag values"
    );
    let ob_purchase = |code: &str| {
        conn.execute(
            "INSERT INTO purchase_transactions (transaction_code, transaction_date, party_id, payment_status, total_amount, paid_amount, balance_due, created_at, is_opening_balance)
             VALUES (?1, '2026-01-01', 2, 'UNPAID', 5, 0, 5, 'x', 1)",
            [code],
        )
    };
    ob_purchase("OB-0003").unwrap();
    assert!(ob_purchase("OB-0004").is_err(), "one opening bill per vendor");
    let before = current_version(&conn).unwrap();
    run_migrations(&conn).unwrap();
    assert_eq!(current_version(&conn).unwrap(), before);
    drop(conn);
    cleanup(&path);
}

#[test]
fn frozen_v11_fixture_matches_the_migrated_schema() {
    let mut steps = V1_TO_V10.to_vec();
    steps.push(V11);
    let (p1, from_fixtures) = db_from_fixtures("v11fx", &steps);
    let (p2, migrated) = db_from_fixtures("v11mig", &[V1]);
    run_migrations(&migrated).unwrap();
    let shape = |c: &Connection| -> Vec<(String, String, i64, Option<String>)> {
        let mut out = Vec::new();
        for t in ["sale_transactions", "purchase_transactions"] {
            let mut st = c
                .prepare(&format!("SELECT '{t}.' || name, type, \"notnull\", dflt_value FROM pragma_table_info('{t}') ORDER BY name"))
                .unwrap();
            out.extend(st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))).unwrap().map(|x| x.unwrap()));
        }
        let mut st = c
            .prepare("SELECT name, tbl_name, 0, sql FROM sqlite_master WHERE type = 'index' AND name LIKE 'ux_%_opening_party' ORDER BY name")
            .unwrap();
        out.extend(st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))).unwrap().map(|x| x.unwrap()));
        out
    };
    assert_eq!(shape(&from_fixtures), shape(&migrated));
    assert!(shape(&migrated).iter().any(|r| r.0 == "ux_sale_opening_party"));
    drop(from_fixtures);
    drop(migrated);
    cleanup(&p1);
    cleanup(&p2);
}

const V1_TO_V11: [&str; 12] = [V1, V2, V3, V4, V5, V6, V7, V8, V8_RECORD, V9, V10, V11];

#[test]
fn v11_database_upgrades_to_v12_adding_service_tables_and_columns() {
    let (path, conn) = db_from_fixtures("v11up", &V1_TO_V11);
    conn.execute_batch(
        "INSERT INTO items (name) VALUES ('Paper'), ('Print');
         INSERT INTO sale_transactions (transaction_code, transaction_date, payment_status, total_amount, paid_amount, balance_due, created_at)
           VALUES ('SAL-1001', '2026-01-01', 'PAID', 10, 10, 0, 'x');
         INSERT INTO sale_items (sale_transaction_id, item_id, item_name, pack_quantity, quantity, unit_price, total_amount, transaction_date)
           VALUES (1, 1, 'Paper', 1, 1, 10, 10, '2026-01-01');",
    )
    .unwrap();
    run_migrations(&conn).expect("upgrade");
    assert!(current_version(&conn).unwrap() >= 12);
    let (sc, st): (Option<f64>, Option<f64>) = conn
        .query_row("SELECT (SELECT service_cost FROM items WHERE id = 1), (SELECT service_cost_total FROM sale_items WHERE id = 1)", [], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap();
    assert_eq!((sc, st), (None, None), "existing rows default to NULL");
    conn.execute("INSERT INTO service_components (service_item_id, component_item_id, quantity_per_unit) VALUES (2, 1, 1.5)", []).unwrap();
    assert!(conn.execute("INSERT INTO service_components (service_item_id, component_item_id, quantity_per_unit) VALUES (2, 1, 2)", []).is_err(), "pair is unique");
    assert!(conn.execute("INSERT INTO service_components (service_item_id, component_item_id, quantity_per_unit) VALUES (2, 2, 1)", []).is_err(), "no self reference");
    assert!(conn.execute("INSERT INTO service_components (service_item_id, component_item_id, quantity_per_unit) VALUES (1, 2, 0)", []).is_err(), "quantity must be positive");
    assert!(conn.execute("UPDATE items SET service_cost = -1 WHERE id = 2", []).is_err());
    assert!(conn.execute("UPDATE sale_items SET service_cost_total = -1 WHERE id = 1", []).is_err());
    let dropped: i64 = conn
        .query_row("SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND name = 'idx_sale_items_item'", [], |r| r.get(0))
        .unwrap();
    assert_eq!(dropped, 0, "the prefix-duplicate index is gone");
    let kept: i64 = conn
        .query_row("SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND name = 'idx_sale_items_item_date'", [], |r| r.get(0))
        .unwrap();
    assert_eq!(kept, 1);
    let before = current_version(&conn).unwrap();
    run_migrations(&conn).unwrap();
    assert_eq!(current_version(&conn).unwrap(), before);
    drop(conn);
    cleanup(&path);
}

#[test]
fn frozen_v12_fixture_matches_the_migrated_schema() {
    let mut steps = V1_TO_V11.to_vec();
    steps.push(V12);
    let (p1, from_fixtures) = db_from_fixtures("v12fx", &steps);
    let (p2, migrated) = db_from_fixtures("v12mig", &[V1]);
    run_migrations(&migrated).unwrap();
    let shape = |c: &Connection| -> Vec<(String, String, i64, Option<String>)> {
        let mut out = Vec::new();
        for t in ["items", "sale_items", "service_components"] {
            let mut st = c
                .prepare(&format!("SELECT '{t}.' || name, type, \"notnull\", dflt_value FROM pragma_table_info('{t}') ORDER BY name"))
                .unwrap();
            out.extend(st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))).unwrap().map(|x| x.unwrap()));
        }
        let mut st = c
            .prepare("SELECT name, tbl_name, 0, sql FROM sqlite_master WHERE type = 'index' AND (name LIKE 'idx_service_%' OR name LIKE 'idx_sale_items_%') ORDER BY name")
            .unwrap();
        out.extend(st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))).unwrap().map(|x| x.unwrap()));
        out
    };
    assert_eq!(shape(&from_fixtures), shape(&migrated));
    assert!(shape(&migrated).iter().any(|r| r.0 == "idx_service_components_component"));
    assert!(!shape(&migrated).iter().any(|r| r.0 == "idx_sale_items_item"));
    drop(from_fixtures);
    drop(migrated);
    cleanup(&p1);
    cleanup(&p2);
}

fn premig_in(dir: &Path) -> usize {
    std::fs::read_dir(dir)
        .map(|rd| {
            rd.filter_map(Result::ok)
                .filter(|e| e.file_name().to_string_lossy().starts_with("book_keeper_premigration_"))
                .count()
        })
        .unwrap_or(0)
}

#[test]
fn premigration_backup_follows_the_chosen_backup_folder() {
    let (path, conn) = db_from_fixtures("premigdir", &[V1, V2]);
    let chosen = path.parent().unwrap().join("chosen_folder");
    conn.execute("UPDATE shop_settings SET backup_directory = ?1 WHERE id = 1", [chosen.to_str().unwrap()]).unwrap();
    run_migrations(&conn).expect("upgrade");
    assert_eq!(premig_in(&chosen), 1);
    assert!(premigration_files(&path).is_empty(), "default folder must stay unused");
    drop(conn);
    cleanup(&path);
}

#[test]
fn premigration_backup_falls_back_when_column_missing_or_folder_unusable() {
    // Schema v1 has no backup_directory column.
    let (path, conn) = db_from_fixtures("premigcol", &[V1]);
    run_migrations(&conn).expect("upgrade");
    assert_eq!(premigration_files(&path).len(), 1);
    drop(conn);
    cleanup(&path);

    // A folder that cannot be created (its parent is a file).
    let (path, conn) = db_from_fixtures("premigbad", &[V1, V2]);
    let blocker = path.parent().unwrap().join("blocker");
    std::fs::write(&blocker, b"file").unwrap();
    let bad = blocker.join("sub");
    conn.execute("UPDATE shop_settings SET backup_directory = ?1 WHERE id = 1", [bad.to_str().unwrap()]).unwrap();
    run_migrations(&conn).expect("upgrade");
    assert_eq!(premigration_files(&path).len(), 1);
    drop(conn);
    cleanup(&path);
}
