//! Generic FTS5 index over one master table (`FtsSpec` + `SqliteFtsIndex`).
//!
//! All writes use the index's own pooled connection and must run AFTER the owning transaction
//! has committed (DD-24). Failures are repaired by the startup `ensure_fts_index` count check
//! and by `data_reset(All)` rebuilds.
use rusqlite::{params_from_iter, Connection};

use crate::core::error::AppError;
use crate::data::contracts::search_indexer::FtsIndex;
use super::connection::SqlitePool;

/// Static description of one FTS5 table mirroring a master table.
pub struct FtsSpec {
    /// FTS table name, e.g. `"expense_type_search_fts"`.
    pub table: &'static str,
    /// Key column (declared `UNINDEXED`, first column of the FTS table).
    pub key_column: &'static str,
    /// Indexed text columns, in insert order.
    pub columns: &'static [&'static str],
    /// `SELECT key, c1..cn FROM .. WHERE is_active = 1`; may yield several rows per key.
    pub source_all_sql: &'static str,
    /// Same SELECT restricted to one key (`?1`).
    pub source_one_sql: &'static str,
    /// `SELECT COUNT(*)` of the keys that should be indexed.
    pub source_count_sql: &'static str,
}

#[derive(Clone)]
pub struct SqliteFtsIndex {
    pool: SqlitePool,
    spec: &'static FtsSpec,
}

fn idx_err(e: rusqlite::Error) -> AppError {
    AppError::SearchIndexError(e.to_string())
}

type SourceRow = (i64, Vec<Option<String>>);

impl SqliteFtsIndex {
    pub fn new(pool: SqlitePool, spec: &'static FtsSpec) -> Self {
        Self { pool, spec }
    }

    pub fn spec(&self) -> &'static FtsSpec {
        self.spec
    }

    fn conn(&self) -> Result<r2d2::PooledConnection<r2d2_sqlite::SqliteConnectionManager>, AppError> {
        self.pool
            .get()
            .map_err(|e| AppError::SearchIndexError(format!("pool checkout failed: {e}")))
    }

    fn insert_sql(&self) -> String {
        let s = self.spec;
        let ph: Vec<String> = (1..=s.columns.len() + 1).map(|i| format!("?{i}")).collect();
        format!(
            "INSERT INTO {} ({}, {}) VALUES ({})",
            s.table,
            s.key_column,
            s.columns.join(", "),
            ph.join(", ")
        )
    }

    fn insert_rows(&self, conn: &Connection, key: i64, rows: &[Vec<Option<String>>]) -> Result<(), AppError> {
        let mut stmt = conn.prepare_cached(&self.insert_sql()).map_err(idx_err)?;
        for row in rows {
            // The key is bound as an integer so `COUNT(DISTINCT key)` and joins stay numeric.
            let mut params: Vec<rusqlite::types::Value> = Vec::with_capacity(row.len() + 1);
            params.push(rusqlite::types::Value::Integer(key));
            for v in row {
                params.push(match v {
                    Some(s) => rusqlite::types::Value::Text(s.clone()),
                    None => rusqlite::types::Value::Null,
                });
            }
            stmt.execute(params_from_iter(params.iter())).map_err(idx_err)?;
        }
        Ok(())
    }

    fn delete_key(&self, conn: &Connection, key: i64) -> Result<(), AppError> {
        conn.execute(
            &format!("DELETE FROM {} WHERE {} = ?1", self.spec.table, self.spec.key_column),
            [key],
        )
        .map_err(idx_err)?;
        Ok(())
    }

    fn read_source(&self, conn: &Connection, sql: &str, key: Option<i64>) -> Result<Vec<SourceRow>, AppError> {
        let n = self.spec.columns.len();
        let mut stmt = conn.prepare(sql).map_err(idx_err)?;
        let map = |r: &rusqlite::Row<'_>| -> rusqlite::Result<SourceRow> {
            let id: i64 = r.get(0)?;
            let mut cols = Vec::with_capacity(n);
            for i in 0..n {
                cols.push(r.get::<_, Option<String>>(i + 1)?);
            }
            Ok((id, cols))
        };
        let rows = match key {
            Some(k) => stmt.query_map([k], map).map_err(idx_err)?.collect::<Result<Vec<_>, _>>(),
            None => stmt.query_map([], map).map_err(idx_err)?.collect::<Result<Vec<_>, _>>(),
        };
        rows.map_err(idx_err)
    }

    /// Replaces `key`'s rows with caller-built rows (each row = `columns.len()` values). Atomic.
    pub fn replace_rows(&self, key: i64, rows: &[Vec<Option<String>>]) -> Result<(), AppError> {
        let conn = self.conn()?;
        let tx = conn.unchecked_transaction().map_err(idx_err)?;
        self.delete_key(&tx, key)?;
        self.insert_rows(&tx, key, rows)?;
        tx.commit().map_err(idx_err)
    }
}

impl FtsIndex for SqliteFtsIndex {
    fn table(&self) -> &'static str {
        self.spec.table
    }

    fn reindex(&self, key: i64) -> Result<(), AppError> {
        let conn = self.conn()?;
        let tx = conn.unchecked_transaction().map_err(idx_err)?;
        let src = self.read_source(&tx, self.spec.source_one_sql, Some(key))?;
        self.delete_key(&tx, key)?;
        let rows: Vec<Vec<Option<String>>> = src.into_iter().map(|(_, c)| c).collect();
        self.insert_rows(&tx, key, &rows)?;
        tx.commit().map_err(idx_err)
    }

    fn reindex_many(&self, keys: &[i64]) -> Result<(), AppError> {
        for &k in keys {
            self.reindex(k)?;
        }
        Ok(())
    }

    fn remove(&self, key: i64) -> Result<(), AppError> {
        let conn = self.conn()?;
        self.delete_key(&conn, key)
    }

    fn rebuild(&self) -> Result<(), AppError> {
        let conn = self.conn()?;
        let tx = conn.unchecked_transaction().map_err(idx_err)?;
        let src = self.read_source(&tx, self.spec.source_all_sql, None)?;
        tx.execute(&format!("DELETE FROM {}", self.spec.table), []).map_err(idx_err)?;
        for (key, cols) in src {
            self.insert_rows(&tx, key, std::slice::from_ref(&cols))?;
        }
        tx.commit().map_err(idx_err)
    }

    fn indexed_count(&self) -> Result<usize, AppError> {
        let conn = self.conn()?;
        let n: i64 = conn
            .query_row(
                &format!("SELECT COUNT(DISTINCT {}) FROM {}", self.spec.key_column, self.spec.table),
                [],
                |r| r.get(0),
            )
            .map_err(idx_err)?;
        Ok(n.max(0) as usize)
    }

    fn source_count(&self) -> Result<usize, AppError> {
        let conn = self.conn()?;
        let n: i64 = conn.query_row(self.spec.source_count_sql, [], |r| r.get(0)).map_err(idx_err)?;
        Ok(n.max(0) as usize)
    }

    fn search_keys(&self, query: &str, limit: usize) -> Result<Vec<i64>, AppError> {
        let Some(fq) = fts_prefix_query(query) else { return Ok(Vec::new()) };
        let conn = self.conn()?;
        let t = self.spec.table;
        let k = self.spec.key_column;
        let sql = format!(
            "SELECT k FROM (SELECT {k} AS k, rank AS rk FROM {t} WHERE {t} MATCH ?1) \
             GROUP BY k ORDER BY MIN(rk), k LIMIT ?2"
        );
        let mut stmt = conn.prepare_cached(&sql).map_err(idx_err)?;
        let rows = stmt
            .query_map(rusqlite::params![fq, limit as i64], |r| r.get::<_, i64>(0))
            .map_err(idx_err)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(idx_err)
    }
}

/// `"tok1"* "tok2"*` from the alphanumeric tokens of `q`; `None` when there are none.
/// Use in repository SQL too (`... MATCH ?`).
pub fn fts_prefix_query(q: &str) -> Option<String> {
    let toks: Vec<String> = q
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .map(|t| format!("\"{t}\"*"))
        .collect();
    if toks.is_empty() { None } else { Some(toks.join(" ")) }
}

/// Post-commit sync: `reindex(key)`; on error logs and continues (startup `ensure_fts_index`
/// repairs). Never fails the command.
pub fn sync_after_commit(index: &dyn FtsIndex, key: i64, ctx: &str) {
    if let Err(e) = index.reindex(key) {
        eprintln!("[search] {ctx} #{key} reindex failed: {e}");
    }
}
