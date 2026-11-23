-- FROZEN: DDL delta of migration 5 as shipped. Never edit.
CREATE TABLE stock_adjustments_v5 (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    adjustment_date TEXT NOT NULL,
    item_id INTEGER NOT NULL REFERENCES items(id),
    quantity REAL NOT NULL CHECK (quantity <> 0),
    unit_cost REAL,
    reason TEXT NOT NULL CHECK (reason IN ('DAMAGED', 'EXPIRED', 'THEFT_LOST', 'COUNT_CORRECTION', 'UNRECORDED_STOCK')),
    notes TEXT,
    created_at TEXT NOT NULL DEFAULT (CURRENT_TIMESTAMP),
    CHECK (reason <> 'UNRECORDED_STOCK' OR (quantity > 0 AND unit_cost IS NOT NULL))
);
INSERT INTO stock_adjustments_v5 (id, adjustment_date, item_id, quantity, unit_cost, reason, notes, created_at)
    SELECT id, adjustment_date, item_id, quantity, unit_cost, reason, notes, created_at FROM stock_adjustments;
DELETE FROM sqlite_sequence WHERE name = 'stock_adjustments_v5';
INSERT INTO sqlite_sequence (name, seq)
    SELECT 'stock_adjustments_v5', seq FROM sqlite_sequence WHERE name = 'stock_adjustments';
DROP TABLE stock_adjustments;
ALTER TABLE stock_adjustments_v5 RENAME TO stock_adjustments;
CREATE INDEX idx_adjustments_item ON stock_adjustments(item_id);
CREATE INDEX idx_adjustments_date ON stock_adjustments(adjustment_date);
INSERT INTO schema_version (version, name) VALUES (5, 'unrecorded_stock_reason');
PRAGMA user_version = 5;
