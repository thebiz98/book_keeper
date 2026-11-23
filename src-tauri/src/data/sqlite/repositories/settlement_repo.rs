//! SQLite implementation of `SettlementRepository`.
use rusqlite::{params, Row};

use crate::core::dtos::LedgerTotals;
use crate::core::entities::PaymentSettlement;
use crate::core::error::AppError;
use crate::data::contracts::settlement_repository::SettlementRepository;
use crate::core::value_objects::{PaymentKind, TransactionType};
use crate::data::sqlite::{enum_col, enum_to_sql, SqliteConn};

const COLS: &str = "id, settlement_date, party_id, kind, transaction_type, transaction_id, return_id, amount_paid, payment_mode, notes, created_at";

fn map(r: &Row<'_>) -> rusqlite::Result<PaymentSettlement> {
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
}

pub struct SqliteSettlementRepository {
    conn: SqliteConn,
}

impl SqliteSettlementRepository {
    pub fn new(conn: SqliteConn) -> Self {
        Self { conn }
    }

    fn query(&self, sql: &str, p: impl rusqlite::Params) -> Result<Vec<PaymentSettlement>, AppError> {
        let mut stmt = self.conn.prepare(sql)?;
        let rows = stmt.query_map(p, map)?.collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }
}

fn kinds_in(kinds: &[PaymentKind]) -> Result<String, AppError> {
    let lits = kinds
        .iter()
        .map(|k| enum_to_sql(k).map(|s| format!("'{s}'")))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(if lits.is_empty() { "''".to_string() } else { lits.join(",") })
}

impl SettlementRepository for SqliteSettlementRepository {
    fn delete_by_transaction(
        &mut self,
        tx_type: TransactionType,
        tx_id: i64,
        kinds: &[PaymentKind],
    ) -> Result<Vec<PaymentSettlement>, AppError> {
        let in_list = kinds_in(kinds)?;
        let rows = self.query(
            &format!(
                "SELECT {COLS} FROM payment_settlements WHERE transaction_type = ?1 AND transaction_id = ?2 \
                 AND kind IN ({in_list}) ORDER BY id"
            ),
            params![enum_to_sql(&tx_type)?, tx_id],
        )?;
        self.conn.execute(
            &format!(
                "DELETE FROM payment_settlements WHERE transaction_type = ?1 AND transaction_id = ?2 AND kind IN ({in_list})"
            ),
            params![enum_to_sql(&tx_type)?, tx_id],
        )?;
        Ok(rows)
    }

    fn update_party_for_transaction(
        &mut self,
        tx_type: TransactionType,
        tx_id: i64,
        party_id: Option<i64>,
        kinds: &[PaymentKind],
    ) -> Result<(), AppError> {
        let in_list = kinds_in(kinds)?;
        self.conn.execute(
            &format!(
                "UPDATE payment_settlements SET party_id = ?3 WHERE transaction_type = ?1 AND transaction_id = ?2 \
                 AND kind IN ({in_list})"
            ),
            params![enum_to_sql(&tx_type)?, tx_id, party_id],
        )?;
        Ok(())
    }

    fn return_credit_return_ids(&self, tx_type: TransactionType, tx_id: i64) -> Result<Vec<i64>, AppError> {
        let mut stmt = self.conn.prepare(
            "SELECT DISTINCT return_id FROM payment_settlements WHERE transaction_type = ?1 AND transaction_id = ?2 \
             AND kind = 'RETURN_CREDIT' AND return_id IS NOT NULL ORDER BY return_id",
        )?;
        let rows = stmt
            .query_map(params![enum_to_sql(&tx_type)?, tx_id], |r| r.get(0))?
            .collect::<Result<Vec<i64>, _>>()?;
        Ok(rows)
    }

    fn delete_by_return(&mut self, return_id: i64) -> Result<Vec<PaymentSettlement>, AppError> {
        let rows = self.query(
            &format!("SELECT {COLS} FROM payment_settlements WHERE return_id = ?1 ORDER BY id"),
            params![return_id],
        )?;
        self.conn.execute("DELETE FROM payment_settlements WHERE return_id = ?1", params![return_id])?;
        Ok(rows)
    }

    fn create_settlement(&mut self, s: &PaymentSettlement) -> Result<i64, AppError> {
        self.conn.execute(
            &format!("INSERT INTO payment_settlements ({COLS}) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)"),
            params![
                if s.id > 0 { Some(s.id) } else { None },
                s.settlement_date, s.party_id, enum_to_sql(&s.kind)?, enum_to_sql(&s.transaction_type)?,
                s.transaction_id, s.return_id, s.amount_paid, s.payment_mode, s.notes, s.created_at
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    fn list_by_party(&self, party_id: i64) -> Result<Vec<PaymentSettlement>, AppError> {
        self.query(
            &format!("SELECT {COLS} FROM payment_settlements WHERE party_id = ?1 ORDER BY id"),
            params![party_id],
        )
    }

    fn list_by_transaction(&self, tx_type: TransactionType, tx_id: i64) -> Result<Vec<PaymentSettlement>, AppError> {
        self.query(
            &format!("SELECT {COLS} FROM payment_settlements WHERE transaction_type = ?1 AND transaction_id = ?2 ORDER BY id"),
            params![enum_to_sql(&tx_type)?, tx_id],
        )
    }

    fn sum_by_kind_for_transaction(&self, tx_type: TransactionType, tx_id: i64) -> Result<LedgerTotals, AppError> {
        let mut stmt = self.conn.prepare(
            "SELECT kind, COALESCE(SUM(amount_paid), 0.0) FROM payment_settlements \
             WHERE transaction_type = ?1 AND transaction_id = ?2 GROUP BY kind",
        )?;
        let rows = stmt
            .query_map(params![enum_to_sql(&tx_type)?, tx_id], |r| {
                Ok((enum_col::<PaymentKind>(r, 0)?, r.get::<_, f64>(1)?))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        let mut totals = LedgerTotals::default();
        for (kind, sum) in rows {
            match kind {
                PaymentKind::Upfront => totals.upfront = sum,
                PaymentKind::Settlement => totals.settlement = sum,
                PaymentKind::ReturnCredit => totals.return_credit = sum,
                PaymentKind::RefundOut => totals.refund_out = sum,
                PaymentKind::RefundIn => totals.refund_in = sum,
            }
        }
        Ok(totals)
    }

    fn sum_by_type_kind_between(
        &self,
        from: &str,
        to: &str,
    ) -> Result<Vec<(TransactionType, PaymentKind, f64)>, AppError> {
        let mut stmt = self.conn.prepare(
            "SELECT transaction_type, kind, COALESCE(SUM(amount_paid), 0.0) FROM payment_settlements \
             WHERE settlement_date >= ?1 AND settlement_date <= ?2 GROUP BY transaction_type, kind",
        )?;
        let rows = stmt
            .query_map(params![from, to], |r| {
                Ok((enum_col::<TransactionType>(r, 0)?, enum_col::<PaymentKind>(r, 1)?, r.get::<_, f64>(2)?))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    fn list_all(&self) -> Result<Vec<PaymentSettlement>, AppError> {
        self.query(&format!("SELECT {COLS} FROM payment_settlements ORDER BY id"), [])
    }
}
