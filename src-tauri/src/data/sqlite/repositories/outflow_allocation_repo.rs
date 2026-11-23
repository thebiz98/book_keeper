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
    fn delete_by_source(
        &mut self,
        source_type: OutflowSourceType,
        source_id: i64,
    ) -> Result<Vec<StockOutflowAllocation>, AppError> {
        let rows = self.find_by_source(source_type, source_id)?;
        self.conn.execute(
            "DELETE FROM stock_outflow_allocations WHERE source_type = ?1 AND source_id = ?2",
            params![enum_to_sql(&source_type)?, source_id],
        )?;
        Ok(rows)
    }

    fn exists_for_lot(&self, lot_id: i64, source_type: OutflowSourceType) -> Result<bool, AppError> {
        Ok(self.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM stock_outflow_allocations WHERE purchase_item_id = ?1 AND source_type = ?2)",
            params![lot_id, enum_to_sql(&source_type)?],
            |r| r.get::<_, bool>(0),
        )?)
    }

    fn source_ids_for_lot(&self, lot_id: i64, source_type: OutflowSourceType) -> Result<Vec<i64>, AppError> {
        let mut stmt = self.conn.prepare(
            "SELECT DISTINCT source_id FROM stock_outflow_allocations WHERE purchase_item_id = ?1 AND source_type = ?2 ORDER BY source_id",
        )?;
        let rows = stmt
            .query_map(params![lot_id, enum_to_sql(&source_type)?], |r| r.get(0))?
            .collect::<Result<Vec<i64>, _>>()?;
        Ok(rows)
    }

    fn update_unit_cost_for_lot(&mut self, lot_id: i64, unit_cost: f64) -> Result<usize, AppError> {
        Ok(self.conn.execute(
            "UPDATE stock_outflow_allocations SET unit_cost = ?2 WHERE purchase_item_id = ?1 \
             AND source_type IN ('ADJUSTMENT', 'REPLACEMENT')",
            params![lot_id, unit_cost],
        )?)
    }

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
