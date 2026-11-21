//! r2d2 connection pool for SQLite.
use std::time::Duration;

use crate::core::error::AppError;

pub type SqlitePool = r2d2::Pool<r2d2_sqlite::SqliteConnectionManager>;

/// Applies the per-connection pragmas (LLD §5.3). `busy_timeout` goes first so that the
/// remaining pragmas (notably the WAL switch, which needs a lock) wait instead of failing
/// with SQLITE_BUSY when several pooled connections initialise concurrently.
pub fn configure_connection(c: &mut rusqlite::Connection) -> rusqlite::Result<()> {
    c.busy_timeout(Duration::from_millis(5000))?;
    // `journal_mode` returns a row, so it can't go through execute_batch.
    c.query_row("PRAGMA journal_mode=WAL", [], |_| Ok(()))?;
    c.execute_batch(
        "PRAGMA synchronous=NORMAL;
         PRAGMA foreign_keys=ON;
         PRAGMA temp_store=MEMORY;
         PRAGMA cache_size=-8000;",
    )
}

pub fn create_pool(db_path: &std::path::Path) -> Result<SqlitePool, AppError> {
    let manager = r2d2_sqlite::SqliteConnectionManager::file(db_path).with_init(configure_connection);
    r2d2::Pool::builder()
        .max_size(4)
        .connection_timeout(Duration::from_secs(5))
        .build(manager)
        .map_err(|e| AppError::DatabaseError(format!("pool creation failed: {e}")))
}
