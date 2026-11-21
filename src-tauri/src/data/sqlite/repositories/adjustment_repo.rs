//! SQLite implementation of `AdjustmentRepository`.
use rusqlite::{params, Row};

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

    fn create(&mut self, a: &StockAdjustment) -> Result<i64, AppError> {
        self.create_adjustment(a)
    }

    fn list_adjustments(
        &self,
        item_id: Option<i64>,
        offset: usize,
        limit: usize,
    ) -> Result<Vec<StockAdjustment>, AppError> {
        let lim = limit.min(i64::MAX as usize) as i64;
        let off = offset.min(i64::MAX as usize) as i64;
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {COLS} FROM stock_adjustments WHERE (?1 IS NULL OR item_id = ?1) ORDER BY id LIMIT ?2 OFFSET ?3"
        ))?;
        let rows = stmt.query_map(params![item_id, lim, off], map)?.collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    fn list_by_item_id(&self, item_id: i64) -> Result<Vec<StockAdjustment>, AppError> {
        self.list_adjustments(Some(item_id), 0, usize::MAX)
    }
}
