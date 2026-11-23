//! SQLite implementation of `AllocationRepository`.
use rusqlite::{params, OptionalExtension, Row};

use crate::core::entities::SaleStockAllocation;
use crate::core::error::AppError;
use crate::data::contracts::allocation_repository::AllocationRepository;
use crate::data::sqlite::SqliteConn;

const COLS: &str = "id, sale_item_id, purchase_item_id, quantity_depleted, returned_quantity, unit_cost";

fn map(r: &Row<'_>) -> rusqlite::Result<SaleStockAllocation> {
    Ok(SaleStockAllocation {
        id: r.get(0)?,
        sale_item_id: r.get(1)?,
        purchase_item_id: r.get(2)?,
        quantity_depleted: r.get(3)?,
        returned_quantity: r.get(4)?,
        unit_cost: r.get(5)?,
    })
}

pub struct SqliteAllocationRepository {
    conn: SqliteConn,
}

impl SqliteAllocationRepository {
    pub fn new(conn: SqliteConn) -> Self {
        Self { conn }
    }

    fn query(&self, sql: &str, p: impl rusqlite::Params) -> Result<Vec<SaleStockAllocation>, AppError> {
        let mut stmt = self.conn.prepare(sql)?;
        let rows = stmt.query_map(p, map)?.collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }
}

impl AllocationRepository for SqliteAllocationRepository {
    fn delete_by_sale_item_id(&mut self, sale_item_id: i64) -> Result<usize, AppError> {
        Ok(self.conn.execute("DELETE FROM sale_stock_allocations WHERE sale_item_id = ?1", params![sale_item_id])?)
    }

    fn sale_item_ids_for_lot(&self, lot_id: i64) -> Result<Vec<i64>, AppError> {
        let mut stmt = self.conn.prepare(
            "SELECT DISTINCT sale_item_id FROM sale_stock_allocations WHERE purchase_item_id = ?1 ORDER BY sale_item_id",
        )?;
        let rows = stmt.query_map(params![lot_id], |r| r.get(0))?.collect::<Result<Vec<i64>, _>>()?;
        Ok(rows)
    }

    fn update_unit_cost_for_lot(&mut self, lot_id: i64, unit_cost: f64) -> Result<usize, AppError> {
        Ok(self.conn.execute(
            "UPDATE sale_stock_allocations SET unit_cost = ?2 WHERE purchase_item_id = ?1",
            params![lot_id, unit_cost],
        )?)
    }

    fn create_allocation(&mut self, a: &SaleStockAllocation) -> Result<i64, AppError> {
        self.conn.execute(
            &format!("INSERT INTO sale_stock_allocations ({COLS}) VALUES (?1,?2,?3,?4,?5,?6)"),
            params![
                if a.id > 0 { Some(a.id) } else { None },
                a.sale_item_id, a.purchase_item_id, a.quantity_depleted, a.returned_quantity, a.unit_cost
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    fn find_by_sale_item_id(&self, sale_item_id: i64) -> Result<Vec<SaleStockAllocation>, AppError> {
        self.query(
            &format!("SELECT {COLS} FROM sale_stock_allocations WHERE sale_item_id = ?1 ORDER BY id"),
            params![sale_item_id],
        )
    }

    fn find_reversible_by_sale_item_id(&self, sale_item_id: i64) -> Result<Vec<SaleStockAllocation>, AppError> {
        self.query(
            &format!(
                "SELECT {COLS} FROM sale_stock_allocations \
                 WHERE sale_item_id = ?1 AND quantity_depleted > returned_quantity ORDER BY id DESC"
            ),
            params![sale_item_id],
        )
    }

    fn update_returned_quantity(&mut self, allocation_id: i64, new_returned_qty: f64) -> Result<(), AppError> {
        const EPS: f64 = 1e-9;
        let depleted: f64 = self
            .conn
            .query_row(
                "SELECT quantity_depleted FROM sale_stock_allocations WHERE id = ?1",
                params![allocation_id],
                |r| r.get(0),
            )
            .optional()?
            .ok_or_else(|| AppError::NotFound {
                entity: "SaleStockAllocation".to_string(),
                id: allocation_id.to_string(),
            })?;
        if !new_returned_qty.is_finite() || new_returned_qty < -EPS || new_returned_qty > depleted + EPS {
            return Err(AppError::ValidationError {
                field: "returned_quantity".to_string(),
                message: format!(
                    "Returned quantity {new_returned_qty} is outside 0..={depleted} for allocation #{allocation_id}"
                ),
            });
        }
        self.conn.execute(
            "UPDATE sale_stock_allocations SET returned_quantity = ?1 WHERE id = ?2",
            params![new_returned_qty.clamp(0.0, depleted), allocation_id],
        )?;
        Ok(())
    }

    fn list_all(&self) -> Result<Vec<SaleStockAllocation>, AppError> {
        self.query(&format!("SELECT {COLS} FROM sale_stock_allocations ORDER BY id"), [])
    }
}
