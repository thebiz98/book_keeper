//! Reader for VedBill backups (`.vb` = ZIP holding `company_manifest.json` + `company.db`).
//! Only the `company.db` entry is extracted (by exact name, so no path traversal), into a
//! private temp folder that is deleted when reading finishes. The database is opened read-only.

use std::collections::HashSet;
use std::io::Read;
use std::path::{Path, PathBuf};
use rusqlite::{Connection, OpenFlags};
use crate::core::error::AppError;
use crate::data::contracts::vedbill_reader::{
    VedbillBackupReader, VedbillItem, VedbillLot, VedbillManifest, VedbillSnapshot,
};

/// VedBill schema version this reader was written against.
pub const SUPPORTED_SCHEMA_VERSION: i64 = 8;

/// Largest `company.db` we are willing to extract (guards against a corrupt or hostile archive).
const MAX_DB_BYTES: u64 = 1024 * 1024 * 1024;

#[derive(Debug, Default, Clone, Copy)]
pub struct ZipVedbillReader;

fn bad_file(message: impl Into<String>) -> AppError {
    AppError::ValidationError { field: "file".to_string(), message: message.into() }
}

const NOT_VB: &str = "This file is not a VedBill backup. Pick a .vb file from Documents\\VEDBIL \
(named like \"<company>-backup-YYYY-MM-DD-HHMM.vb\").";

const TOO_LARGE: &str = "The database inside this backup is unexpectedly large; it may be damaged.";

/// Deletes the temp folder on drop.
struct TempDir(PathBuf);

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn columns(conn: &Connection, table: &str) -> Result<HashSet<String>, AppError> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info(\"{table}\")"))?;
    let cols = stmt.query_map([], |r| r.get::<_, String>(1))?.collect::<Result<HashSet<_>, _>>()?;
    Ok(cols)
}

/// `alias.col` if present, else SQL NULL (so slightly older/newer schemas still read).
fn opt(cols: &HashSet<String>, alias: &str, col: &str) -> String {
    if cols.contains(col) {
        format!("{alias}.\"{col}\"")
    } else {
        "NULL".to_string()
    }
}

fn require(cols: &HashSet<String>, table: &str, needed: &[&str]) -> Result<(), AppError> {
    if cols.is_empty() {
        return Err(bad_file(format!("This backup's format is not supported: the \"{table}\" table is missing.")));
    }
    if let Some(c) = needed.iter().find(|c| !cols.contains(**c)) {
        return Err(bad_file(format!(
            "This backup's format is not supported: column {table}.{c} is missing. It may come from a different VedBill version."
        )));
    }
    Ok(())
}

/// Reads items and stock lots from an extracted VedBill `company.db`.
pub fn read_company_db(db_path: &Path, manifest: VedbillManifest) -> Result<VedbillSnapshot, AppError> {
    let conn = Connection::open_with_flags(db_path, OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX)
        .map_err(|e| bad_file(format!("The database inside the backup could not be opened: {e}")))?;
    conn.query_row("SELECT COUNT(*) FROM sqlite_master", [], |r| r.get::<_, i64>(0))
        .map_err(|_| bad_file("The database inside the backup is damaged. Try a different backup file."))?;

    let icols = columns(&conn, "items")?;
    require(&icols, "items", &["id", "name", "salePrice"])?;
    let ucols = columns(&conn, "units")?;
    let ccols = columns(&conn, "unit_conversions")?;
    let has_units = icols.contains("unit_conversionId")
        && ucols.contains("id")
        && ucols.contains("shortname")
        && ccols.contains("id")
        && ccols.contains("primaryUnitId");
    let (unit_expr, joins) = if has_units {
        (
            "u.shortname",
            "LEFT JOIN unit_conversions uc ON uc.id = i.unit_conversionId LEFT JOIN units u ON u.id = uc.primaryUnitId",
        )
    } else {
        ("NULL", "")
    };
    let sql = format!(
        "SELECT i.id, i.name, {typ}, {unit_expr}, i.salePrice, {pp}, {mrp}, {qty}, {active}, {osd} \
         FROM items i {joins} ORDER BY i.rowid",
        typ = opt(&icols, "i", "type"),
        pp = opt(&icols, "i", "purchasePrice"),
        mrp = opt(&icols, "i", "mrp"),
        qty = opt(&icols, "i", "primaryQuantity"),
        active = opt(&icols, "i", "isActive"),
        osd = opt(&icols, "i", "openingStockDate"),
    );
    let mut stmt = conn.prepare(&sql)?;
    let items = stmt
        .query_map([], |r| {
            Ok(VedbillItem {
                legacy_id: r.get::<_, String>(0)?,
                name: r.get::<_, Option<String>>(1)?.unwrap_or_default(),
                item_type: r.get(2)?,
                unit: r.get(3)?,
                sale_price: r.get::<_, Option<f64>>(4)?.unwrap_or(0.0),
                purchase_price: r.get(5)?,
                mrp: r.get(6)?,
                primary_quantity: r.get(7)?,
                is_active: r.get::<_, Option<i64>>(8)?.map(|v| v != 0).unwrap_or(true),
                opening_stock_date: r.get(9)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    drop(stmt);

    let lcols = columns(&conn, "stock_lots")?;
    let mut lots = Vec::new();
    if !lcols.is_empty() {
        require(&lcols, "stock_lots", &["itemId", "lotDate", "purchasePrice", "remainingPrimaryQty"])?;
        let created = if lcols.contains("createdAt") { "l.createdAt" } else { "NULL" };
        let sql = format!(
            "SELECT l.itemId, l.lotDate, l.purchasePrice, l.remainingPrimaryQty FROM stock_lots l \
             ORDER BY l.itemId, substr(l.lotDate, 1, 10), {created}, l.rowid"
        );
        let mut stmt = conn.prepare(&sql)?;
        lots = stmt
            .query_map([], |r| {
                Ok(VedbillLot {
                    item_id: r.get(0)?,
                    lot_date: r.get::<_, Option<String>>(1)?.unwrap_or_default(),
                    purchase_price: r.get::<_, Option<f64>>(2)?.unwrap_or(0.0),
                    remaining_qty: r.get::<_, Option<f64>>(3)?.unwrap_or(0.0),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
    }

    Ok(VedbillSnapshot { manifest, items, lots })
}

fn parse_manifest(text: &str) -> VedbillManifest {
    let v: serde_json::Value = serde_json::from_str(text).unwrap_or(serde_json::Value::Null);
    VedbillManifest {
        company_name: v.get("companyName").and_then(|x| x.as_str()).map(str::to_string),
        schema_version: v.get("schemaVersion").and_then(|x| x.as_i64()),
    }
}

impl VedbillBackupReader for ZipVedbillReader {
    fn read(&self, path: &str) -> Result<VedbillSnapshot, AppError> {
        let path = path.trim();
        if !path.to_lowercase().ends_with(".vb") {
            return Err(bad_file(NOT_VB));
        }
        let file = std::fs::File::open(path)
            .map_err(|e| AppError::IoError(format!("Could not open the backup file: {e}")))?;
        let mut zip = zip::ZipArchive::new(file).map_err(|_| bad_file(NOT_VB))?;

        let manifest = match zip.by_name("company_manifest.json") {
            Ok(mut f) => {
                let mut s = String::new();
                let _ = f.read_to_string(&mut s);
                parse_manifest(&s)
            }
            Err(_) => VedbillManifest::default(),
        };

        let tmp = TempDir(std::env::temp_dir().join(format!("book_keeper_vedbill_{}", uuid::Uuid::new_v4())));
        std::fs::create_dir_all(&tmp.0)?;
        let db_path = tmp.0.join("company.db");
        {
            let entry = zip.by_name("company.db").map_err(|_| {
                bad_file("This backup has no company.db inside. It may be incomplete; try a different backup file.")
            })?;
            if entry.size() > MAX_DB_BYTES {
                return Err(bad_file(TOO_LARGE));
            }
            let mut out = std::fs::File::create(&db_path)?;
            let copied = std::io::copy(&mut entry.take(MAX_DB_BYTES + 1), &mut out)
                .map_err(|e| bad_file(format!("The backup could not be unpacked: {e}")))?;
            if copied > MAX_DB_BYTES {
                return Err(bad_file(TOO_LARGE));
            }
        }
        // The snapshot holds no connection, so the temp folder is deleted when `tmp` drops.
        read_company_db(&db_path, manifest)
    }
}
