-- FROZEN: DDL delta of migration 7 as shipped. Never edit.
ALTER TABLE sale_transactions ADD COLUMN items_subtotal REAL NOT NULL DEFAULT 0.0;
ALTER TABLE sale_transactions ADD COLUMN charges_total REAL NOT NULL DEFAULT 0.0;
ALTER TABLE sale_transactions ADD COLUMN discounts_total REAL NOT NULL DEFAULT 0.0;
ALTER TABLE purchase_transactions ADD COLUMN items_subtotal REAL NOT NULL DEFAULT 0.0;
ALTER TABLE purchase_transactions ADD COLUMN charges_total REAL NOT NULL DEFAULT 0.0;
ALTER TABLE purchase_transactions ADD COLUMN discounts_total REAL NOT NULL DEFAULT 0.0;
UPDATE sale_transactions SET items_subtotal = total_amount;
UPDATE purchase_transactions
   SET items_subtotal = CASE WHEN source = 'PURCHASE' THEN total_amount ELSE 0.0 END;
CREATE TABLE charge_types (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL COLLATE NOCASE UNIQUE CHECK (length(trim(name)) > 0),
    effect TEXT NOT NULL CHECK (effect IN ('CHARGE', 'DISCOUNT')),
    applies_to TEXT NOT NULL DEFAULT 'BOTH' CHECK (applies_to IN ('SALE', 'PURCHASE', 'BOTH')),
    default_amount REAL CHECK (default_amount IS NULL OR default_amount > 0),
    is_active INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0, 1)),
    created_at TEXT NOT NULL DEFAULT (CURRENT_TIMESTAMP)
);
CREATE TABLE bill_charges (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    transaction_type TEXT NOT NULL CHECK (transaction_type IN ('SALE', 'PURCHASE')),
    transaction_id INTEGER NOT NULL,
    charge_type_id INTEGER NOT NULL REFERENCES charge_types(id),
    charge_name TEXT NOT NULL,
    effect TEXT NOT NULL CHECK (effect IN ('CHARGE', 'DISCOUNT')),
    amount REAL NOT NULL CHECK (amount > 0)
);
CREATE INDEX idx_bill_charges_tx ON bill_charges(transaction_type, transaction_id);
CREATE INDEX idx_bill_charges_type ON bill_charges(charge_type_id);
CREATE VIRTUAL TABLE charge_type_search_fts USING fts5(
    charge_type_id UNINDEXED, name,
    tokenize = 'unicode61 remove_diacritics 2'
);
INSERT INTO schema_version (version, name) VALUES (7, 'bill_charges');
PRAGMA user_version = 7;
