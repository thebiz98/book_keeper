//! SQLite implementation of `AdjustmentAllocationRepository`.
use rusqlite::{params, Row};

use crate::core::entities::AdjustmentStockAllocation;
use crate::core::error::AppError;
use crate::data::contracts::adjustment_allocation_repository::AdjustmentAllocationRepository;
use crate::data::sqlite::SqliteConn;

const COLS: &str = "id, stock_adjustment_id, purchase_item_id, quantity_depleted, unit_cost";

fn map(r: &Row<'_>) -> rusqlite::Result<AdjustmentStockAllocation> {
    Ok(AdjustmentStockAllocation {
        id: r.get(0)?,
        stock_adjustment_id: r.get(1)?,
        purchase_item_id: r.get(2)?,
        quantity_depleted: r.get(3)?,
        unit_cost: r.get(4)?,
    })
}

pub struct SqliteAdjustmentAllocationRepository {
    conn: SqliteConn,
}

impl SqliteAdjustmentAllocationRepository {
    pub fn new(conn: SqliteConn) -> Self {
        Self { conn }
    }

    fn query(&self, sql: &str, p: impl rusqlite::Params) -> Result<Vec<AdjustmentStockAllocation>, AppError> {
        let mut stmt = self.conn.prepare(sql)?;
        let rows = stmt.query_map(p, map)?.collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }
}

impl AdjustmentAllocationRepository for SqliteAdjustmentAllocationRepository {
    fn create_allocation(&mut self, a: &AdjustmentStockAllocation) -> Result<i64, AppError> {
        self.conn.execute(
            &format!("INSERT INTO adjustment_stock_allocations ({COLS}) VALUES (?1,?2,?3,?4,?5)"),
            params![
                if a.id > 0 { Some(a.id) } else { None },
                a.stock_adjustment_id, a.purchase_item_id, a.quantity_depleted, a.unit_cost
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    fn create(&mut self, a: &AdjustmentStockAllocation) -> Result<i64, AppError> {
        self.create_allocation(a)
    }

    fn find_by_adjustment_id(&self, stock_adjustment_id: i64) -> Result<Vec<AdjustmentStockAllocation>, AppError> {
        self.query(
            &format!("SELECT {COLS} FROM adjustment_stock_allocations WHERE stock_adjustment_id = ?1 ORDER BY id"),
            params![stock_adjustment_id],
        )
    }

    fn list_by_adjustment_id(&self, stock_adjustment_id: i64) -> Result<Vec<AdjustmentStockAllocation>, AppError> {
        self.find_by_adjustment_id(stock_adjustment_id)
    }
}
