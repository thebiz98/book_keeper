-- FROZEN: DDL delta of migration 11 as shipped. Never edit.
ALTER TABLE sale_transactions ADD COLUMN is_opening_balance INTEGER NOT NULL DEFAULT 0 CHECK (is_opening_balance IN (0, 1));
ALTER TABLE purchase_transactions ADD COLUMN is_opening_balance INTEGER NOT NULL DEFAULT 0 CHECK (is_opening_balance IN (0, 1));
CREATE UNIQUE INDEX ux_sale_opening_party ON sale_transactions(party_id) WHERE is_opening_balance = 1;
CREATE UNIQUE INDEX ux_purchase_opening_party ON purchase_transactions(party_id) WHERE is_opening_balance = 1;
INSERT INTO schema_version (version, name) VALUES (11, 'party_opening_balance');
PRAGMA user_version = 11;
