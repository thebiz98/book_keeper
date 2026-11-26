-- Frozen copy of migration v13 (line_total_adjustments, FR-47). Never edit.
ALTER TABLE charge_types ADD COLUMN system_key TEXT;
CREATE UNIQUE INDEX ux_charge_types_system_key ON charge_types(system_key) WHERE system_key IS NOT NULL;
UPDATE charge_types SET system_key = 'LINE_EXTRA'
 WHERE id = (SELECT id FROM charge_types WHERE name = 'Item extra charge' COLLATE NOCASE
              AND effect = 'CHARGE' AND applies_to IN ('SALE', 'BOTH') AND is_active = 1);
UPDATE charge_types SET system_key = 'LINE_DISCOUNT'
 WHERE id = (SELECT id FROM charge_types WHERE name = 'Item discount' COLLATE NOCASE
              AND effect = 'DISCOUNT' AND applies_to IN ('SALE', 'BOTH') AND is_active = 1);
INSERT INTO charge_types (name, effect, applies_to, system_key)
SELECT CASE WHEN EXISTS (SELECT 1 FROM charge_types WHERE name = 'Item extra charge' COLLATE NOCASE)
            THEN 'Item extra charge (auto)' ELSE 'Item extra charge' END, 'CHARGE', 'SALE', 'LINE_EXTRA'
 WHERE NOT EXISTS (SELECT 1 FROM charge_types WHERE system_key = 'LINE_EXTRA');
INSERT INTO charge_types (name, effect, applies_to, system_key)
SELECT CASE WHEN EXISTS (SELECT 1 FROM charge_types WHERE name = 'Item discount' COLLATE NOCASE)
            THEN 'Item discount (auto)' ELSE 'Item discount' END, 'DISCOUNT', 'SALE', 'LINE_DISCOUNT'
 WHERE NOT EXISTS (SELECT 1 FROM charge_types WHERE system_key = 'LINE_DISCOUNT');
INSERT INTO charge_type_search_fts (charge_type_id, name)
SELECT id, name FROM charge_types
 WHERE system_key IS NOT NULL AND id NOT IN (SELECT charge_type_id FROM charge_type_search_fts);
INSERT INTO schema_version (version, name) VALUES (13, 'line_total_adjustments');
PRAGMA user_version = 13;
