//! SQLite implementation of `PartyRepository`.
use rusqlite::{params, OptionalExtension, Row};

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
        let pt = party_type.as_ref().map(enum_to_sql);
        let mut stmt = self.conn.prepare_cached(&format!(
            "SELECT {COLS} FROM parties WHERE (?1 IS NULL OR party_type = ?1) AND (?2 = 0 OR is_active = 1) ORDER BY id"
        ))?;
        let rows = stmt.query_map(params![pt, active_only as i64], map_party)?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    fn list_by_type(&self, party_type: PartyType, active_only: bool) -> Result<Vec<Party>, AppError> {
        self.list(Some(party_type), active_only)
    }

    fn create(&mut self, p: &Party) -> Result<i64, AppError> {
        let id = if p.id > 0 { Some(p.id) } else { None };
        self.conn.execute(
            &format!("INSERT INTO parties ({COLS}) VALUES (?1,?2,?3,?4,?5,?6,?7,?8)"),
            params![id, p.name, p.phone, enum_to_sql(&p.party_type), p.notes, p.is_active, p.created_at, p.updated_at],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    fn update(&mut self, p: &Party) -> Result<(), AppError> {
        let n = self.conn.execute(
            "UPDATE parties SET name=?2, phone=?3, party_type=?4, notes=?5, is_active=?6, created_at=?7, updated_at=?8 WHERE id=?1",
            params![p.id, p.name, p.phone, enum_to_sql(&p.party_type), p.notes, p.is_active, p.created_at, p.updated_at],
        )?;
        if n == 0 {
            return Err(AppError::NotFound { entity: "Party".to_string(), id: p.id.to_string() });
        }
        Ok(())
    }
}
