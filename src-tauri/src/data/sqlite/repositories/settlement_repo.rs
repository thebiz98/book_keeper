//! SQLite implementation of `SettlementRepository`.
use rusqlite::{params, Row};

use crate::core::entities::PaymentSettlement;
use crate::core::error::AppError;
use crate::data::contracts::settlement_repository::SettlementRepository;
use crate::data::sqlite::SqliteConn;

const COLS: &str = "id, settlement_date, party_id, transaction_type, transaction_id, amount_paid, payment_mode, notes, created_at";

fn map(r: &Row<'_>) -> rusqlite::Result<PaymentSettlement> {
    Ok(PaymentSettlement {
        id: r.get(0)?,
        settlement_date: r.get(1)?,
        party_id: r.get(2)?,
        transaction_type: r.get(3)?,
        transaction_id: r.get(4)?,
        amount_paid: r.get(5)?,
        payment_mode: r.get(6)?,
        notes: r.get(7)?,
        created_at: r.get(8)?,
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

impl SettlementRepository for SqliteSettlementRepository {
    fn create_settlement(&mut self, s: &PaymentSettlement) -> Result<i64, AppError> {
        self.conn.execute(
            &format!("INSERT INTO payment_settlements ({COLS}) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)"),
            params![
                if s.id > 0 { Some(s.id) } else { None },
                s.settlement_date, s.party_id, s.transaction_type, s.transaction_id,
                s.amount_paid, s.payment_mode, s.notes, s.created_at
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    fn create(&mut self, s: &PaymentSettlement) -> Result<i64, AppError> {
        self.create_settlement(s)
    }

    fn list_by_party(&self, party_id: i64) -> Result<Vec<PaymentSettlement>, AppError> {
        self.query(
            &format!("SELECT {COLS} FROM payment_settlements WHERE party_id = ?1 ORDER BY id"),
            params![party_id],
        )
    }

    fn list_by_party_id(&self, party_id: i64) -> Result<Vec<PaymentSettlement>, AppError> {
        self.list_by_party(party_id)
    }

    fn list_by_transaction(&self, tx_type: &str, tx_id: i64) -> Result<Vec<PaymentSettlement>, AppError> {
        self.query(
            &format!("SELECT {COLS} FROM payment_settlements WHERE transaction_type = ?1 AND transaction_id = ?2 ORDER BY id"),
            params![tx_type, tx_id],
        )
    }

    fn get_daily_settlement_total(&self, date: &str, tx_type: &str) -> Result<f64, AppError> {
        Ok(self.conn.query_row(
            "SELECT COALESCE(SUM(amount_paid), 0.0) FROM payment_settlements WHERE settlement_date = ?1 AND transaction_type = ?2",
            params![date, tx_type],
            |r| r.get::<_, f64>(0),
        )?)
    }

    fn list_all(&self) -> Result<Vec<PaymentSettlement>, AppError> {
        self.query(&format!("SELECT {COLS} FROM payment_settlements ORDER BY id"), [])
    }
}
