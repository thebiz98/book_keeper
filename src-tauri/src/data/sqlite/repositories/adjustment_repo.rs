//! SQLite implementation of `AdjustmentRepository`.
use rusqlite::{params, OptionalExtension, Row};

use crate::core::dtos::AdjustmentListRow;
use crate::core::money::round2;
use crate::core::entities::StockAdjustment;
use crate::core::error::AppError;
use crate::data::contracts::adjustment_repository::AdjustmentRepository;
use crate::data::sqlite::{enum_col, enum_to_sql, SqliteConn};

const COLS: &str = "id, adjustment_date, item_id, quantity, unit_cost, reason, notes, created_at";

fn map(r: &Row<'_>) -> rusqlite::Result<StockAdjustment> {
    Ok(StockAdjustment {
        id: r.get(0)?,
        adjustment_date: r.get(1)?,
        item_id: r.get(2)?,
        quantity: r.get(3)?,
        unit_cost: r.get(4)?,
        reason: enum_col(r, 5)?,
        notes: r.get(6)?,
        created_at: r.get(7)?,
    })
}

pub struct SqliteAdjustmentRepository {
    conn: SqliteConn,
}

impl SqliteAdjustmentRepository {
    pub fn new(conn: SqliteConn) -> Self {
        Self { conn }
    }
}

impl AdjustmentRepository for SqliteAdjustmentRepository {
    fn create_adjustment(&mut self, a: &StockAdjustment) -> Result<i64, AppError> {
        self.conn.execute(
            &format!("INSERT INTO stock_adjustments ({COLS}) VALUES (?1,?2,?3,?4,?5,?6,?7,?8)"),
            params![
                if a.id > 0 { Some(a.id) } else { None },
                a.adjustment_date, a.item_id, a.quantity, a.unit_cost, enum_to_sql(&a.reason)?, a.notes, a.created_at
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    fn update_unit_cost(&mut self, id: i64, unit_cost: f64) -> Result<(), AppError> {
        self.conn.execute("UPDATE stock_adjustments SET unit_cost = ?1 WHERE id = ?2", params![unit_cost, id])?;
        Ok(())
    }

    fn find_by_id(&self, id: i64) -> Result<Option<StockAdjustment>, AppError> {
        Ok(self
            .conn
            .query_row(&format!("SELECT {COLS} FROM stock_adjustments WHERE id = ?1"), params![id], map)
            .optional()?)
    }

    fn delete_adjustment(&mut self, id: i64) -> Result<(), AppError> {
        self.conn.execute("DELETE FROM stock_adjustments WHERE id = ?1", params![id])?;
        Ok(())
    }

    fn list_adjustments(
        &self,
        item_id: Option<i64>,
        offset: usize,
        limit: usize,
    ) -> Result<(Vec<StockAdjustment>, usize), AppError> {
        let lim = limit.min(i64::MAX as usize) as i64;
        let off = offset.min(i64::MAX as usize) as i64;
        let total: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM stock_adjustments WHERE (?1 IS NULL OR item_id = ?1)",
            params![item_id],
            |r| r.get(0),
        )?;
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {COLS} FROM stock_adjustments WHERE (?1 IS NULL OR item_id = ?1) \
             ORDER BY adjustment_date DESC, id DESC LIMIT ?2 OFFSET ?3"
        ))?;
        let rows = stmt.query_map(params![item_id, lim, off], map)?.collect::<Result<Vec<_>, _>>()?;
        Ok((rows, total as usize))
    }

    fn list_adjustment_rows(
        &self,
        item_id: Option<i64>,
        offset: usize,
        limit: usize,
    ) -> Result<(Vec<AdjustmentListRow>, usize), AppError> {
        let lim = limit.min(i64::MAX as usize) as i64;
        let off = offset.min(i64::MAX as usize) as i64;
        let total: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM stock_adjustments WHERE (?1 IS NULL OR item_id = ?1)",
            params![item_id],
            |r| r.get(0),
        )?;
        let mut stmt = self.conn.prepare(
            "SELECT j.id, j.adjustment_date, j.created_at, j.item_id, i.name, i.base_unit, i.is_active,                     j.quantity, j.reason, j.unit_cost, j.notes, COALESCE(o.loss, 0)              FROM stock_adjustments j JOIN items i ON i.id = j.item_id              LEFT JOIN (SELECT source_id, SUM(quantity_depleted * unit_cost) AS loss                         FROM stock_outflow_allocations WHERE source_type = 'ADJUSTMENT' GROUP BY source_id) o                     ON o.source_id = j.id              WHERE (?1 IS NULL OR j.item_id = ?1)              ORDER BY j.adjustment_date DESC, j.id DESC LIMIT ?2 OFFSET ?3",
        )?;
        let rows = stmt
            .query_map(params![item_id, lim, off], |r| {
                let quantity: f64 = r.get(7)?;
                let unit_cost: Option<f64> = r.get(9)?;
                let loss: f64 = r.get(11)?;
                let value = if quantity < 0.0 { loss } else { quantity * unit_cost.unwrap_or(0.0) };
                Ok(AdjustmentListRow {
                    id: r.get(0)?,
                    adjustment_date: r.get(1)?,
                    created_at: r.get(2)?,
                    item_id: r.get(3)?,
                    item_name: r.get(4)?,
                    base_unit: r.get(5)?,
                    item_is_active: r.get(6)?,
                    quantity,
                    reason: enum_col(r, 8)?,
                    unit_cost,
                    notes: r.get(10)?,
                    value: round2(value),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok((rows, total as usize))
    }
}
