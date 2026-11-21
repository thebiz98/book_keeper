//! SQLite implementation of `ReturnRepository`.
use rusqlite::{params, OptionalExtension, Row};

use crate::core::dtos::ReturnsFilter;
use crate::core::entities::{PaymentSettlement, ReturnItem, ReturnTransaction};
use crate::core::error::AppError;
use crate::core::value_objects::ReturnType;
use crate::data::contracts::return_repository::ReturnRepository;
use crate::data::sqlite::code_gen::{next_code, RETURN_CODE};
use crate::data::sqlite::{enum_col, enum_to_sql, opt_enum_col, SqliteConn};

const TX_COLS: &str = "id, return_code, return_date, return_type, party_id, party_name, original_transaction_id, settlement_type, total_amount, reason, notes, created_at";
const ITEM_COLS: &str = "id, return_transaction_id, line_type, item_id, item_name, sale_item_id, purchase_item_id, quantity, unit_price, total_amount, cost_amount, condition";

fn map_tx(r: &Row<'_>) -> rusqlite::Result<ReturnTransaction> {
    Ok(ReturnTransaction {
        id: r.get(0)?,
        return_code: r.get(1)?,
        return_date: r.get(2)?,
        return_type: enum_col(r, 3)?,
        party_id: r.get(4)?,
        party_name: r.get(5)?,
        original_transaction_id: r.get(6)?,
        settlement_type: enum_col(r, 7)?,
        total_amount: r.get(8)?,
        reason: r.get(9)?,
        notes: r.get(10)?,
        created_at: r.get(11)?,
    })
}

fn map_item(r: &Row<'_>) -> rusqlite::Result<ReturnItem> {
    Ok(ReturnItem {
        id: r.get(0)?,
        return_transaction_id: r.get(1)?,
        line_type: enum_col(r, 2)?,
        item_id: r.get(3)?,
        item_name: r.get(4)?,
        sale_item_id: r.get(5)?,
        purchase_item_id: r.get(6)?,
        quantity: r.get(7)?,
        unit_price: r.get(8)?,
        total_amount: r.get(9)?,
        cost_amount: r.get(10)?,
        condition: opt_enum_col(r, 11)?,
    })
}

pub struct SqliteReturnRepository {
    conn: SqliteConn,
}

impl SqliteReturnRepository {
    pub fn new(conn: SqliteConn) -> Self {
        Self { conn }
    }
}

impl ReturnRepository for SqliteReturnRepository {
    fn create_return_transaction(&mut self, t: &ReturnTransaction) -> Result<i64, AppError> {
        self.conn.execute(
            &format!("INSERT INTO return_transactions ({TX_COLS}) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)"),
            params![
                if t.id > 0 { Some(t.id) } else { None },
                t.return_code, t.return_date, enum_to_sql(&t.return_type)?, t.party_id, t.party_name,
                t.original_transaction_id, enum_to_sql(&t.settlement_type)?, t.total_amount,
                t.reason, t.notes, t.created_at
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    fn create_return_item(&mut self, i: &ReturnItem) -> Result<i64, AppError> {
        self.conn.execute(
            &format!("INSERT INTO return_items ({ITEM_COLS}) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)"),
            params![
                if i.id > 0 { Some(i.id) } else { None },
                i.return_transaction_id, enum_to_sql(&i.line_type)?, i.item_id, i.item_name,
                i.sale_item_id, i.purchase_item_id, i.quantity, i.unit_price, i.total_amount,
                i.cost_amount, i.condition.as_ref().map(enum_to_sql).transpose()?
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    fn update_return_item_cost(&mut self, return_item_id: i64, cost_amount: f64) -> Result<(), AppError> {
        let n = self.conn.execute(
            "UPDATE return_items SET cost_amount = ?1 WHERE id = ?2",
            params![cost_amount, return_item_id],
        )?;
        if n == 0 {
            return Err(AppError::NotFound { entity: "ReturnItem".to_string(), id: return_item_id.to_string() });
        }
        Ok(())
    }

    fn update_return_total(&mut self, return_id: i64, total_amount: f64) -> Result<(), AppError> {
        let n = self.conn.execute(
            "UPDATE return_transactions SET total_amount = ?1 WHERE id = ?2",
            params![total_amount, return_id],
        )?;
        if n == 0 {
            return Err(AppError::NotFound { entity: "ReturnTransaction".to_string(), id: return_id.to_string() });
        }
        Ok(())
    }

    fn find_ledger_by_return_id(&self, return_id: i64) -> Result<Vec<PaymentSettlement>, AppError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, settlement_date, party_id, kind, transaction_type, transaction_id, return_id, amount_paid, payment_mode, notes, created_at \
             FROM payment_settlements WHERE return_id = ?1 ORDER BY id",
        )?;
        let rows = stmt
            .query_map(params![return_id], |r| {
                Ok(PaymentSettlement {
                    id: r.get(0)?,
                    settlement_date: r.get(1)?,
                    party_id: r.get(2)?,
                    kind: enum_col(r, 3)?,
                    transaction_type: enum_col(r, 4)?,
                    transaction_id: r.get(5)?,
                    return_id: r.get(6)?,
                    amount_paid: r.get(7)?,
                    payment_mode: r.get(8)?,
                    notes: r.get(9)?,
                    created_at: r.get(10)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    fn list_returns_page(
        &self,
        filter: &ReturnsFilter,
        offset: usize,
        limit: usize,
    ) -> Result<(Vec<ReturnTransaction>, usize), AppError> {
        let rt = filter.return_type.as_ref().map(enum_to_sql).transpose()?;
        let where_clause = "WHERE (?1 IS NULL OR return_type = ?1) AND (?2 IS NULL OR return_date >= ?2) AND (?3 IS NULL OR return_date <= ?3)";
        let total: i64 = self.conn.query_row(
            &format!("SELECT COUNT(*) FROM return_transactions {where_clause}"),
            params![rt, filter.start_date, filter.end_date],
            |r| r.get(0),
        )?;
        let lim = limit.min(i64::MAX as usize) as i64;
        let off = offset.min(i64::MAX as usize) as i64;
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {TX_COLS} FROM return_transactions {where_clause} ORDER BY return_date DESC, id DESC LIMIT ?4 OFFSET ?5"
        ))?;
        let rows = stmt
            .query_map(params![rt, filter.start_date, filter.end_date, lim, off], map_tx)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok((rows, total.max(0) as usize))
    }

    fn find_return_by_id(&self, id: i64) -> Result<Option<ReturnTransaction>, AppError> {
        Ok(self
            .conn
            .query_row(&format!("SELECT {TX_COLS} FROM return_transactions WHERE id = ?1"), params![id], map_tx)
            .optional()?)
    }

    fn get_next_return_code(&self) -> Result<String, AppError> {
        next_code(&self.conn, "return_transactions", "return_code", &RETURN_CODE)
    }

    fn list_returns(
        &self,
        return_type: Option<ReturnType>,
        offset: usize,
        limit: usize,
    ) -> Result<Vec<ReturnTransaction>, AppError> {
        let rt = return_type.as_ref().map(enum_to_sql).transpose()?;
        let lim = limit.min(i64::MAX as usize) as i64;
        let off = offset.min(i64::MAX as usize) as i64;
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {TX_COLS} FROM return_transactions WHERE (?1 IS NULL OR return_type = ?1) ORDER BY id LIMIT ?2 OFFSET ?3"
        ))?;
        let rows = stmt.query_map(params![rt, lim, off], map_tx)?.collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    fn find_items_by_return_id(&self, tx_id: i64) -> Result<Vec<ReturnItem>, AppError> {
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {ITEM_COLS} FROM return_items WHERE return_transaction_id = ?1 ORDER BY id"
        ))?;
        let rows = stmt.query_map(params![tx_id], map_item)?.collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }
}
