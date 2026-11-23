-- FROZEN: DDL delta of migration 2 as shipped. Never edit.
ALTER TABLE shop_settings ADD COLUMN backup_directory TEXT;
INSERT INTO schema_version (version, name) VALUES (2, 'backup_directory');
PRAGMA user_version = 2;
