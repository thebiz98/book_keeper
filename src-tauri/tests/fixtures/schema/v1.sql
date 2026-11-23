-- FROZEN: schema as shipped by migration 1. Never edit.
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
CREATE TABLE schema_version (version INTEGER PRIMARY KEY, name TEXT NOT NULL, applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP);
INSERT INTO schema_version (version, name) VALUES (1, 'aligned_schema');
PRAGMA user_version = 1;
