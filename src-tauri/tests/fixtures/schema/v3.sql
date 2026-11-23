-- FROZEN: DDL delta of migration 3 as shipped. Never edit.
ALTER TABLE items ADD COLUMN item_type TEXT NOT NULL DEFAULT 'PHYSICAL' CHECK (item_type IN ('PHYSICAL', 'SERVICE'));
INSERT INTO schema_version (version, name) VALUES (3, 'item_type');
PRAGMA user_version = 3;
