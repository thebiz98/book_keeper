//! SQLite implementation of `ExpenseRepository`.
use rusqlite::types::Value;
use rusqlite::{params, params_from_iter, OptionalExtension, Row};

use crate::core::dtos::{clean_text, ExpenseFilter, ExpenseTypeTotal};
use crate::core::entities::{Expense, ExpenseType};
use crate::core::error::AppError;
use crate::data::contracts::ExpenseRepository;
use crate::data::sqlite::code_gen::{next_code, EXPENSE_CODE};
use crate::data::sqlite::{like_contains, SqliteConn};

pub struct SqliteExpenseRepository {
    conn: SqliteConn,
}

impl SqliteExpenseRepository {
    pub fn new(conn: SqliteConn) -> Self {
        Self { conn }
    }
}

const TYPE_COLS: &str = "id, name, is_active, created_at";
const COLS: &str =
    "id, expense_code, expense_date, expense_type_id, expense_type_name, amount, notes, created_at, updated_at";

fn map_type(r: &Row<'_>) -> rusqlite::Result<ExpenseType> {
    Ok(ExpenseType { id: r.get(0)?, name: r.get(1)?, is_active: r.get(2)?, created_at: r.get(3)? })
}

fn map_expense(r: &Row<'_>) -> rusqlite::Result<Expense> {
    Ok(Expense {
        id: r.get(0)?,
        expense_code: r.get(1)?,
        expense_date: r.get(2)?,
        expense_type_id: r.get(3)?,
        expense_type_name: r.get(4)?,
        amount: r.get(5)?,
        notes: r.get(6)?,
        created_at: r.get(7)?,
        updated_at: r.get(8)?,
    })
}

fn where_clause(f: &ExpenseFilter) -> (String, Vec<Value>) {
    let mut conds: Vec<String> = Vec::new();
    let mut vals: Vec<Value> = Vec::new();
    if let Some(s) = f.start_date.as_deref() {
        conds.push("e.expense_date >= ?".into());
        vals.push(Value::Text(s.to_string()));
    }
    if let Some(s) = f.end_date.as_deref() {
        conds.push("e.expense_date <= ?".into());
        vals.push(Value::Text(s.to_string()));
    }
    if let Some(t) = f.expense_type_id {
        conds.push("e.expense_type_id = ?".into());
        vals.push(Value::Integer(t));
    }
    if let Some(q) = clean_text(f.search_query.as_deref()) {
        let pat = like_contains(q);
        conds.push(
            "(e.expense_code LIKE ? ESCAPE '\\' OR e.expense_type_name LIKE ? ESCAPE '\\' \
             OR COALESCE(e.notes,'') LIKE ? ESCAPE '\\')"
                .into(),
        );
        for _ in 0..3 {
            vals.push(Value::Text(pat.clone()));
        }
    }
    let w = if conds.is_empty() { String::new() } else { format!(" WHERE {}", conds.join(" AND ")) };
    (w, vals)
}

fn not_found(entity: &str, id: i64) -> AppError {
    AppError::NotFound { entity: entity.to_string(), id: id.to_string() }
}

impl ExpenseRepository for SqliteExpenseRepository {
    fn create_type(&mut self, t: &ExpenseType) -> Result<i64, AppError> {
        let id = if t.id > 0 { Some(t.id) } else { None };
        self.conn.execute(
            &format!("INSERT INTO expense_types ({TYPE_COLS}) VALUES (?1,?2,?3,?4)"),
            params![id, t.name, t.is_active, t.created_at],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    fn update_type_name(&mut self, id: i64, name: &str) -> Result<(), AppError> {
        let n = self.conn.execute("UPDATE expense_types SET name = ?2 WHERE id = ?1", params![id, name])?;
        if n == 0 {
            return Err(not_found("ExpenseType", id));
        }
        Ok(())
    }

    fn find_type_by_id(&self, id: i64) -> Result<Option<ExpenseType>, AppError> {
        Ok(self
            .conn
            .query_row(&format!("SELECT {TYPE_COLS} FROM expense_types WHERE id = ?1"), [id], map_type)
            .optional()?)
    }

    fn find_type_by_name(&self, name: &str) -> Result<Option<ExpenseType>, AppError> {
        Ok(self
            .conn
            .query_row(
                &format!("SELECT {TYPE_COLS} FROM expense_types WHERE name = ?1 COLLATE NOCASE"),
                [name],
                map_type,
            )
            .optional()?)
    }

    fn list_types(&self, active_only: bool) -> Result<Vec<ExpenseType>, AppError> {
        let mut stmt = self.conn.prepare_cached(&format!(
            "SELECT {TYPE_COLS} FROM expense_types WHERE (?1 = 0 OR is_active = 1) ORDER BY name COLLATE NOCASE, id"
        ))?;
        let rows = stmt.query_map([active_only as i64], map_type)?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    fn find_types_by_ids(&self, ids: &[i64]) -> Result<Vec<ExpenseType>, AppError> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let ph = vec!["?"; ids.len()].join(",");
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {TYPE_COLS} FROM expense_types WHERE is_active = 1 AND id IN ({ph})"
        ))?;
        let rows = stmt.query_map(params_from_iter(ids.iter()), map_type)?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    fn count_types(&self, active_only: bool) -> Result<usize, AppError> {
        let n: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM expense_types WHERE (?1 = 0 OR is_active = 1)",
            [active_only as i64],
            |r| r.get(0),
        )?;
        Ok(n as usize)
    }

    fn get_next_expense_code(&self) -> Result<String, AppError> {
        next_code(&self.conn, "expenses", "expense_code", &EXPENSE_CODE)
    }

    fn create(&mut self, e: &Expense) -> Result<i64, AppError> {
        let id = if e.id > 0 { Some(e.id) } else { None };
        self.conn.execute(
            &format!("INSERT INTO expenses ({COLS}) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)"),
            params![
                id, e.expense_code, e.expense_date, e.expense_type_id, e.expense_type_name, e.amount, e.notes,
                e.created_at, e.updated_at
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    fn update(&mut self, e: &Expense) -> Result<(), AppError> {
        let n = self.conn.execute(
            "UPDATE expenses SET expense_date=?2, expense_type_id=?3, expense_type_name=?4, amount=?5, notes=?6, \
             updated_at=?7 WHERE id=?1",
            params![e.id, e.expense_date, e.expense_type_id, e.expense_type_name, e.amount, e.notes, e.updated_at],
        )?;
        if n == 0 {
            return Err(not_found("Expense", e.id));
        }
        Ok(())
    }

    fn delete(&mut self, id: i64) -> Result<(), AppError> {
        let n = self.conn.execute("DELETE FROM expenses WHERE id = ?1", [id])?;
        if n == 0 {
            return Err(not_found("Expense", id));
        }
        Ok(())
    }

    fn find_by_id(&self, id: i64) -> Result<Option<Expense>, AppError> {
        Ok(self
            .conn
            .query_row(&format!("SELECT {COLS} FROM expenses WHERE id = ?1"), [id], map_expense)
            .optional()?)
    }

    fn list(&self, f: &ExpenseFilter, offset: usize, limit: usize) -> Result<(Vec<Expense>, usize, f64), AppError> {
        let (w, vals) = where_clause(f);
        let (total, sum): (i64, f64) = self.conn.query_row(
            &format!("SELECT COUNT(*), COALESCE(SUM(e.amount), 0.0) FROM expenses e{w}"),
            params_from_iter(vals.iter()),
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        let mut page_vals = vals;
        page_vals.push(Value::Integer(limit as i64));
        page_vals.push(Value::Integer(offset as i64));
        let cols = COLS.split(", ").map(|c| format!("e.{c}")).collect::<Vec<_>>().join(", ");
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {cols} FROM expenses e{w} ORDER BY e.expense_date DESC, e.id DESC LIMIT ? OFFSET ?"
        ))?;
        let rows = stmt.query_map(params_from_iter(page_vals.iter()), map_expense)?;
        Ok((rows.collect::<Result<Vec<_>, _>>()?, total as usize, sum))
    }

    fn sum_between(&self, from: &str, to: &str) -> Result<f64, AppError> {
        Ok(self.conn.query_row(
            "SELECT COALESCE(SUM(amount), 0.0) FROM expenses WHERE expense_date BETWEEN ?1 AND ?2",
            params![from, to],
            |r| r.get(0),
        )?)
    }

    fn totals_by_type_between(&self, from: &str, to: &str) -> Result<Vec<ExpenseTypeTotal>, AppError> {
        // The label is the snapshot name of the most recently recorded expense of the type (DD-30).
        let mut stmt = self.conn.prepare(
            "SELECT e.expense_type_id, \
                    (SELECT l.expense_type_name FROM expenses l WHERE l.expense_type_id = e.expense_type_id \
                       AND l.expense_date BETWEEN ?1 AND ?2 ORDER BY l.expense_date DESC, l.id DESC LIMIT 1) AS label, \
                    SUM(e.amount) AS total \
             FROM expenses e WHERE e.expense_date BETWEEN ?1 AND ?2 \
             GROUP BY e.expense_type_id ORDER BY total DESC, label COLLATE NOCASE",
        )?;
        let rows = stmt.query_map(params![from, to], |r| {
            Ok(ExpenseTypeTotal { expense_type_id: r.get(0)?, expense_type_name: r.get(1)?, amount: r.get(2)? })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }
}
