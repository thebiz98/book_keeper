-- FROZEN: DDL delta of migration 10 as shipped. Never edit.
ALTER TABLE shop_settings ADD COLUMN opening_cash REAL NOT NULL DEFAULT 0 CHECK (opening_cash >= 0);
INSERT INTO schema_version (version, name) VALUES (10, 'cash_opening');
PRAGMA user_version = 10;
