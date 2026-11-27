//! SQLite implementation of `BillChargeRepository`.
use rusqlite::{params, Row};

use crate::core::entities::BillCharge;
use crate::core::error::AppError;
use crate::core::value_objects::TransactionType;
use crate::data::contracts::bill_charge_repository::BillChargeRepository;
use crate::data::sqlite::{enum_col, enum_to_sql, SqliteConn};

const COLS: &str = "id, transaction_type, transaction_id, charge_type_id, charge_name, effect, amount";

fn map(r: &Row<'_>) -> rusqlite::Result<BillCharge> {
    Ok(BillCharge {
        id: r.get(0)?,
        transaction_type: enum_col(r, 1)?,
        transaction_id: r.get(2)?,
        charge_type_id: r.get(3)?,
        charge_name: r.get(4)?,
        effect: enum_col(r, 5)?,
        amount: r.get(6)?,
    })
}

pub struct SqliteBillChargeRepository {
    conn: SqliteConn,
}

impl SqliteBillChargeRepository {
    pub fn new(conn: SqliteConn) -> Self {
        Self { conn }
    }
}

impl BillChargeRepository for SqliteBillChargeRepository {
    fn create(&mut self, c: &BillCharge) -> Result<i64, AppError> {
        let id = if c.id > 0 { Some(c.id) } else { None };
        self.conn.execute(
            &format!("INSERT INTO bill_charges ({COLS}) VALUES (?1,?2,?3,?4,?5,?6,?7)"),
            params![
                id, enum_to_sql(&c.transaction_type)?, c.transaction_id, c.charge_type_id, c.charge_name,
                enum_to_sql(&c.effect)?, c.amount
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    fn list_by_transaction(&self, t: TransactionType, id: i64) -> Result<Vec<BillCharge>, AppError> {
        let mut stmt = self.conn.prepare_cached(&format!(
            "SELECT {COLS} FROM bill_charges WHERE transaction_type = ?1 AND transaction_id = ?2 ORDER BY id"
        ))?;
        let rows = stmt.query_map(params![enum_to_sql(&t)?, id], map)?.collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    fn delete_by_id(&mut self, id: i64) -> Result<(), AppError> {
        self.conn.execute("DELETE FROM bill_charges WHERE id = ?1", params![id])?;
        Ok(())
    }

    fn delete_by_transaction(&mut self, t: TransactionType, id: i64) -> Result<usize, AppError> {
        Ok(self.conn.execute(
            "DELETE FROM bill_charges WHERE transaction_type = ?1 AND transaction_id = ?2",
            params![enum_to_sql(&t)?, id],
        )?)
    }
}
