//! SQLite implementation of `AllocationRepository`.
use rusqlite::{params, Row};

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

    fn create(&mut self, a: &SaleStockAllocation) -> Result<i64, AppError> {
        self.create_allocation(a)
    }

    fn find_by_sale_item_id(&self, sale_item_id: i64) -> Result<Vec<SaleStockAllocation>, AppError> {
        self.query(
            &format!("SELECT {COLS} FROM sale_stock_allocations WHERE sale_item_id = ?1 ORDER BY id"),
            params![sale_item_id],
        )
    }

    fn list_by_sale_item_id(&self, sale_item_id: i64) -> Result<Vec<SaleStockAllocation>, AppError> {
        self.find_by_sale_item_id(sale_item_id)
    }

    fn list_all(&self) -> Result<Vec<SaleStockAllocation>, AppError> {
        self.query(&format!("SELECT {COLS} FROM sale_stock_allocations ORDER BY id"), [])
    }
}
