-- FROZEN: DDL delta of migration 9 as shipped. Never edit.
ALTER TABLE sale_items ADD COLUMN transaction_date TEXT NOT NULL DEFAULT '';
UPDATE sale_items SET transaction_date = (SELECT transaction_date FROM sale_transactions s WHERE s.id = sale_items.sale_transaction_id);
ALTER TABLE purchase_items ADD COLUMN transaction_date TEXT NOT NULL DEFAULT '';
UPDATE purchase_items SET transaction_date = (SELECT transaction_date FROM purchase_transactions p WHERE p.id = purchase_items.purchase_transaction_id);
CREATE INDEX idx_sale_items_item_date ON sale_items(item_id, transaction_date DESC, sale_transaction_id DESC);
CREATE INDEX idx_purchase_items_item_date ON purchase_items(item_id, transaction_date DESC, purchase_transaction_id DESC);
INSERT INTO schema_version (version, name) VALUES (9, 'line_transaction_date');
PRAGMA user_version = 9;
