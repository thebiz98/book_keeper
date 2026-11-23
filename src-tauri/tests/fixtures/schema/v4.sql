-- FROZEN: DDL delta of migration 4 as shipped. Never edit.
ALTER TABLE items ADD COLUMN low_stock_threshold REAL NOT NULL DEFAULT 5 CHECK (low_stock_threshold >= 0);
UPDATE items SET is_low_stock =
  CASE WHEN item_type = 'PHYSICAL' AND current_stock < low_stock_threshold THEN 1 ELSE 0 END;
CREATE VIRTUAL TABLE party_search_fts USING fts5(
    party_id UNINDEXED, name, phone, phone_digits,
    tokenize = 'unicode61 remove_diacritics 2'
);
INSERT INTO party_search_fts (party_id, name, phone, phone_digits)
  SELECT id, name, phone,
         REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(COALESCE(phone,''),' ',''),'-',''),'+',''),'(',''),')',''),'.','')
  FROM parties WHERE is_active = 1;
INSERT INTO schema_version (version, name) VALUES (4, 'low_stock_threshold_party_fts');
PRAGMA user_version = 4;
