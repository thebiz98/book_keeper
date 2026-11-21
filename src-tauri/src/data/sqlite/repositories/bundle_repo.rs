//! SQLite implementation of `BundleRepository`.
use rusqlite::{params, OptionalExtension, Row};

use crate::core::dtos::BundleSuggestion;
use crate::core::entities::ItemBundle;
use crate::core::error::AppError;
use crate::data::contracts::BundleRepository;
use crate::data::sqlite::SqliteConn;

pub struct SqliteBundleRepository {
    conn: SqliteConn,
}

impl SqliteBundleRepository {
    pub fn new(conn: SqliteConn) -> Self {
        Self { conn }
    }
}

const COLS: &str = "id, item_id, bundle_name, units_per_bundle, mrp, default_bundle_price, barcode";

fn map_bundle(r: &Row<'_>) -> rusqlite::Result<ItemBundle> {
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

impl BundleRepository for SqliteBundleRepository {
    fn find_by_id(&self, id: i64) -> Result<Option<ItemBundle>, AppError> {
        Ok(self
            .conn
            .query_row(&format!("SELECT {COLS} FROM item_bundles WHERE id = ?1"), params![id], map_bundle)
            .optional()?)
    }

    fn find_by_item_id(&self, item_id: i64) -> Result<Vec<ItemBundle>, AppError> {
        let mut stmt = self
            .conn
            .prepare_cached(&format!("SELECT {COLS} FROM item_bundles WHERE item_id = ?1 ORDER BY id"))?;
        let rows = stmt.query_map(params![item_id], map_bundle)?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    fn find_by_item_ids(&self, item_ids: &[i64]) -> Result<Vec<ItemBundle>, AppError> {
        if item_ids.is_empty() {
            return Ok(Vec::new());
        }
        let ph = vec!["?"; item_ids.len()].join(",");
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {COLS} FROM item_bundles WHERE item_id IN ({ph}) ORDER BY id"
        ))?;
        let rows = stmt
            .query_map(rusqlite::params_from_iter(item_ids.iter()), map_bundle)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    fn find_by_barcode(&self, barcode: &str) -> Result<Option<ItemBundle>, AppError> {
        Ok(self
            .conn
            .query_row(
                &format!("SELECT {COLS} FROM item_bundles WHERE barcode = ?1 ORDER BY id LIMIT 1"),
                params![barcode],
                map_bundle,
            )
            .optional()?)
    }

    fn create(&mut self, b: &ItemBundle) -> Result<i64, AppError> {
        let id = if b.id > 0 { Some(b.id) } else { None };
        self.conn.execute(
            &format!("INSERT INTO item_bundles ({COLS}) VALUES (?1,?2,?3,?4,?5,?6,?7)"),
            params![id, b.item_id, b.bundle_name, b.units_per_bundle, b.mrp, b.default_bundle_price, b.barcode],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    fn update(&mut self, b: &ItemBundle) -> Result<(), AppError> {
        let n = self.conn.execute(
            "UPDATE item_bundles SET item_id=?2, bundle_name=?3, units_per_bundle=?4, mrp=?5, \
             default_bundle_price=?6, barcode=?7 WHERE id=?1",
            params![b.id, b.item_id, b.bundle_name, b.units_per_bundle, b.mrp, b.default_bundle_price, b.barcode],
        )?;
        if n == 0 {
            return Err(AppError::NotFound { entity: "ItemBundle".to_string(), id: b.id.to_string() });
        }
        Ok(())
    }

    fn delete(&mut self, bundle_id: i64) -> Result<(), AppError> {
        let n = self.conn.execute("DELETE FROM item_bundles WHERE id = ?1", params![bundle_id])?;
        if n == 0 {
            return Err(AppError::NotFound { entity: "ItemBundle".to_string(), id: bundle_id.to_string() });
        }
        Ok(())
    }

    fn get_bundle_suggestions(&self) -> Result<Vec<BundleSuggestion>, AppError> {
        // Distinct (name NOCASE, multiplier) over bundles of active items, most used first.
        // The multiplier is never rounded; the display name is the lexicographically first spelling.
        let mut stmt = self.conn.prepare_cached(
            "SELECT MIN(b.bundle_name), b.units_per_bundle, COUNT(*) AS c \
             FROM item_bundles b JOIN items i ON i.id = b.item_id \
             WHERE i.is_active = 1 \
             GROUP BY b.bundle_name COLLATE NOCASE, b.units_per_bundle \
             ORDER BY c DESC, b.bundle_name COLLATE NOCASE, b.units_per_bundle",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(BundleSuggestion {
                bundle_name: r.get(0)?,
                units_per_bundle: r.get(1)?,
                usage_count: r.get(2)?,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }
}
