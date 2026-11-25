-- FROZEN: DDL delta of migration 12 as shipped. Never edit.
CREATE TABLE service_components (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  service_item_id INTEGER NOT NULL REFERENCES items(id),
  component_item_id INTEGER NOT NULL REFERENCES items(id),
  quantity_per_unit REAL NOT NULL CHECK (quantity_per_unit > 0),
  UNIQUE (service_item_id, component_item_id),
  CHECK (service_item_id <> component_item_id)
);
CREATE INDEX idx_service_components_component ON service_components(component_item_id);
ALTER TABLE items ADD COLUMN service_cost REAL CHECK (service_cost IS NULL OR service_cost >= 0);
ALTER TABLE sale_items ADD COLUMN service_cost_total REAL CHECK (service_cost_total IS NULL OR service_cost_total >= 0);
DROP INDEX IF EXISTS idx_sale_items_item;
INSERT INTO schema_version (version, name) VALUES (12, 'service_items');
PRAGMA user_version = 12;
