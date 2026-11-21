//! SQLite implementation of `OutflowAllocationRepository`.
use rusqlite::{params, Row};

use crate::core::entities::StockOutflowAllocation;
use crate::core::error::AppError;
use crate::core::value_objects::OutflowSourceType;
use crate::data::contracts::outflow_allocation_repository::OutflowAllocationRepository;
use crate::data::sqlite::{enum_col, enum_to_sql, SqliteConn};

const COLS: &str = "id, source_type, source_id, purchase_item_id, quantity_depleted, unit_cost";

fn map(r: &Row<'_>) -> rusqlite::Result<StockOutflowAllocation> {
    Ok(StockOutflowAllocation {
        id: r.get(0)?,
        source_type: enum_col(r, 1)?,
        source_id: r.get(2)?,
        purchase_item_id: r.get(3)?,
        quantity_depleted: r.get(4)?,
        unit_cost: r.get(5)?,
    })
}

pub struct SqliteOutflowAllocationRepository {
    conn: SqliteConn,
}

impl SqliteOutflowAllocationRepository {
    pub fn new(conn: SqliteConn) -> Self {
        Self { conn }
    }
}

impl OutflowAllocationRepository for SqliteOutflowAllocationRepository {
    fn create_allocation(&mut self, a: &StockOutflowAllocation) -> Result<i64, AppError> {
        self.conn.execute(
            &format!("INSERT INTO stock_outflow_allocations ({COLS}) VALUES (?1,?2,?3,?4,?5,?6)"),
            params![
                if a.id > 0 { Some(a.id) } else { None },
                enum_to_sql(&a.source_type)?, a.source_id, a.purchase_item_id, a.quantity_depleted, a.unit_cost
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    fn find_by_source(
        &self,
        source_type: OutflowSourceType,
        source_id: i64,
    ) -> Result<Vec<StockOutflowAllocation>, AppError> {
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {COLS} FROM stock_outflow_allocations WHERE source_type = ?1 AND source_id = ?2 ORDER BY id"
        ))?;
        let rows = stmt
            .query_map(params![enum_to_sql(&source_type)?, source_id], map)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }
}
