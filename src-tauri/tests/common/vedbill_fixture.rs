//! Builds a synthetic VedBill backup (`.vb` = ZIP with `company_manifest.json` + `company.db`)
//! using the subset of VedBill schema v8 the importer reads. No customer data is committed.

use std::io::Write;
use std::path::PathBuf;

pub struct VbItem<'a> {
    pub id: &'a str,
    pub name: &'a str,
    pub item_type: &'a str,
    pub unit: &'a str,
    pub sale_price: f64,
    pub purchase_price: Option<f64>,
    pub mrp: f64,
    pub primary_quantity: f64,
}

/// (item id, lot date, purchase price, remaining qty)
pub type VbLot<'a> = (&'a str, &'a str, f64, f64);

pub fn build_vb(items: &[VbItem], lots: &[VbLot], schema_version: i64) -> PathBuf {
    super::sweep_stale_temp();
    let dir = std::env::temp_dir().join(format!("bk_vb_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let db = dir.join("company.db");
    {
        let c = rusqlite::Connection::open(&db).unwrap();
        c.execute_batch(
            "CREATE TABLE units (id TEXT PRIMARY KEY, firmId TEXT NOT NULL, fullname TEXT NOT NULL, shortname TEXT NOT NULL,
                createdAt TEXT NOT NULL, updatedAt TEXT NOT NULL);
             CREATE TABLE unit_conversions (id TEXT PRIMARY KEY, firmId TEXT NOT NULL, primaryUnitId TEXT NOT NULL,
                secondaryUnitId TEXT NOT NULL, conversionRate REAL NOT NULL, createdAt TEXT NOT NULL, updatedAt TEXT NOT NULL);
             CREATE TABLE items (id TEXT PRIMARY KEY, firmId TEXT NOT NULL, name TEXT NOT NULL, type TEXT NOT NULL,
                hsnCode TEXT, itemCode TEXT, categoryId TEXT, unit_conversionId TEXT, salePrice REAL NOT NULL,
                purchasePrice REAL, mrp REAL DEFAULT 0, primaryQuantity REAL, openingStockDate TEXT,
                isActive INTEGER NOT NULL DEFAULT 1, createdAt TEXT NOT NULL, updatedAt TEXT NOT NULL);
             CREATE TABLE stock_lots (id TEXT PRIMARY KEY, firmId TEXT NOT NULL, itemId TEXT NOT NULL, godownId TEXT NOT NULL,
                documentId TEXT, lotDate TEXT NOT NULL, purchasePrice REAL NOT NULL,
                remainingPrimaryQty REAL NOT NULL DEFAULT 0, remainingSecondaryQty REAL NOT NULL DEFAULT 0,
                createdAt TEXT NOT NULL);",
        )
        .unwrap();
        for (i, it) in items.iter().enumerate() {
            let unit_id = format!("u-{}", it.unit);
            c.execute(
                "INSERT OR IGNORE INTO units VALUES (?1, 'f', ?2, ?2, 'x', 'x')",
                rusqlite::params![unit_id, it.unit],
            )
            .unwrap();
            let conv = format!("c-{i}");
            c.execute(
                "INSERT INTO unit_conversions VALUES (?1, 'f', ?2, ?2, 1, 'x', 'x')",
                rusqlite::params![conv, unit_id],
            )
            .unwrap();
            c.execute(
                "INSERT INTO items (id, firmId, name, type, unit_conversionId, salePrice, purchasePrice, mrp, primaryQuantity,
                    createdAt, updatedAt) VALUES (?1, 'f', ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'x', 'x')",
                rusqlite::params![it.id, it.name, it.item_type, conv, it.sale_price, it.purchase_price, it.mrp, it.primary_quantity],
            )
            .unwrap();
        }
        for (i, (item, date, cost, qty)) in lots.iter().enumerate() {
            c.execute(
                "INSERT INTO stock_lots (id, firmId, itemId, godownId, lotDate, purchasePrice, remainingPrimaryQty, createdAt)
                 VALUES (?1, 'f', ?2, 'g', ?3, ?4, ?5, ?6)",
                rusqlite::params![format!("l-{i}"), item, date, cost, qty, format!("2026-01-01T00:00:{i:02}Z")],
            )
            .unwrap();
        }
    }

    let vb = dir.join("TEST COMPANY-backup-2026-11-22-1405.vb");
    let mut zip = zip::ZipWriter::new(std::fs::File::create(&vb).unwrap());
    let opts = zip::write::SimpleFileOptions::default();
    zip.start_file("company_manifest.json", opts).unwrap();
    zip.write_all(format!(r#"{{"companyName":"TEST COMPANY","schemaVersion":{schema_version}}}"#).as_bytes())
        .unwrap();
    zip.add_directory("product-images/", opts).unwrap();
    zip.start_file("company.db", opts).unwrap();
    zip.write_all(&std::fs::read(&db).unwrap()).unwrap();
    zip.finish().unwrap();
    vb
}

pub fn product<'a>(id: &'a str, name: &'a str, qty: f64, cost: f64) -> VbItem<'a> {
    VbItem {
        id,
        name,
        item_type: "PRODUCT",
        unit: "PCS",
        sale_price: 10.0,
        purchase_price: Some(cost),
        mrp: 12.0,
        primary_quantity: qty,
    }
}
