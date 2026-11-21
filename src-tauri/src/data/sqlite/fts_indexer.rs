//! FTS5-backed typeahead search index (`item_search_fts`).
use rusqlite::{params, Connection};

use crate::core::dtos::{ItemBundleSearchResult, ItemSearchResult};
use crate::core::entities::{Item, ItemBundle};
use crate::core::error::AppError;
use crate::data::contracts::search_indexer::ItemSearchIndexer;
use super::connection::SqlitePool;

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

fn load_bundles(conn: &Connection, item_id: i64) -> Result<Vec<ItemBundle>, AppError> {
    let mut stmt = conn
        .prepare_cached(
            "SELECT id, item_id, bundle_name, units_per_bundle, mrp, default_bundle_price, barcode \
             FROM item_bundles WHERE item_id = ?1 ORDER BY id",
        )
        .map_err(idx_err)?;
    let rows = stmt
        .query_map(params![item_id], |r| {
            Ok(ItemBundle {
                id: r.get(0)?,
                item_id: r.get(1)?,
                bundle_name: r.get(2)?,
                units_per_bundle: r.get(3)?,
                mrp: r.get(4)?,
                default_bundle_price: r.get(5)?,
                barcode: r.get(6)?,
            })
        })
        .map_err(idx_err)?;
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

impl ItemSearchIndexer for SqliteFts5SearchIndexer {
    fn search(&self, query: &str) -> Result<Vec<ItemSearchResult>, AppError> {
        self.search_with_limit(query, 50)
    }

    fn search_with_limit(&self, query: &str, limit: usize) -> Result<Vec<ItemSearchResult>, AppError> {
        let clean_q = query.trim().trim_end_matches('*').trim();
        if clean_q.is_empty() || limit == 0 {
            return Ok(Vec::new());
        }
        let conn = self.conn()?;

        let mut ids: Vec<i64> = Vec::new();
        if let Some(fq) = fts_query(clean_q) {
            let mut stmt = conn
                .prepare_cached(
                    "SELECT f.item_id FROM item_search_fts f JOIN items i ON i.id = f.item_id \
                     WHERE i.is_active = 1 AND item_search_fts MATCH ?1 ORDER BY f.rank",
                )
                .map_err(idx_err)?;
            let rows = stmt.query_map(params![fq], |r| r.get::<_, i64>(0));
            if let Ok(rows) = rows {
                for id in rows.flatten() {
                    if !ids.contains(&id) {
                        ids.push(id);
                    }
                }
            }
        }
        if ids.is_empty() {
            // Substring fallback (matches the reference test double's short-query behaviour).
            let mut stmt = conn
                .prepare_cached(
                    "SELECT id FROM items WHERE is_active = 1 AND (name LIKE '%' || ?1 || '%' \
                     OR barcode LIKE '%' || ?1 || '%' OR category LIKE '%' || ?1 || '%') ORDER BY id LIMIT 1000",
                )
                .map_err(idx_err)?;
            let rows = stmt.query_map(params![clean_q], |r| r.get::<_, i64>(0)).map_err(idx_err)?;
            ids = rows.flatten().collect();
        }

        let mut scored: Vec<(f64, ItemSearchResult)> = Vec::new();
        for id in ids {
            let item = conn
                .query_row(
                    "SELECT name, category, barcode, base_unit, mrp, default_sale_price, current_stock, is_low_stock \
                     FROM items WHERE id = ?1 AND is_active = 1",
                    params![id],
                    |r| {
                        Ok(ItemSearchResult {
                            item_id: id,
                            name: r.get(0)?,
                            category: r.get(1)?,
                            barcode: r.get(2)?,
                            base_unit: r.get(3)?,
                            mrp: r.get(4)?,
                            default_sale_price: r.get(5)?,
                            current_stock: r.get(6)?,
                            is_low_stock: r.get(7)?,
                            bundles: Vec::new(),
                        })
                    },
                )
                .ok();
            let Some(mut res) = item else { continue };
            let bundles = load_bundles(&conn, id)?;
            let exact = res.barcode.as_deref() == Some(clean_q)
                || bundles.iter().any(|b| b.barcode.as_deref() == Some(clean_q));
            res.bundles = bundles
                .into_iter()
                .map(|b| ItemBundleSearchResult {
                    bundle_id: b.id,
                    bundle_name: b.bundle_name,
                    units_per_bundle: b.units_per_bundle,
                    bundle_price: b.default_bundle_price,
                    barcode: b.barcode,
                })
                .collect();
            scored.push((if exact { 10.0 } else { 1.0 }, res));
        }
        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        Ok(scored.into_iter().map(|r| r.1).take(limit).collect())
    }

    fn index_item(&self, item: &Item) -> Result<(), AppError> {
        self.index_item_with_bundles(item, &[])
    }

    fn index_item_with_bundles(&self, item: &Item, bundles: &[ItemBundle]) -> Result<(), AppError> {
        let conn = self.conn()?;
        conn.execute("DELETE FROM item_search_fts WHERE item_id = ?1", params![item.id]).map_err(idx_err)?;
        if bundles.is_empty() {
            let stored = load_bundles(&conn, item.id)?;
            insert_rows(&conn, item, &stored)
        } else {
            insert_rows(&conn, item, bundles)
        }
    }

    fn remove_item(&self, item_id: i64) -> Result<(), AppError> {
        let conn = self.conn()?;
        conn.execute("DELETE FROM item_search_fts WHERE item_id = ?1", params![item_id]).map_err(idx_err)?;
        Ok(())
    }

    fn rebuild_index(&self) -> Result<(), AppError> {
        let conn = self.conn()?;
        let items: Vec<Item> = {
            let mut stmt = conn
                .prepare(
                    "SELECT id, name, category, barcode, base_unit, mrp, default_sale_price, current_stock, \
                     is_low_stock, is_active, created_at, updated_at FROM items WHERE is_active = 1",
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
                    })
                })
                .map_err(idx_err)?;
            rows.collect::<Result<Vec<_>, _>>().map_err(idx_err)?
        };
        conn.execute("DELETE FROM item_search_fts", []).map_err(idx_err)?;
        for item in &items {
            let bundles = load_bundles(&conn, item.id)?;
            insert_rows(&conn, item, &bundles)?;
        }
        Ok(())
    }

    fn rebuild_index_with_bundles(&self, items_with_bundles: &[(Item, Vec<ItemBundle>)]) -> Result<(), AppError> {
        if items_with_bundles.is_empty() {
            return self.rebuild_index();
        }
        let conn = self.conn()?;
        conn.execute("DELETE FROM item_search_fts", []).map_err(idx_err)?;
        for (item, bundles) in items_with_bundles {
            insert_rows(&conn, item, bundles)?;
        }
        Ok(())
    }
}
