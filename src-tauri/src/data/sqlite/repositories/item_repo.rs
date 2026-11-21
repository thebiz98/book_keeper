//! SQLite implementation of `ItemRepository`.
use rusqlite::{params, OptionalExtension, Row};

use crate::core::entities::Item;
use crate::core::error::AppError;
use crate::data::contracts::ItemRepository;
use crate::data::sqlite::SqliteConn;

pub struct SqliteItemRepository {
    conn: SqliteConn,
}

impl SqliteItemRepository {
    pub fn new(conn: SqliteConn) -> Self {
        Self { conn }
    }
}

const COLS: &str = "id, name, category, barcode, base_unit, mrp, default_sale_price, current_stock, is_low_stock, is_active, created_at, updated_at";

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
            "SELECT {COLS} FROM items WHERE is_active = 1 AND (?1 IS NULL OR category = ?1) \
             AND (?2 = 0 OR is_low_stock = 1) ORDER BY name COLLATE NOCASE, id LIMIT ?3 OFFSET ?4"
        );
        let mut stmt = self.conn.prepare_cached(&sql)?;
        let limit = i64::try_from(limit).unwrap_or(i64::MAX);
        let offset = i64::try_from(offset).unwrap_or(i64::MAX);
        let rows = stmt.query_map(params![category, low_stock_only as i64, limit, offset], map_item)?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    fn create(&mut self, item: &Item) -> Result<i64, AppError> {
        let id = if item.id > 0 { Some(item.id) } else { None };
        self.conn.execute(
            &format!("INSERT INTO items ({COLS}) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)"),
            params![
                id, item.name, item.category, item.barcode, item.base_unit, item.mrp,
                item.default_sale_price, item.current_stock, item.is_low_stock, item.is_active,
                item.created_at, item.updated_at
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    fn update(&mut self, item: &Item) -> Result<(), AppError> {
        let n = self.conn.execute(
            "UPDATE items SET name=?2, category=?3, barcode=?4, base_unit=?5, mrp=?6, default_sale_price=?7, \
             current_stock=?8, is_low_stock=?9, is_active=?10, created_at=?11, updated_at=?12 WHERE id=?1",
            params![
                item.id, item.name, item.category, item.barcode, item.base_unit, item.mrp,
                item.default_sale_price, item.current_stock, item.is_low_stock, item.is_active,
                item.created_at, item.updated_at
            ],
        )?;
        if n == 0 { Err(not_found(item.id)) } else { Ok(()) }
    }

    fn update_stock_cache(&mut self, item_id: i64, new_stock: f64, is_low_stock: bool) -> Result<(), AppError> {
        let n = self.conn.execute(
            "UPDATE items SET current_stock = ?2, is_low_stock = ?3 WHERE id = ?1",
            params![item_id, new_stock, is_low_stock],
        )?;
        if n == 0 { Err(not_found(item_id)) } else { Ok(()) }
    }
}
