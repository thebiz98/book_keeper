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
