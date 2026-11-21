//! Versioned schema migrations using `PRAGMA user_version`.
use crate::core::error::AppError;

const SCHEMA_V1: &str = r#"
CREATE TABLE IF NOT EXISTS shop_settings (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    shop_name TEXT NOT NULL,
    phone TEXT,
    address TEXT,
    tax_id TEXT
);
INSERT OR IGNORE INTO shop_settings (id, shop_name) VALUES (1, 'My Shop');

CREATE TABLE IF NOT EXISTS items (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    category TEXT,
    barcode TEXT,
    base_unit TEXT NOT NULL,
    mrp REAL,
    default_sale_price REAL NOT NULL DEFAULT 0,
    current_stock REAL NOT NULL DEFAULT 0,
    is_low_stock INTEGER NOT NULL DEFAULT 0,
    is_active INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_items_barcode ON items(barcode);
CREATE INDEX IF NOT EXISTS idx_items_name ON items(name);
CREATE INDEX IF NOT EXISTS idx_items_category ON items(category);

CREATE TABLE IF NOT EXISTS item_bundles (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    item_id INTEGER NOT NULL REFERENCES items(id),
    bundle_name TEXT NOT NULL,
    units_per_bundle REAL NOT NULL,
    mrp REAL,
    default_bundle_price REAL,
    barcode TEXT
);
CREATE INDEX IF NOT EXISTS idx_item_bundles_item_id ON item_bundles(item_id);
CREATE INDEX IF NOT EXISTS idx_item_bundles_barcode ON item_bundles(barcode);

CREATE TABLE IF NOT EXISTS parties (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    phone TEXT,
    party_type TEXT NOT NULL,
    notes TEXT,
    is_active INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_parties_type ON parties(party_type);

CREATE TABLE IF NOT EXISTS purchase_transactions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    transaction_code TEXT NOT NULL UNIQUE,
    transaction_date TEXT NOT NULL,
    party_id INTEGER REFERENCES parties(id),
    payment_mode TEXT,
    payment_status TEXT NOT NULL,
    total_amount REAL NOT NULL,
    paid_amount REAL NOT NULL,
    balance_due REAL NOT NULL,
    notes TEXT,
    created_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_purchase_tx_date ON purchase_transactions(transaction_date);
CREATE INDEX IF NOT EXISTS idx_purchase_tx_party ON purchase_transactions(party_id);

CREATE TABLE IF NOT EXISTS purchase_items (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    purchase_transaction_id INTEGER NOT NULL REFERENCES purchase_transactions(id),
    item_id INTEGER NOT NULL REFERENCES items(id),
    bundle_id INTEGER REFERENCES item_bundles(id),
    bundle_quantity REAL NOT NULL,
    quantity REAL NOT NULL,
    unit_cost REAL NOT NULL,
    total_amount REAL NOT NULL,
    remaining_quantity REAL NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_purchase_items_tx ON purchase_items(purchase_transaction_id);
CREATE INDEX IF NOT EXISTS idx_purchase_items_item ON purchase_items(item_id, remaining_quantity);

CREATE TABLE IF NOT EXISTS sale_transactions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    transaction_code TEXT NOT NULL UNIQUE,
    transaction_date TEXT NOT NULL,
    party_id INTEGER REFERENCES parties(id),
    payment_mode TEXT,
    payment_status TEXT NOT NULL,
    total_amount REAL NOT NULL,
    paid_amount REAL NOT NULL,
    balance_due REAL NOT NULL,
    notes TEXT,
    created_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_sale_tx_date ON sale_transactions(transaction_date);
CREATE INDEX IF NOT EXISTS idx_sale_tx_party ON sale_transactions(party_id);

CREATE TABLE IF NOT EXISTS sale_items (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    sale_transaction_id INTEGER NOT NULL REFERENCES sale_transactions(id),
    item_id INTEGER NOT NULL REFERENCES items(id),
    bundle_id INTEGER REFERENCES item_bundles(id),
    bundle_quantity REAL NOT NULL,
    quantity REAL NOT NULL,
    unit_price REAL NOT NULL,
    total_amount REAL NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_sale_items_tx ON sale_items(sale_transaction_id);
CREATE INDEX IF NOT EXISTS idx_sale_items_item ON sale_items(item_id);

CREATE TABLE IF NOT EXISTS sale_stock_allocations (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    sale_item_id INTEGER NOT NULL REFERENCES sale_items(id),
    purchase_item_id INTEGER NOT NULL REFERENCES purchase_items(id),
    quantity_depleted REAL NOT NULL,
    unit_cost REAL NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_sale_alloc_sale_item ON sale_stock_allocations(sale_item_id);
CREATE INDEX IF NOT EXISTS idx_sale_alloc_purchase_item ON sale_stock_allocations(purchase_item_id);

CREATE TABLE IF NOT EXISTS return_transactions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    return_code TEXT NOT NULL UNIQUE,
    return_date TEXT NOT NULL,
    return_type TEXT NOT NULL,
    party_id INTEGER REFERENCES parties(id),
    original_transaction_id INTEGER,
    settlement_type TEXT NOT NULL,
    total_amount REAL NOT NULL,
    reason TEXT,
    notes TEXT,
    created_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_return_tx_date ON return_transactions(return_date);
CREATE INDEX IF NOT EXISTS idx_return_tx_party ON return_transactions(party_id);

CREATE TABLE IF NOT EXISTS return_items (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    return_transaction_id INTEGER NOT NULL REFERENCES return_transactions(id),
    item_id INTEGER NOT NULL REFERENCES items(id),
    sale_item_id INTEGER REFERENCES sale_items(id),
    purchase_item_id INTEGER REFERENCES purchase_items(id),
    quantity REAL NOT NULL,
    unit_price REAL NOT NULL,
    total_amount REAL NOT NULL,
    condition TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_return_items_tx ON return_items(return_transaction_id);
CREATE INDEX IF NOT EXISTS idx_return_items_item ON return_items(item_id);

CREATE TABLE IF NOT EXISTS payment_settlements (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    settlement_date TEXT NOT NULL,
    party_id INTEGER REFERENCES parties(id),
    transaction_type TEXT NOT NULL,
    transaction_id INTEGER NOT NULL,
    amount_paid REAL NOT NULL,
    payment_mode TEXT,
    notes TEXT,
    created_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_settlements_party ON payment_settlements(party_id);
CREATE INDEX IF NOT EXISTS idx_settlements_tx ON payment_settlements(transaction_type, transaction_id);
CREATE INDEX IF NOT EXISTS idx_settlements_date ON payment_settlements(settlement_date);

CREATE TABLE IF NOT EXISTS stock_adjustments (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    adjustment_date TEXT NOT NULL,
    item_id INTEGER NOT NULL REFERENCES items(id),
    quantity REAL NOT NULL,
    reason TEXT NOT NULL,
    notes TEXT,
    created_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_adjustments_item ON stock_adjustments(item_id);
CREATE INDEX IF NOT EXISTS idx_adjustments_date ON stock_adjustments(adjustment_date);

CREATE TABLE IF NOT EXISTS adjustment_stock_allocations (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    stock_adjustment_id INTEGER NOT NULL REFERENCES stock_adjustments(id),
    purchase_item_id INTEGER NOT NULL REFERENCES purchase_items(id),
    quantity_depleted REAL NOT NULL,
    unit_cost REAL NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_adj_alloc_adj ON adjustment_stock_allocations(stock_adjustment_id);
CREATE INDEX IF NOT EXISTS idx_adj_alloc_pi ON adjustment_stock_allocations(purchase_item_id);

CREATE VIRTUAL TABLE IF NOT EXISTS item_search_fts USING fts5(
    item_id UNINDEXED,
    name,
    category,
    barcode,
    tokenize = 'unicode61 remove_diacritics 2'
);
"#;

pub fn run_migrations(conn: &rusqlite::Connection) -> Result<(), AppError> {
    let version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    if version < 1 {
        conn.execute_batch("BEGIN IMMEDIATE;")?;
        let res = conn
            .execute_batch(SCHEMA_V1)
            .and_then(|_| conn.execute_batch("PRAGMA user_version = 1;"));
        match res {
            Ok(_) => conn.execute_batch("COMMIT;")?,
            Err(e) => {
                let _ = conn.execute_batch("ROLLBACK;");
                return Err(AppError::MigrationError(e.to_string()));
            }
        }
    }
    Ok(())
}
