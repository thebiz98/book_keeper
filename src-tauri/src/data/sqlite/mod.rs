//! SQLite v1 persistence engine (M2).
//!
//! ## Shared conventions (all repositories MUST follow these)
//! * **Connection seam:** every repository struct holds a [`SqliteConn`] clone and is
//!   constructed with `Repo::new(conn: SqliteConn)`. The owning
//!   `SqliteTransactionContext` has already issued `BEGIN IMMEDIATE` on that connection,
//!   so repositories never begin/commit transactions themselves.
//! * **Table names** (snake_case plural of the HLD ER entity):
//!   `shop_settings`, `items`, `item_bundles`, `parties`, `purchase_transactions`,
//!   `purchase_items`, `sale_transactions`, `sale_items`, `sale_stock_allocations`,
//!   `return_transactions`, `return_items`, `payment_settlements`, `stock_adjustments`,
//!   `adjustment_stock_allocations`, plus FTS5 virtual table `item_search_fts`.
//! * **Columns** are exactly the Rust entity field names (`core::entities`), same order.
//!   `id INTEGER PRIMARY KEY AUTOINCREMENT`. Booleans are `INTEGER` 0/1, money/qty `REAL`,
//!   dates/timestamps `TEXT`.
//! * **Enums** are stored as `TEXT` using [`enum_to_sql`] / [`enum_from_sql`] (serde name,
//!   e.g. `PaymentStatus::Paid` -> `"Paid"`).
//! * **Explicit ids:** `create(&entity)` inserts the given `id` when `id > 0` (fixtures rely
//!   on this), otherwise lets SQLite assign one. Always return the row id.
//! * Errors: `rusqlite::Error` converts into `AppError` via `?` (`From` impl exists).

pub mod connection;
pub mod fts_indexer;
pub mod migrations;
pub mod repositories;
pub mod sqlite_uow;

use std::rc::Rc;

use serde::{de::DeserializeOwned, Serialize};

use crate::core::error::AppError;

/// Pooled connection shared by the transaction context and all its repositories.
/// `Rc` is fine: a `TransactionContext` is confined to one thread.
pub type SqliteConn = Rc<r2d2::PooledConnection<r2d2_sqlite::SqliteConnectionManager>>;

/// Serialises a unit-variant enum to its serde name for TEXT storage.
pub fn enum_to_sql<T: Serialize>(value: &T) -> String {
    match serde_json::to_value(value) {
        Ok(serde_json::Value::String(s)) => s,
        Ok(other) => other.to_string(),
        Err(_) => String::new(),
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
