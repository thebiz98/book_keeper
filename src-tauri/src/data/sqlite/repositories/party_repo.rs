//! SQLite implementation of `PartyRepository`.
use rusqlite::{params, OptionalExtension, Row};

use crate::core::dtos::{DeletedPartyRow, PartyDueSummary};
use crate::core::entities::{Party, PartyListRow, PartyOpeningBalance};
use crate::core::money::{round2, MONEY_EPS};
use crate::core::error::AppError;
use crate::core::value_objects::PartyType;
use crate::data::contracts::PartyRepository;
use crate::data::sqlite::fts::fts_prefix_query;
use crate::data::sqlite::{enum_col, enum_to_sql, SqliteConn};

pub struct SqlitePartyRepository {
    conn: SqliteConn,
}

impl SqlitePartyRepository {
    pub fn new(conn: SqliteConn) -> Self {
        Self { conn }
    }

    /// The party's opening bill (FR-35), read from the side its type owns.
    fn opening_balance_of(&self, party_id: i64, party_type: PartyType) -> Result<Option<PartyOpeningBalance>, AppError> {
        let table = match party_type {
            PartyType::Customer => "sale_transactions",
            PartyType::Vendor => "purchase_transactions",
        };
        Ok(self
            .conn
            .query_row(
                &format!(
                    "SELECT id, transaction_code, total_amount, transaction_date, total_amount - balance_due \
                     FROM {table} WHERE party_id = ?1 AND is_opening_balance = 1"
                ),
                params![party_id],
                |r| {
                    Ok(PartyOpeningBalance {
                        bill_id: r.get(0)?,
                        transaction_code: r.get(1)?,
                        amount: r.get(2)?,
                        as_of_date: r.get(3)?,
                        settled_amount: round2(r.get(4)?),
                    })
                },
            )
            .optional()?)
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
        opening_balance: None,
    })
}

impl PartyRepository for SqlitePartyRepository {
    fn find_by_id(&self, id: i64) -> Result<Option<Party>, AppError> {
        let found = self
            .conn
            .query_row(&format!("SELECT {COLS} FROM parties WHERE id = ?1"), params![id], map_party)
            .optional()?;
        match found {
            Some(mut p) => {
                p.opening_balance = self.opening_balance_of(p.id, p.party_type)?;
                Ok(Some(p))
            }
            None => Ok(None),
        }
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

    fn list_rows(
        &self,
        party_type: Option<PartyType>,
        active_only: bool,
        search: Option<&str>,
    ) -> Result<Vec<PartyListRow>, AppError> {
        let pt = party_type.as_ref().map(enum_to_sql).transpose()?;
        let q = search.map(str::trim).filter(|q| !q.is_empty());
        // Non-blank query with no alphanumeric token matches nothing.
        let fq = match q {
            Some(q) => match fts_prefix_query(q) {
                Some(f) => Some(f),
                None => return Ok(Vec::new()),
            },
            None => None,
        };
        // 3+ digits also match anywhere in the phone digits ("last digits" lookup).
        let digits = q
            .filter(|q| q.len() >= 3 && q.chars().all(|c| c.is_ascii_digit()))
            .map(|q| format!("%{q}%"));
        // Balance and bill count per party in the same statement (sale side for customers,
        // PURCHASE-source side for vendors, DD-07).
        let sql = "SELECT p.id, p.name, p.phone, p.party_type, p.notes, p.is_active, p.created_at, p.updated_at, \
               CASE p.party_type \
                 WHEN 'CUSTOMER' THEN (SELECT COALESCE(SUM(s.balance_due), 0.0) FROM sale_transactions s WHERE s.party_id = p.id) \
                 ELSE (SELECT COALESCE(SUM(u.balance_due), 0.0) FROM purchase_transactions u WHERE u.party_id = p.id AND u.source = 'PURCHASE') \
               END, \
               CASE p.party_type \
                 WHEN 'CUSTOMER' THEN (SELECT COUNT(*) FROM sale_transactions s WHERE s.party_id = p.id AND s.is_opening_balance = 0) \
                 ELSE (SELECT COUNT(*) FROM purchase_transactions u WHERE u.party_id = p.id AND u.source = 'PURCHASE' AND u.is_opening_balance = 0) \
               END \
             FROM parties p \
             WHERE (?1 IS NULL OR p.party_type = ?1) AND (?2 = 0 OR p.is_active = 1) \
               AND (?3 IS NULL \
                    OR p.id IN (SELECT party_id FROM party_search_fts WHERE party_search_fts MATCH ?3) \
                    OR (?4 IS NOT NULL AND p.id IN (SELECT party_id FROM party_search_fts WHERE phone_digits LIKE ?4))) \
             ORDER BY p.name COLLATE NOCASE, p.id";
        let mut stmt = self.conn.prepare_cached(sql)?;
        let mut rows = stmt
            .query_map(params![pt, active_only as i64, fq, digits], |r| {
                Ok(PartyListRow {
                    party: map_party(r)?,
                    outstanding_balance: r.get(8)?,
                    transaction_count: r.get(9)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        for row in &mut rows {
            row.party.opening_balance = self.opening_balance_of(row.party.id, row.party.party_type)?;
        }
        Ok(rows)
    }

    fn set_active(&mut self, id: i64, active: bool, updated_at: &str) -> Result<(), AppError> {
        let n = self.conn.execute(
            "UPDATE parties SET is_active = ?2, updated_at = ?3 WHERE id = ?1",
            params![id, active as i64, updated_at],
        )?;
        if n == 0 {
            return Err(AppError::NotFound { entity: "Party".to_string(), id: id.to_string() });
        }
        Ok(())
    }

    fn hard_delete(&mut self, id: i64) -> Result<(), AppError> {
        let n = self.conn.execute("DELETE FROM parties WHERE id = ?1", params![id])?;
        if n == 0 {
            return Err(AppError::NotFound { entity: "Party".to_string(), id: id.to_string() });
        }
        Ok(())
    }

    fn reference_count(&self, id: i64) -> Result<i64, AppError> {
        let (a, b, c, d) = self.reference_breakdown(id)?;
        Ok(a + b + c + d)
    }

    fn reference_breakdown(&self, id: i64) -> Result<(i64, i64, i64, i64), AppError> {
        Ok(self.conn.query_row(
            "SELECT (SELECT COUNT(*) FROM sale_transactions WHERE party_id = ?1), \
                    (SELECT COUNT(*) FROM purchase_transactions WHERE party_id = ?1), \
                    (SELECT COUNT(*) FROM return_transactions WHERE party_id = ?1), \
                    (SELECT COUNT(*) FROM payment_settlements WHERE party_id = ?1)",
            params![id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )?)
    }

    fn open_dues(&self, id: i64, party_type: PartyType) -> Result<(i64, f64), AppError> {
        let (table, source_filter) = match party_type {
            PartyType::Customer => ("sale_transactions", ""),
            PartyType::Vendor => ("purchase_transactions", " AND source = 'PURCHASE'"),
        };
        let (n, sum): (i64, f64) = self.conn.query_row(
            &format!(
                "SELECT COUNT(*), COALESCE(SUM(balance_due), 0.0) FROM {table} \
                 WHERE party_id = ?1 AND balance_due > {MONEY_EPS}{source_filter}"
            ),
            params![id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        Ok((n, round2(sum)))
    }

    fn list_inactive(&self, search: &str, offset: usize, limit: usize) -> Result<(Vec<DeletedPartyRow>, usize), AppError> {
        let q = search.trim();
        let like = format!("%{}%", q.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_"));
        let digits: String = q.chars().filter(char::is_ascii_digit).collect();
        // A query without digits matches no phone (an empty pattern would match everything).
        let phone_like = if digits.is_empty() { None } else { Some(format!("%{digits}%")) };
        let filter = "is_active = 0 AND (name LIKE ?1 ESCAPE '\\' OR (?2 IS NOT NULL AND \
                      REPLACE(REPLACE(REPLACE(REPLACE(REPLACE(COALESCE(phone, ''), ' ', ''), '-', ''), '+', ''), '(', ''), ')', '') LIKE ?2))";
        let total: i64 = self.conn.query_row(
            &format!("SELECT COUNT(*) FROM parties WHERE {filter}"),
            params![like, phone_like],
            |r| r.get(0),
        )?;
        let mut stmt = self.conn.prepare_cached(&format!(
            "SELECT id, name, party_type, phone, updated_at FROM parties WHERE {filter} \
             ORDER BY updated_at DESC, id LIMIT ?3 OFFSET ?4"
        ))?;
        let limit = i64::try_from(limit).unwrap_or(i64::MAX);
        let offset = i64::try_from(offset).unwrap_or(i64::MAX);
        let rows = stmt
            .query_map(params![like, phone_like, limit, offset], |r| {
                Ok(DeletedPartyRow {
                    party_id: r.get(0)?,
                    name: r.get(1)?,
                    party_type: enum_col(r, 2)?,
                    phone: r.get(3)?,
                    deleted_at: r.get(4)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok((rows, total.max(0) as usize))
    }

    fn get_party_due_summary(&self, party_type: PartyType) -> Result<Vec<PartyDueSummary>, AppError> {
        // Customers owe on sales; vendors are owed on PURCHASE-source purchases only (DD-07).
        // One grouped query over the open bills of every party of the type.
        let (table, source_filter) = match party_type {
            PartyType::Customer => ("sale_transactions", ""),
            PartyType::Vendor => ("purchase_transactions", " AND t.source = 'PURCHASE'"),
        };
        let sql = format!(
            "SELECT p.id, p.name, p.phone, COUNT(t.id), MIN(t.transaction_date), \
                    SUM(t.total_amount), SUM(t.paid_amount), SUM(t.balance_due), p.is_active, \
                    SUM(t.total_amount - t.balance_due) \
             FROM parties p JOIN {table} t ON t.party_id = p.id AND t.balance_due > 0{source_filter} \
             WHERE p.party_type = ?1 \
             GROUP BY p.id ORDER BY p.name COLLATE NOCASE, p.id"
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt
            .query_map(params![enum_to_sql(&party_type)?], |r| {
                Ok(PartyDueSummary {
                    party_id: r.get(0)?,
                    party_name: r.get(1)?,
                    phone: r.get(2)?,
                    party_type,
                    unsettled_bills_count: r.get::<_, i64>(3)? as usize,
                    earliest_bill_date: r.get(4)?,
                    total_invoiced: r.get(5)?,
                    total_paid: r.get(6)?,
                    paid_and_credited: r.get(9)?,
                    balance_due: r.get(7)?,
                    is_active: r.get(8)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }
}
