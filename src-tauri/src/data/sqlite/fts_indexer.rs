//! FTS5-backed typeahead search index (`item_search_fts`).
use rusqlite::{params, Connection};

use crate::core::dtos::{ItemBundleSearchResult, ItemSearchResult};
use crate::core::entities::{Item, ItemBundle};
use crate::core::error::AppError;
use crate::data::contracts::search_indexer::ItemSearchIndexer;
use super::connection::SqlitePool;
use super::enum_col;

pub struct SqliteFts5SearchIndexer {
    pool: SqlitePool,
}

impl SqliteFts5SearchIndexer {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    fn conn(&self) -> Result<r2d2::PooledConnection<r2d2_sqlite::SqliteConnectionManager>, AppError> {
        self.pool
            .get()
            .map_err(|e| AppError::SearchIndexError(format!("pool checkout failed: {e}")))
    }
}

fn idx_err(e: rusqlite::Error) -> AppError {
    AppError::SearchIndexError(e.to_string())
}

fn insert_rows(conn: &Connection, item: &Item, bundles: &[ItemBundle]) -> Result<(), AppError> {
    if !item.is_active {
        return Ok(());
    }
    conn.execute(
        "INSERT INTO item_search_fts (item_id, name, category, barcode) VALUES (?1, ?2, ?3, ?4)",
        params![item.id, item.name, item.category, item.barcode],
    )
    .map_err(idx_err)?;
    for b in bundles.iter().filter(|b| b.barcode.as_deref().is_some_and(|s| !s.is_empty())) {
        conn.execute(
            "INSERT INTO item_search_fts (item_id, name, category, barcode) VALUES (?1, ?2, ?3, ?4)",
            params![item.id, item.name, item.category, b.barcode],
        )
        .map_err(idx_err)?;
    }
    Ok(())
}

fn map_bundle(r: &rusqlite::Row<'_>) -> rusqlite::Result<ItemBundle> {
    Ok(ItemBundle {
        id: r.get(0)?,
        item_id: r.get(1)?,
        bundle_name: r.get(2)?,
        units_per_bundle: r.get(3)?,
        mrp: r.get(4)?,
        default_bundle_price: r.get(5)?,
        barcode: r.get(6)?,
    })
}

fn load_bundles(conn: &Connection, item_id: i64) -> Result<Vec<ItemBundle>, AppError> {
    let mut stmt = conn
        .prepare_cached(
            "SELECT id, item_id, bundle_name, units_per_bundle, mrp, default_bundle_price, barcode \
             FROM item_bundles WHERE item_id = ?1 ORDER BY id",
        )
        .map_err(idx_err)?;
    let rows = stmt.query_map(params![item_id], map_bundle).map_err(idx_err)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(idx_err)
}

fn fts_query(q: &str) -> Option<String> {
    let toks: Vec<String> = q
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .map(|t| format!("\"{t}\"*"))
        .collect();
    if toks.is_empty() { None } else { Some(toks.join(" ")) }
}

/// Default typeahead page size.
pub const DEFAULT_SEARCH_LIMIT: usize = 8;
const MAX_SEARCH_LIMIT: usize = 200;

fn placeholders(n: usize) -> String {
    vec!["?"; n].join(",")
}

fn map_item_result(r: &rusqlite::Row<'_>) -> rusqlite::Result<ItemSearchResult> {
    Ok(ItemSearchResult {
        item_id: r.get(0)?,
        name: r.get(1)?,
        category: r.get(2)?,
        barcode: r.get(3)?,
        base_unit: r.get(4)?,
        mrp: r.get(5)?,
        default_sale_price: r.get(6)?,
        current_stock: r.get(7)?,
        is_low_stock: r.get(8)?,
        item_type: enum_col(r, 9)?,
        bundles: Vec::new(),
        matched_bundle_id: None,
    })
}

fn load_results(conn: &Connection, ids: Vec<i64>, clean_q: &str) -> Result<Vec<ItemSearchResult>, AppError> {
    // Batched loads: one query for items, one for bundles (no N+1).
    let ph = placeholders(ids.len());
    let mut by_id: std::collections::HashMap<i64, ItemSearchResult> = {
        let mut stmt = conn
            .prepare(&format!(
                "SELECT id, name, category, barcode, base_unit, mrp, default_sale_price, current_stock, \
                 is_low_stock, item_type FROM items WHERE is_active = 1 AND id IN ({ph})"
            ))
            .map_err(idx_err)?;
        let rows = stmt
            .query_map(rusqlite::params_from_iter(ids.iter()), map_item_result)
            .map_err(idx_err)?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(idx_err)?
            .into_iter()
            .map(|r| (r.item_id, r))
            .collect()
    };
    let bundles: Vec<ItemBundle> = {
        let mut stmt = conn
            .prepare(&format!(
                "SELECT id, item_id, bundle_name, units_per_bundle, mrp, default_bundle_price, barcode \
                 FROM item_bundles WHERE item_id IN ({ph}) ORDER BY id"
            ))
            .map_err(idx_err)?;
        let rows = stmt
            .query_map(rusqlite::params_from_iter(ids.iter()), map_bundle)
            .map_err(idx_err)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(idx_err)?
    };
    for b in bundles {
        let Some(res) = by_id.get_mut(&b.item_id) else { continue };
        let bc = b.barcode.clone();
        let item_hit = res.barcode.as_deref().is_some_and(|x| x == clean_q);
        if res.matched_bundle_id.is_none() && !item_hit {
            if let Some(bc) = bc.as_deref() {
                // Exact bundle barcode wins; otherwise a bundle barcode prefix that the
                // item's own barcode does not satisfy.
                let exact = bc == clean_q;
                let prefix = bc.starts_with(clean_q)
                    && !res.barcode.as_deref().is_some_and(|x| x.starts_with(clean_q));
                if exact || prefix {
                    res.matched_bundle_id = Some(b.id);
                }
            }
        }
        res.bundles.push(ItemBundleSearchResult {
            bundle_id: b.id,
            bundle_name: b.bundle_name,
            units_per_bundle: b.units_per_bundle,
            bundle_price: b.default_bundle_price,
            barcode: b.barcode,
        });
    }
    Ok(ids.into_iter().filter_map(|id| by_id.remove(&id)).collect())
}

impl ItemSearchIndexer for SqliteFts5SearchIndexer {
    fn search(&self, query: &str) -> Result<Vec<ItemSearchResult>, AppError> {
        self.search_with_limit(query, DEFAULT_SEARCH_LIMIT)
    }

    fn search_with_limit(&self, query: &str, limit: usize) -> Result<Vec<ItemSearchResult>, AppError> {
        let clean_q = query.trim().trim_end_matches('*').trim();
        if clean_q.is_empty() || limit == 0 {
            return Ok(Vec::new());
        }
        let Some(fq) = fts_query(clean_q) else { return Ok(Vec::new()) };
        let limit = limit.min(MAX_SEARCH_LIMIT);
        let conn = self.conn()?;

        // One row per item: exact barcode (item or bundle row) first, then FTS rank. LIMIT in SQL.
        let ids: Vec<i64> = {
            let mut stmt = conn
                .prepare_cached(
                    "SELECT item_id FROM ( \
                       SELECT f.item_id AS item_id, (f.barcode = ?2) AS ex, f.rank AS rk \
                       FROM item_search_fts f JOIN items i ON i.id = f.item_id \
                       WHERE i.is_active = 1 AND item_search_fts MATCH ?1) \
                     GROUP BY item_id ORDER BY MAX(ex) DESC, MIN(rk), item_id LIMIT ?3",
                )
                .map_err(idx_err)?;
            let rows = stmt
                .query_map(params![fq, clean_q, limit as i64], |r| r.get::<_, i64>(0))
                .map_err(idx_err)?;
            rows.collect::<Result<Vec<_>, _>>().map_err(idx_err)?
        };
        if ids.is_empty() {
            return Ok(Vec::new());
        }

        load_results(&conn, ids, clean_q)
    }

    fn search_page(
        &self,
        query: &str,
        category: Option<&str>,
        low_stock_only: bool,
        offset: usize,
        limit: usize,
    ) -> Result<(Vec<ItemSearchResult>, usize), AppError> {
        let clean_q = query.trim().trim_end_matches('*').trim();
        let Some(fq) = fts_query(clean_q) else { return Ok((Vec::new(), 0)) };
        let conn = self.conn()?;
        let total: i64 = conn
            .query_row(
                "SELECT COUNT(DISTINCT f.item_id) FROM item_search_fts f JOIN items i ON i.id = f.item_id \
                 WHERE i.is_active = 1 AND item_search_fts MATCH ?1 \
                 AND (?2 IS NULL OR i.category = ?2 COLLATE NOCASE) AND (?3 = 0 OR i.is_low_stock = 1)",
                params![fq, category, low_stock_only as i64],
                |r| r.get(0),
            )
            .map_err(idx_err)?;
        let ids: Vec<i64> = {
            let mut stmt = conn
                .prepare_cached(
                    "SELECT item_id FROM ( \
                       SELECT f.item_id AS item_id, (f.barcode = ?2) AS ex, f.rank AS rk \
                       FROM item_search_fts f JOIN items i ON i.id = f.item_id \
                       WHERE i.is_active = 1 AND item_search_fts MATCH ?1 \
                       AND (?5 IS NULL OR i.category = ?5 COLLATE NOCASE) AND (?6 = 0 OR i.is_low_stock = 1)) \
                     GROUP BY item_id ORDER BY MAX(ex) DESC, MIN(rk), item_id LIMIT ?3 OFFSET ?4",
                )
                .map_err(idx_err)?;
            let rows = stmt
                .query_map(
                    params![fq, clean_q, limit as i64, offset as i64, category, low_stock_only as i64],
                    |r| r.get::<_, i64>(0),
                )
                .map_err(idx_err)?;
            rows.collect::<Result<Vec<_>, _>>().map_err(idx_err)?
        };
        if ids.is_empty() {
            return Ok((Vec::new(), total.max(0) as usize));
        }
        Ok((load_results(&conn, ids, clean_q)?, total.max(0) as usize))
    }

    fn index_item(&self, item: &Item) -> Result<(), AppError> {
        self.index_item_with_bundles(item, &[])
    }

    fn index_item_with_bundles(&self, item: &Item, bundles: &[ItemBundle]) -> Result<(), AppError> {
        let conn = self.conn()?;
        // Delete + insert are one atomic unit on this connection.
        let tx = conn.unchecked_transaction().map_err(idx_err)?;
        tx.execute("DELETE FROM item_search_fts WHERE item_id = ?1", params![item.id]).map_err(idx_err)?;
        if bundles.is_empty() {
            let stored = load_bundles(&tx, item.id)?;
            insert_rows(&tx, item, &stored)?;
        } else {
            insert_rows(&tx, item, bundles)?;
        }
        tx.commit().map_err(idx_err)
    }

    fn remove_item(&self, item_id: i64) -> Result<(), AppError> {
        let conn = self.conn()?;
        conn.execute("DELETE FROM item_search_fts WHERE item_id = ?1", params![item_id]).map_err(idx_err)?;
        Ok(())
    }

    fn indexed_item_count(&self) -> Result<usize, AppError> {
        let conn = self.conn()?;
        let n: i64 = conn
            .query_row("SELECT COUNT(DISTINCT item_id) FROM item_search_fts", [], |r| r.get(0))
            .map_err(idx_err)?;
        Ok(n as usize)
    }

    fn rebuild_index(&self) -> Result<(), AppError> {
        let conn = self.conn()?;
        let tx = conn.unchecked_transaction().map_err(idx_err)?;
        let items: Vec<Item> = {
            let mut stmt = tx
                .prepare(
                    "SELECT id, name, category, barcode, base_unit, mrp, default_sale_price, current_stock, \
                     is_low_stock, is_active, created_at, updated_at, item_type FROM items WHERE is_active = 1",
                )
                .map_err(idx_err)?;
            let rows = stmt
                .query_map([], |r| {
                    Ok(Item {
                        id: r.get(0)?,
                        name: r.get(1)?,
                        category: r.get(2)?,
                        barcode: r.get(3)?,
                        base_unit: r.get(4)?,
                        mrp: r.get(5)?,
                        default_sale_price: r.get(6)?,
                        current_stock: r.get(7)?,
                        is_low_stock: r.get(8)?,
                        is_active: r.get(9)?,
                        created_at: r.get(10)?,
                        updated_at: r.get(11)?,
                        item_type: enum_col(r, 12)?,
                    })
                })
                .map_err(idx_err)?;
            rows.collect::<Result<Vec<_>, _>>().map_err(idx_err)?
        };
        tx.execute("DELETE FROM item_search_fts", []).map_err(idx_err)?;
        for item in &items {
            let bundles = load_bundles(&tx, item.id)?;
            insert_rows(&tx, item, &bundles)?;
        }
        tx.commit().map_err(idx_err)
    }
}
