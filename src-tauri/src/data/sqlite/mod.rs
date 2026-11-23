//! SQLite v1 persistence engine (M2).
//!
//! ## Shared conventions (all repositories MUST follow these)
//! * **Connection seam:** every repository struct holds a [`SqliteConn`] clone and is
//!   constructed with `Repo::new(conn: SqliteConn)`. The owning
//!   `SqliteTransactionContext` has already begun the transaction on that connection
//!   (`BEGIN IMMEDIATE` for `begin_transaction`, `BEGIN DEFERRED` + `query_only` for
//!   `begin_read`), so repositories never begin/commit transactions themselves.
//! * **Codes:** every human-readable code comes from [`code_gen::next_code`].
//! * **Table names** (snake_case plural of the HLD ER entity):
//!   `shop_settings`, `items`, `item_bundles`, `parties`, `purchase_transactions`,
//!   `purchase_items`, `sale_transactions`, `sale_items`, `sale_stock_allocations`,
//!   `return_transactions`, `return_items`, `payment_settlements`, `stock_adjustments`,
//!   `stock_outflow_allocations`, plus FTS5 virtual table `item_search_fts`.
//! * **Columns** are exactly the Rust entity field names (`core::entities`), same order; repositories
//!   use explicit named column lists (never positional `SELECT *`).
//!   `id INTEGER PRIMARY KEY AUTOINCREMENT`. Booleans are `INTEGER` 0/1, money/qty `REAL`,
//!   dates/timestamps `TEXT`.
//! * **Enums** are stored as `TEXT` using [`enum_to_sql`] / [`enum_from_sql`] (serde name,
//!   SCREAMING_SNAKE_CASE, e.g. `PaymentStatus::Paid` -> `"PAID"`; DD-23).
//! * **Explicit ids:** `create(&entity)` inserts the given `id` when `id > 0` (fixtures rely
//!   on this), otherwise lets SQLite assign one. Always return the row id.
//! * Errors: `rusqlite::Error` converts into `AppError` via `?` (`From` impl exists).

pub mod code_gen;
pub mod connection;
pub mod fts;
pub mod fts_indexer;
pub mod fts_specs;
pub mod migrations;
pub mod repositories;
pub mod sqlite_uow;

use std::rc::Rc;

use serde::{de::DeserializeOwned, Serialize};

use crate::core::error::AppError;

/// Pooled connection shared by the transaction context and all its repositories.
/// `Rc` is fine: a `TransactionContext` is confined to one thread.
pub type SqliteConn = Rc<r2d2::PooledConnection<r2d2_sqlite::SqliteConnectionManager>>;

/// Serialises a unit-variant enum to its serde name (SCREAMING_SNAKE_CASE) for TEXT storage.
/// Errors propagate instead of silently producing an empty string.
pub fn enum_to_sql<T: Serialize>(value: &T) -> Result<String, AppError> {
    match serde_json::to_value(value) {
        Ok(serde_json::Value::String(s)) => Ok(s),
        Ok(other) => Err(AppError::DatabaseError(format!("enum did not serialize to a string: {other}"))),
        Err(e) => Err(AppError::DatabaseError(format!("enum serialization failed: {e}"))),
    }
}

/// Parses a TEXT column back into a unit-variant enum.
pub fn enum_from_sql<T: DeserializeOwned>(raw: &str) -> Result<T, AppError> {
    serde_json::from_value(serde_json::Value::String(raw.to_string()))
        .map_err(|e| AppError::DatabaseError(format!("invalid enum value '{raw}': {e}")))
}

/// Same as [`enum_from_sql`] but usable inside rusqlite row-mapping closures.
pub fn enum_col<T: DeserializeOwned>(row: &rusqlite::Row<'_>, idx: usize) -> rusqlite::Result<T> {
    let raw: String = row.get(idx)?;
    serde_json::from_value(serde_json::Value::String(raw.clone())).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(idx, rusqlite::types::Type::Text, Box::new(e))
    })
}

/// Nullable enum column (`NULL` -> `None`).
pub fn opt_enum_col<T: DeserializeOwned>(row: &rusqlite::Row<'_>, idx: usize) -> rusqlite::Result<Option<T>> {
    let raw: Option<String> = row.get(idx)?;
    match raw {
        None => Ok(None),
        Some(raw) => serde_json::from_value(serde_json::Value::String(raw))
            .map(Some)
            .map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(idx, rusqlite::types::Type::Text, Box::new(e))
            }),
    }
}

/// `%q%` with `\`, `%` and `_` escaped, for `LIKE ? ESCAPE '\'`.
pub fn like_contains(q: &str) -> String {
    let mut out = String::with_capacity(q.len() + 2);
    out.push('%');
    for c in q.chars() {
        if matches!(c, '\\' | '%' | '_') {
            out.push('\\');
        }
        out.push(c);
    }
    out.push('%');
    out
}

/// `WHERE` clause (with leading space) and bound values shared by the sales and purchases history
/// lists. Columns are qualified with alias `t`. `base_cond` is a fixed predicate (the purchase
/// source), `search_cols` the columns `search_query` is matched against (case-insensitive for ASCII).
pub fn history_where(
    filter: &crate::core::dtos::SalesHistoryFilter,
    base_cond: Option<&str>,
    search_cols: &[&str],
) -> Result<(String, Vec<rusqlite::types::Value>), AppError> {
    use rusqlite::types::Value;
    let mut conds: Vec<String> = base_cond.map(|c| vec![c.to_string()]).unwrap_or_default();
    let mut vals: Vec<Value> = Vec::new();
    if let Some(p) = filter.party_id {
        conds.push("t.party_id = ?".into());
        vals.push(Value::Integer(p));
    }
    if let Some(s) = filter.payment_status.and_then(|f| f.as_status()) {
        conds.push("t.payment_status = ?".into());
        vals.push(Value::Text(enum_to_sql(&s)?));
    }
    if let Some(s) = filter.start_date.as_deref() {
        conds.push("t.transaction_date >= ?".into());
        vals.push(Value::Text(s.to_string()));
    }
    if let Some(e) = filter.end_date.as_deref() {
        conds.push("t.transaction_date <= ?".into());
        vals.push(Value::Text(e.to_string()));
    }
    if let Some(q) = crate::core::dtos::clean_text(filter.search_query.as_deref()) {
        let pat = like_contains(q);
        let parts: Vec<String> = search_cols.iter().map(|c| format!("{c} LIKE ? ESCAPE '\\'")).collect();
        conds.push(format!("({})", parts.join(" OR ")));
        for _ in search_cols {
            vals.push(Value::Text(pat.clone()));
        }
    }
    let w = if conds.is_empty() { String::new() } else { format!(" WHERE {}", conds.join(" AND ")) };
    Ok((w, vals))
}
