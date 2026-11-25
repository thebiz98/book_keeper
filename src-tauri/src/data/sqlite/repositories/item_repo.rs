//! SQLite implementation of `ItemRepository`.
use rusqlite::{params, OptionalExtension, Row};

use crate::core::dtos::DeletedItemRow;
use crate::core::entities::Item;
use crate::core::money::round2;
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

const COLS: &str = "id, name, category, barcode, base_unit, mrp, default_sale_price, current_stock, is_low_stock, is_active, created_at, updated_at, item_type, low_stock_threshold, service_cost";

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
        service_cost: r.get(14)?,
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
                 (?13 = 'PHYSICAL' AND ?8 < ?14), ?10,?11,?12,?13,?14,?15)"
            ),
            params![
                id, item.name, item.category, item.barcode, item.base_unit, item.mrp,
                item.default_sale_price, item.current_stock, item.is_low_stock, item.is_active,
                item.created_at, item.updated_at, t, item.low_stock_threshold, item.service_cost
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    fn update(&mut self, item: &Item) -> Result<(), AppError> {
        let t = enum_to_sql(&item.item_type)?;
        let n = self.conn.execute(
            "UPDATE items SET name=?2, category=?3, barcode=?4, base_unit=?5, mrp=?6, \
             default_sale_price=?7, is_active=?8, updated_at=?9, item_type=?10, low_stock_threshold=?11, service_cost=?12, \
             is_low_stock = (?10 = 'PHYSICAL' AND current_stock < ?11) WHERE id=?1",
            params![
                item.id, item.name, item.category, item.barcode, item.base_unit, item.mrp,
                item.default_sale_price, item.is_active, item.updated_at, t, item.low_stock_threshold,
                item.service_cost
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

    fn set_active(&mut self, id: i64, active: bool, updated_at: &str) -> Result<(), AppError> {
        let n = self.conn.execute(
            "UPDATE items SET is_active = ?2, updated_at = ?3 WHERE id = ?1",
            params![id, active as i64, updated_at],
        )?;
        if n == 0 { Err(not_found(id)) } else { Ok(()) }
    }

    fn list_inactive(&self, search: &str, offset: usize, limit: usize) -> Result<(Vec<DeletedItemRow>, usize), AppError> {
        let like = format!(
            "%{}%",
            search.trim().replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_")
        );
        let total: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM items WHERE is_active = 0 AND name LIKE ?1 ESCAPE '\\'",
            params![like],
            |r| r.get(0),
        )?;
        let mut stmt = self.conn.prepare_cached(
            "SELECT i.id, i.name, i.item_type, i.category, i.base_unit, i.current_stock, \
                    COALESCE(s.v, 0.0), i.updated_at \
             FROM items i \
             LEFT JOIN (SELECT item_id, SUM(remaining_quantity * unit_cost) AS v FROM purchase_items \
                        WHERE remaining_quantity > 0 GROUP BY item_id) s ON s.item_id = i.id \
             WHERE i.is_active = 0 AND i.name LIKE ?1 ESCAPE '\\' \
             ORDER BY i.updated_at DESC, i.id LIMIT ?2 OFFSET ?3",
        )?;
        let limit = i64::try_from(limit).unwrap_or(i64::MAX);
        let offset = i64::try_from(offset).unwrap_or(i64::MAX);
        let rows = stmt
            .query_map(params![like, limit, offset], |r| {
                Ok(DeletedItemRow {
                    item_id: r.get(0)?,
                    name: r.get(1)?,
                    item_type: enum_col(r, 2)?,
                    category: r.get(3)?,
                    base_unit: r.get(4)?,
                    current_stock: r.get(5)?,
                    stock_value: round2(r.get(6)?),
                    deleted_at: r.get(7)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok((rows, total.max(0) as usize))
    }

    fn services_using_active(&self, component_id: i64) -> Result<Vec<String>, AppError> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT s.name FROM service_components c JOIN items s ON s.id = c.service_item_id \
             WHERE c.component_item_id = ?1 AND s.is_active = 1 ORDER BY s.name COLLATE NOCASE, s.id",
        )?;
        let rows = stmt.query_map(params![component_id], |r| r.get::<_, String>(0))?.collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    fn usage_reasons(&self, item_id: i64) -> Result<Vec<String>, AppError> {
        let c: [i64; 6] = self.conn.query_row(
            "SELECT \
               (SELECT COUNT(DISTINCT sale_transaction_id) FROM sale_items WHERE item_id = ?1), \
               (SELECT COUNT(*) FROM return_items WHERE item_id = ?1), \
               (SELECT COUNT(*) FROM stock_adjustments WHERE item_id = ?1), \
               (SELECT COUNT(*) FROM purchase_items p JOIN purchase_transactions t ON t.id = p.purchase_transaction_id \
                 WHERE p.item_id = ?1 AND t.source <> 'OPENING'), \
               (SELECT COUNT(*) FROM purchase_items p JOIN purchase_transactions t ON t.id = p.purchase_transaction_id \
                 WHERE p.item_id = ?1 AND t.source = 'OPENING' AND (p.remaining_quantity < p.quantity \
                   OR EXISTS (SELECT 1 FROM sale_stock_allocations a WHERE a.purchase_item_id = p.id) \
                   OR EXISTS (SELECT 1 FROM stock_outflow_allocations o WHERE o.purchase_item_id = p.id))), \
               (SELECT COUNT(*) FROM service_components WHERE component_item_id = ?1)",
            params![item_id],
            |r| Ok([r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?]),
        )?;
        let mut out = Vec::new();
        if c[0] > 0 {
            out.push(format!("sold in {} bill(s)", c[0]));
        }
        if c[1] > 0 {
            out.push(format!("in {} return line(s)", c[1]));
        }
        if c[2] > 0 {
            out.push(format!("{} stock adjustment(s)", c[2]));
        }
        if c[3] > 0 {
            out.push(format!("{} purchase or adjustment lot(s)", c[3]));
        }
        if c[4] > 0 {
            out.push("its opening stock was used".to_string());
        }
        if c[5] > 0 {
            out.push(format!("a component of {} service(s)", c[5]));
        }
        Ok(out)
    }

    fn hard_delete(&mut self, item_id: i64) -> Result<(), AppError> {
        let mut stmt = self.conn.prepare(
            "SELECT DISTINCT p.purchase_transaction_id FROM purchase_items p \
             JOIN purchase_transactions t ON t.id = p.purchase_transaction_id \
             WHERE p.item_id = ?1 AND t.source = 'OPENING'",
        )?;
        let headers = stmt.query_map(params![item_id], |r| r.get::<_, i64>(0))?.collect::<Result<Vec<_>, _>>()?;
        drop(stmt);
        self.conn.execute(
            "DELETE FROM purchase_items WHERE item_id = ?1 AND purchase_transaction_id IN \
             (SELECT id FROM purchase_transactions WHERE source = 'OPENING')",
            params![item_id],
        )?;
        for h in headers {
            // Imported lots share one OPN- header per date: drop it only when it is left empty.
            self.conn.execute(
                "DELETE FROM purchase_transactions WHERE id = ?1 AND source = 'OPENING' \
                 AND NOT EXISTS (SELECT 1 FROM purchase_items WHERE purchase_transaction_id = ?1)",
                params![h],
            )?;
        }
        self.conn.execute("DELETE FROM item_bundles WHERE item_id = ?1", params![item_id])?;
        self.conn.execute("DELETE FROM service_components WHERE service_item_id = ?1", params![item_id])?;
        let n = self.conn.execute("DELETE FROM items WHERE id = ?1", params![item_id])?;
        if n == 0 { Err(not_found(item_id)) } else { Ok(()) }
    }

    fn stock_value(&self, item_id: i64) -> Result<f64, AppError> {
        let v: f64 = self.conn.query_row(
            "SELECT COALESCE(SUM(remaining_quantity * unit_cost), 0.0) FROM purchase_items \
             WHERE item_id = ?1 AND remaining_quantity > 0",
            params![item_id],
            |r| r.get(0),
        )?;
        Ok(round2(v))
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
