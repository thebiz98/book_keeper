-- FROZEN: DDL delta of migration 8 as shipped. Never edit.
ALTER TABLE sale_transactions ADD COLUMN updated_at TEXT;
ALTER TABLE purchase_transactions ADD COLUMN updated_at TEXT;
CREATE INDEX idx_return_tx_original ON return_transactions(return_type, original_transaction_id);
