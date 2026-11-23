//! `FtsSpec` constants, one per FTS5 table. Later branches add `EXPENSE_TYPE_FTS` (v6) and
//! `CHARGE_TYPE_FTS` (v7) here.
use super::fts::FtsSpec;

/// Item name/category/barcode plus one extra row per bundle barcode (several rows per key).
pub static ITEM_FTS: FtsSpec = FtsSpec {
    table: "item_search_fts",
    key_column: "item_id",
    columns: &["name", "category", "barcode"],
    source_all_sql: "SELECT id, name, category, barcode FROM items WHERE is_active = 1 \
        UNION ALL \
        SELECT i.id, i.name, i.category, b.barcode FROM item_bundles b JOIN items i ON i.id = b.item_id \
        WHERE i.is_active = 1 AND b.barcode IS NOT NULL AND b.barcode <> ''",
    source_one_sql: "SELECT id, name, category, barcode FROM items WHERE is_active = 1 AND id = ?1 \
        UNION ALL \
        SELECT i.id, i.name, i.category, b.barcode FROM item_bundles b JOIN items i ON i.id = b.item_id \
        WHERE i.is_active = 1 AND i.id = ?1 AND b.barcode IS NOT NULL AND b.barcode <> ''",
    source_count_sql: "SELECT COUNT(*) FROM items WHERE is_active = 1",
};

/// Party name, phone and the phone with separators removed (so `9876543210` finds `98765-43210`).
pub static PARTY_FTS: FtsSpec = FtsSpec {
    table: "party_search_fts",
    key_column: "party_id",
    columns: &["name", "phone", "phone_digits"],
    source_all_sql: "SELECT id, name, phone, \
        REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(COALESCE(phone,''),' ',''),'-',''),'+',''),'(',''),')',''),'.','') \
        FROM parties WHERE is_active = 1",
    source_one_sql: "SELECT id, name, phone, \
        REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(COALESCE(phone,''),' ',''),'-',''),'+',''),'(',''),')',''),'.','') \
        FROM parties WHERE is_active = 1 AND id = ?1",
    source_count_sql: "SELECT COUNT(*) FROM parties WHERE is_active = 1",
};
