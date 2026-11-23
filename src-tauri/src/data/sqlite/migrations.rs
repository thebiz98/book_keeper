//! Numbered, forward-only schema migrations recorded in `schema_version` (LLD §5.3, NFR-04).
//!
//! * DD-29: migration 1 IS the aligned schema (pre-release rewrite). A legacy pre-alignment
//!   database (tables present but no `schema_version`) is rejected, never touched.
//! * Each step runs in its own `BEGIN IMMEDIATE` transaction with `foreign_keys` OFF
//!   (the pragma is a no-op inside a transaction, so it is toggled around it) and
//!   `PRAGMA foreign_key_check` must come back empty before COMMIT.
//! * `PRAGMA user_version` mirrors the highest applied version as a fast guard.
use crate::core::error::AppError;

/// One forward-only migration step.
pub struct Migration {
    pub version: i64,
    pub name: &'static str,
    pub sql: &'static str,
}

const SCHEMA_VERSION_DDL: &str = "CREATE TABLE IF NOT EXISTS schema_version (
    version INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);";

const SCHEMA_V1: &str = r#"
CREATE TABLE shop_settings (
    id INTEGER PRIMARY KEY AUTOINCREMENT CHECK (id = 1),
    shop_name TEXT NOT NULL,
    phone TEXT,
    address TEXT,
    tax_id TEXT
);
INSERT INTO shop_settings (id, shop_name) VALUES (1, 'My Shop');

CREATE TABLE items (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL COLLATE NOCASE UNIQUE,
    category TEXT,
    barcode TEXT,
    base_unit TEXT NOT NULL DEFAULT 'PCS',
    mrp REAL,
    default_sale_price REAL NOT NULL DEFAULT 0.0,
    current_stock REAL NOT NULL DEFAULT 0.0,
    is_low_stock INTEGER NOT NULL DEFAULT 0 CHECK (is_low_stock IN (0, 1)),
    is_active INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0, 1)),
    created_at TEXT NOT NULL DEFAULT (CURRENT_TIMESTAMP),
    updated_at TEXT NOT NULL DEFAULT (CURRENT_TIMESTAMP)
);
CREATE INDEX idx_items_name ON items(name COLLATE NOCASE);
CREATE INDEX idx_items_barcode ON items(barcode);
CREATE INDEX idx_items_category ON items(category);

CREATE TABLE item_bundles (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    item_id INTEGER NOT NULL REFERENCES items(id),
    bundle_name TEXT NOT NULL,
    units_per_bundle REAL NOT NULL CHECK (units_per_bundle > 1),
    mrp REAL,
    default_bundle_price REAL,
    barcode TEXT,
    UNIQUE (item_id, bundle_name)
);
CREATE INDEX idx_item_bundles_barcode ON item_bundles(barcode);

CREATE TABLE parties (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    phone TEXT,
    party_type TEXT NOT NULL CHECK (party_type IN ('CUSTOMER', 'VENDOR')),
    notes TEXT,
    is_active INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0, 1)),
    created_at TEXT NOT NULL DEFAULT (CURRENT_TIMESTAMP),
    updated_at TEXT NOT NULL DEFAULT (CURRENT_TIMESTAMP)
);
CREATE INDEX idx_parties_type ON parties(party_type);

CREATE TABLE purchase_transactions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    transaction_code TEXT NOT NULL UNIQUE,
    source TEXT NOT NULL DEFAULT 'PURCHASE'
        CHECK (source IN ('PURCHASE', 'ADJUSTMENT', 'OPENING', 'RETURN')),
    supplier_invoice_ref TEXT,
    transaction_date TEXT NOT NULL,
    party_id INTEGER REFERENCES parties(id),
    party_name TEXT,
    payment_mode TEXT,
    payment_status TEXT NOT NULL CHECK (payment_status IN ('PAID', 'PARTIAL', 'UNPAID')),
    total_amount REAL NOT NULL,
    paid_amount REAL NOT NULL DEFAULT 0.0,
    balance_due REAL NOT NULL DEFAULT 0.0,
    notes TEXT,
    created_at TEXT NOT NULL DEFAULT (CURRENT_TIMESTAMP)
);
CREATE INDEX idx_purchase_tx_source_date ON purchase_transactions(source, transaction_date);
CREATE INDEX idx_purchase_tx_party ON purchase_transactions(party_id);

CREATE TABLE purchase_items (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    purchase_transaction_id INTEGER NOT NULL REFERENCES purchase_transactions(id),
    item_id INTEGER NOT NULL REFERENCES items(id),
    item_name TEXT NOT NULL,
    pack_label TEXT,
    pack_quantity REAL NOT NULL,
    quantity REAL NOT NULL,
    unit_cost REAL NOT NULL,
    total_amount REAL NOT NULL,
    remaining_quantity REAL NOT NULL,
    CHECK (remaining_quantity >= 0 AND remaining_quantity <= quantity)
);
CREATE INDEX idx_purchase_items_tx ON purchase_items(purchase_transaction_id);
CREATE INDEX idx_purchase_items_item ON purchase_items(item_id, remaining_quantity);

CREATE TABLE sale_transactions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    transaction_code TEXT NOT NULL UNIQUE,
    transaction_date TEXT NOT NULL,
    party_id INTEGER REFERENCES parties(id),
    party_name TEXT,
    payment_mode TEXT,
    payment_status TEXT NOT NULL CHECK (payment_status IN ('PAID', 'PARTIAL', 'UNPAID')),
    total_amount REAL NOT NULL,
    paid_amount REAL NOT NULL DEFAULT 0.0,
    balance_due REAL NOT NULL DEFAULT 0.0,
    notes TEXT,
    created_at TEXT NOT NULL DEFAULT (CURRENT_TIMESTAMP)
);
CREATE INDEX idx_sale_tx_date ON sale_transactions(transaction_date);
CREATE INDEX idx_sale_tx_party ON sale_transactions(party_id);

CREATE TABLE sale_items (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    sale_transaction_id INTEGER NOT NULL REFERENCES sale_transactions(id),
    item_id INTEGER NOT NULL REFERENCES items(id),
    item_name TEXT NOT NULL,
    pack_label TEXT,
    pack_quantity REAL NOT NULL,
    quantity REAL NOT NULL,
    unit_price REAL NOT NULL,
    total_amount REAL NOT NULL
);
CREATE INDEX idx_sale_items_tx ON sale_items(sale_transaction_id);
CREATE INDEX idx_sale_items_item ON sale_items(item_id);

CREATE TABLE sale_stock_allocations (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    sale_item_id INTEGER NOT NULL REFERENCES sale_items(id),
    purchase_item_id INTEGER NOT NULL REFERENCES purchase_items(id),
    quantity_depleted REAL NOT NULL,
    returned_quantity REAL NOT NULL DEFAULT 0,
    unit_cost REAL NOT NULL,
    CHECK (returned_quantity >= 0 AND returned_quantity <= quantity_depleted)
);
CREATE INDEX idx_sale_alloc_sale_item ON sale_stock_allocations(sale_item_id);
CREATE INDEX idx_sale_alloc_purchase_item ON sale_stock_allocations(purchase_item_id);

CREATE TABLE return_transactions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    return_code TEXT NOT NULL UNIQUE,
    return_date TEXT NOT NULL,
    return_type TEXT NOT NULL CHECK (return_type IN ('CUSTOMER', 'VENDOR')),
    party_id INTEGER REFERENCES parties(id),
    party_name TEXT,
    original_transaction_id INTEGER,
    settlement_type TEXT NOT NULL CHECK (settlement_type IN ('REFUND', 'REPLACEMENT')),
    total_amount REAL NOT NULL,
    reason TEXT,
    notes TEXT,
    created_at TEXT NOT NULL DEFAULT (CURRENT_TIMESTAMP),
    CHECK (NOT (return_type = 'VENDOR' AND settlement_type = 'REPLACEMENT'))
);
CREATE INDEX idx_return_tx_date ON return_transactions(return_date);
CREATE INDEX idx_return_tx_party ON return_transactions(party_id);

CREATE TABLE return_items (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    return_transaction_id INTEGER NOT NULL REFERENCES return_transactions(id),
    item_id INTEGER NOT NULL REFERENCES items(id),
    item_name TEXT NOT NULL,
    sale_item_id INTEGER REFERENCES sale_items(id),
    purchase_item_id INTEGER REFERENCES purchase_items(id),
    line_type TEXT NOT NULL DEFAULT 'RETURNED' CHECK (line_type IN ('RETURNED', 'REPLACEMENT_ISSUED')),
    quantity REAL NOT NULL,
    unit_price REAL NOT NULL,
    total_amount REAL NOT NULL,
    cost_amount REAL NOT NULL DEFAULT 0,
    condition TEXT CHECK (condition IS NULL OR condition IN ('RESTOCKABLE', 'DAMAGED_SCRAP'))
);
CREATE INDEX idx_return_items_tx ON return_items(return_transaction_id);
CREATE INDEX idx_return_items_item ON return_items(item_id);
CREATE INDEX idx_return_items_sale_item ON return_items(sale_item_id);
CREATE INDEX idx_return_items_purchase_item ON return_items(purchase_item_id);

CREATE TABLE payment_settlements (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    settlement_date TEXT NOT NULL,
    party_id INTEGER REFERENCES parties(id),
    kind TEXT NOT NULL CHECK (kind IN ('UPFRONT', 'SETTLEMENT', 'RETURN_CREDIT', 'REFUND_OUT', 'REFUND_IN')),
    transaction_type TEXT NOT NULL CHECK (transaction_type IN ('SALE', 'PURCHASE')),
    transaction_id INTEGER,
    return_id INTEGER REFERENCES return_transactions(id),
    amount_paid REAL NOT NULL CHECK (amount_paid > 0),
    payment_mode TEXT,
    notes TEXT,
    created_at TEXT NOT NULL DEFAULT (CURRENT_TIMESTAMP),
    CHECK (kind IN ('REFUND_OUT', 'REFUND_IN') OR transaction_id IS NOT NULL)
);
CREATE INDEX idx_settlements_party ON payment_settlements(party_id);
CREATE INDEX idx_settlements_tx ON payment_settlements(transaction_type, transaction_id);
CREATE INDEX idx_settlements_date ON payment_settlements(settlement_date);
CREATE INDEX idx_settlements_return ON payment_settlements(return_id);

CREATE TABLE stock_adjustments (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    adjustment_date TEXT NOT NULL,
    item_id INTEGER NOT NULL REFERENCES items(id),
    quantity REAL NOT NULL CHECK (quantity <> 0),
    unit_cost REAL,
    reason TEXT NOT NULL CHECK (reason IN ('DAMAGED', 'EXPIRED', 'THEFT_LOST', 'COUNT_CORRECTION')),
    notes TEXT,
    created_at TEXT NOT NULL DEFAULT (CURRENT_TIMESTAMP)
);
CREATE INDEX idx_adjustments_item ON stock_adjustments(item_id);
CREATE INDEX idx_adjustments_date ON stock_adjustments(adjustment_date);

CREATE TABLE stock_outflow_allocations (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    source_type TEXT NOT NULL CHECK (source_type IN ('ADJUSTMENT', 'REPLACEMENT', 'VENDOR_RETURN')),
    source_id INTEGER NOT NULL,
    purchase_item_id INTEGER NOT NULL REFERENCES purchase_items(id),
    quantity_depleted REAL NOT NULL,
    unit_cost REAL NOT NULL
);
CREATE INDEX idx_outflow_alloc_source ON stock_outflow_allocations(source_type, source_id);
CREATE INDEX idx_outflow_alloc_pi ON stock_outflow_allocations(purchase_item_id);

CREATE VIRTUAL TABLE item_search_fts USING fts5(
    item_id UNINDEXED,
    name,
    category,
    barcode,
    tokenize = 'unicode61 remove_diacritics 2'
);
"#;

/// All known migrations, ascending and gap-free. Append only; never edit a shipped step.
pub const MIGRATIONS: &[Migration] = &[
    Migration { version: 1, name: "aligned_schema", sql: SCHEMA_V1 },
    Migration {
        version: 2,
        name: "backup_directory",
        sql: "ALTER TABLE shop_settings ADD COLUMN backup_directory TEXT;",
    },
    Migration {
        version: 3,
        name: "item_type",
        sql: "ALTER TABLE items ADD COLUMN item_type TEXT NOT NULL DEFAULT 'PHYSICAL' CHECK (item_type IN ('PHYSICAL', 'SERVICE'));",
    },
];

/// Highest schema version this build knows.
pub fn latest_version() -> i64 {
    MIGRATIONS.last().map(|m| m.version).unwrap_or(0)
}

fn table_exists(conn: &rusqlite::Connection, name: &str) -> Result<bool, AppError> {
    let n: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
        [name],
        |r| r.get(0),
    )?;
    Ok(n > 0)
}

/// Applied version according to `schema_version` (0 when the table is absent or empty).
pub fn current_version(conn: &rusqlite::Connection) -> Result<i64, AppError> {
    if !table_exists(conn, "schema_version")? {
        return Ok(0);
    }
    Ok(conn.query_row("SELECT COALESCE(MAX(version), 0) FROM schema_version", [], |r| r.get(0))?)
}

/// True when user tables exist but there is no `schema_version` table: a pre-alignment database.
fn is_legacy(conn: &rusqlite::Connection) -> Result<bool, AppError> {
    if table_exists(conn, "schema_version")? {
        return Ok(false);
    }
    let n: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%'",
        [],
        |r| r.get(0),
    )?;
    Ok(n > 0)
}

fn legacy_error() -> AppError {
    AppError::MigrationError(
        "This database was created by a pre-release version of Book Keeper (no schema_version table) \
         and cannot be upgraded automatically. Export your data and start with a fresh database file \
         (or set BOOK_KEEPER_DB_PATH to a new location). The existing file has not been modified."
            .to_string(),
    )
}

fn mig_err(context: &str, e: impl std::fmt::Display) -> AppError {
    AppError::MigrationError(format!("{context}: {e}"))
}

/// Applies every pending migration. Safe to call on every start-up (idempotent).
pub fn run_migrations(conn: &rusqlite::Connection) -> Result<(), AppError> {
    // Cheap pre-checks that never write.
    let user_version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    let applied = current_version(conn)?;
    let latest = latest_version();
    if applied.max(user_version) > latest {
        return Err(AppError::MigrationError(format!(
            "Database schema version {} is newer than this application supports ({latest}). \
             Please update Book Keeper.",
            applied.max(user_version)
        )));
    }
    if is_legacy(conn)? {
        return Err(legacy_error());
    }
    if applied >= latest {
        return Ok(());
    }

    // TODO(T9): when `applied > 0` (a real upgrade, not first creation) take a pre-migration
    // backup through BackupService before applying any step.

    conn.execute_batch("PRAGMA foreign_keys = OFF;")?;
    let result = apply_pending(conn);
    // Always restore enforcement on this connection, whatever happened.
    let restore = conn.execute_batch("PRAGMA foreign_keys = ON;");
    result?;
    restore?;
    Ok(())
}

fn apply_pending(conn: &rusqlite::Connection) -> Result<(), AppError> {
    for m in MIGRATIONS {
        conn.execute_batch("BEGIN IMMEDIATE;")?;
        match apply_step(conn, m) {
            Ok(true) => conn.execute_batch("COMMIT;").map_err(|e| mig_err("commit failed", e))?,
            Ok(false) => {
                let _ = conn.execute_batch("ROLLBACK;");
            }
            Err(e) => {
                let _ = conn.execute_batch("ROLLBACK;");
                return Err(e);
            }
        }
    }
    Ok(())
}

/// Runs one step inside the open transaction. Returns `Ok(false)` if it was already applied
/// (decided after the write lock is held, so two racing starters cannot both apply it).
fn apply_step(conn: &rusqlite::Connection, m: &Migration) -> Result<bool, AppError> {
    if is_legacy(conn)? {
        return Err(legacy_error());
    }
    conn.execute_batch(SCHEMA_VERSION_DDL)?;
    if current_version(conn)? >= m.version {
        return Ok(false);
    }
    conn.execute_batch(m.sql)
        .map_err(|e| mig_err(&format!("migration {} ({}) failed", m.version, m.name), e))?;
    conn.execute(
        "INSERT INTO schema_version (version, name) VALUES (?1, ?2)",
        rusqlite::params![m.version, m.name],
    )?;
    conn.execute_batch(&format!("PRAGMA user_version = {};", m.version))?;

    let mut stmt = conn.prepare("PRAGMA foreign_key_check")?;
    let violations = stmt.query_map([], |_| Ok(()))?.count();
    if violations > 0 {
        return Err(AppError::MigrationError(format!(
            "migration {} ({}) left {violations} foreign key violation(s)",
            m.version, m.name
        )));
    }
    Ok(true)
}
