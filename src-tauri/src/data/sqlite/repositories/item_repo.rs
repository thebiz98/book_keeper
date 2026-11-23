//! SQLite implementation of `ItemRepository`.
use rusqlite::{params, OptionalExtension, Row};

use crate::core::entities::Item;
use crate::core::stock_rules::LOW_STOCK_SQL;
use crate::core::error::AppError;
use crate::data::contracts::ItemRepository;
use crate::data::sqlite::{enum_col, enum_to_sql, SqliteConn};

pub struct SqliteItemRepository {
    conn: SqliteConn,
}

impl SqliteItemRepository {
    pub fn new(conn: SqliteConn) -> Self {
        Self { conn }
    }
}

const COLS: &str = "id, name, category, barcode, base_unit, mrp, default_sale_price, current_stock, is_low_stock, is_active, created_at, updated_at, item_type, low_stock_threshold";

fn map_item(r: &Row<'_>) -> rusqlite::Result<Item> {
    Ok(Item {
        id: r.get(0)?,
        name: r.get(1)?,
        category: r.get(2)?,
        barcode: r.get(3)?,
        base_unit: r.get(4)?,
        mrp: r.get(5)?,
        default_sale_price: r.get(6)?,
        current_stock: r.get(7)?,
        is_low_stock: r.get(8)?,
        is_active: r.get(9)?,
        created_at: r.get(10)?,
        updated_at: r.get(11)?,
        item_type: enum_col(r, 12)?,
        low_stock_threshold: r.get(13)?,
    })
}

fn not_found(id: i64) -> AppError {
    AppError::NotFound { entity: "Item".to_string(), id: id.to_string() }
}

impl ItemRepository for SqliteItemRepository {
    fn find_by_id(&self, id: i64) -> Result<Option<Item>, AppError> {
        Ok(self
            .conn
            .query_row(&format!("SELECT {COLS} FROM items WHERE id = ?1"), params![id], map_item)
            .optional()?)
    }

    fn find_by_name(&self, name: &str) -> Result<Option<Item>, AppError> {
        Ok(self
            .conn
            .query_row(
                &format!("SELECT {COLS} FROM items WHERE name = ?1 COLLATE NOCASE ORDER BY id LIMIT 1"),
                params![name],
                map_item,
            )
            .optional()?)
    }

    fn find_by_barcode(&self, barcode: &str) -> Result<Option<Item>, AppError> {
        Ok(self
            .conn
            .query_row(
                &format!("SELECT {COLS} FROM items WHERE barcode = ?1 ORDER BY id LIMIT 1"),
                params![barcode],
                map_item,
            )
            .optional()?)
    }

    fn list_catalog(
        &self,
        category: Option<&str>,
        low_stock_only: bool,
        offset: usize,
        limit: usize,
    ) -> Result<Vec<Item>, AppError> {
        let sql = format!(
            "SELECT {COLS} FROM items WHERE is_active = 1 AND (?1 IS NULL OR category = ?1 COLLATE NOCASE) \
             AND (?2 = 0 OR is_low_stock = 1) ORDER BY name COLLATE NOCASE, id LIMIT ?3 OFFSET ?4"
        );
        let mut stmt = self.conn.prepare_cached(&sql)?;
        let limit = i64::try_from(limit).unwrap_or(i64::MAX);
        let offset = i64::try_from(offset).unwrap_or(i64::MAX);
        let rows = stmt.query_map(params![category, low_stock_only as i64, limit, offset], map_item)?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    fn count_catalog(&self, category: Option<&str>, low_stock_only: bool) -> Result<usize, AppError> {
        let n: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM items WHERE is_active = 1 AND (?1 IS NULL OR category = ?1 COLLATE NOCASE) \
             AND (?2 = 0 OR is_low_stock = 1)",
            params![category, low_stock_only as i64],
            |r| r.get(0),
        )?;
        Ok(n as usize)
    }

    fn list_categories(&self) -> Result<Vec<String>, AppError> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT DISTINCT TRIM(category) FROM items \
             WHERE is_active = 1 AND category IS NOT NULL AND TRIM(category) <> '' \
             ORDER BY TRIM(category) COLLATE NOCASE",
        )?;
        let rows = stmt.query_map([], |r| r.get::<_, String>(0))?.collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    fn create(&mut self, item: &Item) -> Result<i64, AppError> {
        let id = if item.id > 0 { Some(item.id) } else { None };
        let t = enum_to_sql(&item.item_type)?;
        self.conn.execute(
            &format!(
                "INSERT INTO items ({COLS}) VALUES (?1,?2,?3,?4,?5,?6,?7,?8, \
                 (?13 = 'PHYSICAL' AND ?8 < ?14), ?10,?11,?12,?13,?14)"
            ),
            params![
                id, item.name, item.category, item.barcode, item.base_unit, item.mrp,
                item.default_sale_price, item.current_stock, item.is_low_stock, item.is_active,
                item.created_at, item.updated_at, t, item.low_stock_threshold
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    fn update(&mut self, item: &Item) -> Result<(), AppError> {
        let t = enum_to_sql(&item.item_type)?;
        let n = self.conn.execute(
            "UPDATE items SET name=?2, category=?3, barcode=?4, base_unit=?5, mrp=?6, \
             default_sale_price=?7, is_active=?8, updated_at=?9, item_type=?10, low_stock_threshold=?11, \
             is_low_stock = (?10 = 'PHYSICAL' AND current_stock < ?11) WHERE id=?1",
            params![
                item.id, item.name, item.category, item.barcode, item.base_unit, item.mrp,
                item.default_sale_price, item.is_active, item.updated_at, t, item.low_stock_threshold
            ],
        )?;
        if n == 0 { Err(not_found(item.id)) } else { Ok(()) }
    }

    fn update_stock_cache(&mut self, item_id: i64, new_stock: f64) -> Result<(), AppError> {
        let n = self.conn.execute(
            "UPDATE items SET current_stock = ?2, \
             is_low_stock = (item_type = 'PHYSICAL' AND ?2 < low_stock_threshold) WHERE id = ?1",
            params![item_id, new_stock],
        )?;
        if n == 0 { Err(not_found(item_id)) } else { Ok(()) }
    }

    fn apply_stock_delta(&mut self, item_id: i64, delta: f64) -> Result<(), AppError> {
        // SQLite evaluates the right-hand sides against the OLD row values.
        let n = self.conn.execute(
            "UPDATE items SET current_stock = current_stock + ?1, \
             is_low_stock = (item_type = 'PHYSICAL' AND (current_stock + ?1) < low_stock_threshold) WHERE id = ?2",
            params![delta, item_id],
        )?;
        if n == 0 { Err(not_found(item_id)) } else { Ok(()) }
    }

    fn update_low_stock_threshold(&mut self, item_id: i64, threshold: f64, updated_at: &str) -> Result<(), AppError> {
        let n = self.conn.execute(
            "UPDATE items SET low_stock_threshold = ?2, \
             is_low_stock = (item_type = 'PHYSICAL' AND current_stock < ?2), updated_at = ?3 WHERE id = ?1",
            params![item_id, threshold, updated_at],
        )?;
        if n == 0 { Err(not_found(item_id)) } else { Ok(()) }
    }

    fn recompute_low_stock_flags(&mut self) -> Result<usize, AppError> {
        Ok(self.conn.execute(
            &format!("UPDATE items SET is_low_stock = {LOW_STOCK_SQL} WHERE is_low_stock <> {LOW_STOCK_SQL}"),
            [],
        )?)
    }

    fn stock_cache_audit(&self) -> Result<Vec<(i64, f64, f64)>, AppError> {
        let mut stmt = self.conn.prepare(
            "SELECT i.id, i.current_stock, \
                    COALESCE((SELECT SUM(p.remaining_quantity) FROM purchase_items p WHERE p.item_id = i.id), 0.0) \
             FROM items i ORDER BY i.id",
        )?;
        let rows = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }
}
