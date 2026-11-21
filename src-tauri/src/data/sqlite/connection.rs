//! r2d2 connection pool for SQLite.
use crate::core::error::AppError;

pub type SqlitePool = r2d2::Pool<r2d2_sqlite::SqliteConnectionManager>;

pub fn create_pool(db_path: &std::path::Path) -> Result<SqlitePool, AppError> {
    let manager = r2d2_sqlite::SqliteConnectionManager::file(db_path).with_init(|c| {
        // `journal_mode` returns a row, so it can't go through execute_batch.
        c.query_row("PRAGMA journal_mode=WAL", [], |_| Ok(()))?;
        c.busy_timeout(std::time::Duration::from_millis(5000))?;
        c.execute_batch("PRAGMA foreign_keys=ON; PRAGMA synchronous=NORMAL;")
    });
    r2d2::Pool::builder()
        .max_size(4)
        .build(manager)
        .map_err(|e| AppError::DatabaseError(format!("pool creation failed: {e}")))
}
