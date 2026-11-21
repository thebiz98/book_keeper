//! SQLite implementation of `PartyRepository`.
use rusqlite::{params, OptionalExtension, Row};

use crate::core::dtos::PartyDueSummary;
use crate::core::entities::Party;
use crate::core::error::AppError;
use crate::core::value_objects::PartyType;
use crate::data::contracts::PartyRepository;
use crate::data::sqlite::{enum_col, enum_to_sql, SqliteConn};

pub struct SqlitePartyRepository {
    conn: SqliteConn,
}

impl SqlitePartyRepository {
    pub fn new(conn: SqliteConn) -> Self {
        Self { conn }
    }
}

const COLS: &str = "id, name, phone, party_type, notes, is_active, created_at, updated_at";

fn map_party(r: &Row<'_>) -> rusqlite::Result<Party> {
    Ok(Party {
        id: r.get(0)?,
        name: r.get(1)?,
        phone: r.get(2)?,
        party_type: enum_col(r, 3)?,
        notes: r.get(4)?,
        is_active: r.get(5)?,
        created_at: r.get(6)?,
        updated_at: r.get(7)?,
    })
}

impl PartyRepository for SqlitePartyRepository {
    fn find_by_id(&self, id: i64) -> Result<Option<Party>, AppError> {
        Ok(self
            .conn
            .query_row(&format!("SELECT {COLS} FROM parties WHERE id = ?1"), params![id], map_party)
            .optional()?)
    }

    fn list(&self, party_type: Option<PartyType>, active_only: bool) -> Result<Vec<Party>, AppError> {
        let pt = party_type.as_ref().map(enum_to_sql).transpose()?;
        let mut stmt = self.conn.prepare_cached(&format!(
            "SELECT {COLS} FROM parties WHERE (?1 IS NULL OR party_type = ?1) AND (?2 = 0 OR is_active = 1) ORDER BY id"
        ))?;
        let rows = stmt.query_map(params![pt, active_only as i64], map_party)?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    fn create(&mut self, p: &Party) -> Result<i64, AppError> {
        let id = if p.id > 0 { Some(p.id) } else { None };
        self.conn.execute(
            &format!("INSERT INTO parties ({COLS}) VALUES (?1,?2,?3,?4,?5,?6,?7,?8)"),
            params![id, p.name, p.phone, enum_to_sql(&p.party_type)?, p.notes, p.is_active, p.created_at, p.updated_at],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    fn update(&mut self, p: &Party) -> Result<(), AppError> {
        let n = self.conn.execute(
            "UPDATE parties SET name=?2, phone=?3, party_type=?4, notes=?5, is_active=?6, created_at=?7, updated_at=?8 WHERE id=?1",
            params![p.id, p.name, p.phone, enum_to_sql(&p.party_type)?, p.notes, p.is_active, p.created_at, p.updated_at],
        )?;
        if n == 0 {
            return Err(AppError::NotFound { entity: "Party".to_string(), id: p.id.to_string() });
        }
        Ok(())
    }

    fn get_party_due_summary(&self, party_type: PartyType) -> Result<Vec<PartyDueSummary>, AppError> {
        // Customers owe on sales; vendors are owed on PURCHASE-source purchases only (DD-07).
        let (table, source_filter) = match party_type {
            PartyType::Customer => ("sale_transactions", ""),
            PartyType::Vendor => ("purchase_transactions", " AND t.source = 'PURCHASE'"),
        };
        let sql = format!(
            "SELECT p.id, p.name, p.phone, SUM(t.balance_due), MIN(t.transaction_date), COUNT(t.id) \
             FROM parties p JOIN {table} t ON t.party_id = p.id AND t.balance_due > 0{source_filter} \
             WHERE p.party_type = ?1 AND p.is_active = 1 GROUP BY p.id ORDER BY p.id"
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt
            .query_map(params![enum_to_sql(&party_type)?], |r| {
                Ok(PartyDueSummary {
                    party_id: r.get(0)?,
                    party_name: r.get(1)?,
                    phone: r.get(2)?,
                    party_type,
                    total_due: r.get(3)?,
                    oldest_bill_date: r.get(4)?,
                    unpaid_bills_count: r.get::<_, i64>(5)? as usize,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }
}
