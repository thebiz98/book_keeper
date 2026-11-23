//! SQLite implementation of `ChargeTypeRepository`.
use rusqlite::{params, OptionalExtension, Row};

use crate::core::entities::ChargeType;
use crate::core::error::AppError;
use crate::core::value_objects::TransactionType;
use crate::data::contracts::charge_type_repository::ChargeTypeRepository;
use crate::data::sqlite::{enum_col, enum_to_sql, SqliteConn};

const COLS: &str = "id, name, effect, applies_to, default_amount, is_active, created_at";

fn map(r: &Row<'_>) -> rusqlite::Result<ChargeType> {
    Ok(ChargeType {
        id: r.get(0)?,
        name: r.get(1)?,
        effect: enum_col(r, 2)?,
        applies_to: enum_col(r, 3)?,
        default_amount: r.get(4)?,
        is_active: r.get(5)?,
        created_at: r.get(6)?,
    })
}

pub struct SqliteChargeTypeRepository {
    conn: SqliteConn,
}

impl SqliteChargeTypeRepository {
    pub fn new(conn: SqliteConn) -> Self {
        Self { conn }
    }
}

impl ChargeTypeRepository for SqliteChargeTypeRepository {
    fn find_by_id(&self, id: i64) -> Result<Option<ChargeType>, AppError> {
        Ok(self
            .conn
            .query_row(&format!("SELECT {COLS} FROM charge_types WHERE id = ?1"), [id], map)
            .optional()?)
    }

    fn find_by_name(&self, name: &str) -> Result<Option<ChargeType>, AppError> {
        Ok(self
            .conn
            .query_row(&format!("SELECT {COLS} FROM charge_types WHERE name = ?1 COLLATE NOCASE"), [name], map)
            .optional()?)
    }

    fn create(&mut self, ct: &ChargeType) -> Result<i64, AppError> {
        let id = if ct.id > 0 { Some(ct.id) } else { None };
        self.conn.execute(
            &format!("INSERT INTO charge_types ({COLS}) VALUES (?1,?2,?3,?4,?5,?6,?7)"),
            params![
                id, ct.name, enum_to_sql(&ct.effect)?, enum_to_sql(&ct.applies_to)?, ct.default_amount,
                ct.is_active, ct.created_at
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    fn list_by_ids_for_side(&self, ids: &[i64], side: TransactionType) -> Result<Vec<ChargeType>, AppError> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let marks = vec!["?"; ids.len()].join(",");
        let sql = format!(
            "SELECT {COLS} FROM charge_types WHERE is_active = 1 AND applies_to IN (?, 'BOTH') AND id IN ({marks})"
        );
        let mut args: Vec<rusqlite::types::Value> = vec![enum_to_sql(&side)?.into()];
        args.extend(ids.iter().map(|i| rusqlite::types::Value::Integer(*i)));
        let mut stmt = self.conn.prepare(&sql)?;
        let mut rows = stmt.query_map(rusqlite::params_from_iter(args.iter()), map)?.collect::<Result<Vec<_>, _>>()?;
        rows.sort_by_key(|c| ids.iter().position(|i| *i == c.id).unwrap_or(usize::MAX));
        Ok(rows)
    }

    fn list_for_side(&self, side: TransactionType, limit: usize) -> Result<Vec<ChargeType>, AppError> {
        let mut stmt = self.conn.prepare_cached(&format!(
            "SELECT {COLS} FROM charge_types WHERE is_active = 1 AND applies_to IN (?1, 'BOTH') \
             ORDER BY name COLLATE NOCASE, id LIMIT ?2"
        ))?;
        let rows = stmt
            .query_map(params![enum_to_sql(&side)?, limit.min(i64::MAX as usize) as i64], map)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }
}
