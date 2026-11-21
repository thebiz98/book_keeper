use std::fs::File;
use std::sync::Arc;
use crate::core::dtos::{BackupFileInfo, MigrationPreview, MigrationResult, TableStats};
use crate::core::entities::{Item, Party, PurchaseItem, PurchaseTransaction};
use crate::core::error::AppError;
use crate::core::value_objects::{PartyType, PaymentStatus};
use crate::data::contracts::search_indexer::ItemSearchIndexer;
use crate::data::contracts::unit_of_work::UnitOfWork;
use crate::services::contracts::migration_service::MigrationService;

#[derive(Clone)]
pub struct MigrationServiceImpl<U: UnitOfWork = Arc<dyn UnitOfWork>> {
    pub uow: U,
    pub search_indexer: Option<Arc<dyn ItemSearchIndexer>>,
}

impl<U: UnitOfWork> MigrationServiceImpl<U> {
    pub fn new(uow: U) -> Self {
        Self {
            uow,
            search_indexer: None,
        }
    }

    pub fn with_indexer(uow: U, search_indexer: Arc<dyn ItemSearchIndexer>) -> Self {
        Self {
            uow,
            search_indexer: Some(search_indexer),
        }
    }
}

impl<U: UnitOfWork> MigrationService for MigrationServiceImpl<U> {
    fn scan_backup_directory(&self) -> Result<Vec<BackupFileInfo>, AppError> {
        let mut results = Vec::new();
        let search_dirs = [
            std::env::var("USERPROFILE")
                .or_else(|_| std::env::var("HOME"))
                .ok()
                .map(|h| std::path::PathBuf::from(h).join("Documents").join("VEDBIL")),
            std::env::var("USERPROFILE")
                .or_else(|_| std::env::var("HOME"))
                .ok()
                .map(|h| std::path::PathBuf::from(h).join("Documents")),
            Some(std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."))),
        ];
        for dir in search_dirs.into_iter().flatten() {
            if dir.is_dir() {
                if let Ok(entries) = std::fs::read_dir(dir) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.is_file()
                            && path.extension().and_then(|ext| ext.to_str()) == Some("vb")
                        {
                            if let Ok(metadata) = entry.metadata() {
                                let file_name = path
                                    .file_name()
                                    .and_then(|n| n.to_str())
                                    .unwrap_or("unknown.vb")
                                    .to_string();
                                let file_path = path.to_string_lossy().to_string();
                                let file_size_bytes = metadata.len();
                                let modified_at = metadata
                                    .modified()
                                    .ok()
                                    .map(|t| {
                                        let dt: chrono::DateTime<chrono::Utc> = t.into();
                                        dt.to_rfc3339()
                                    })
                                    .unwrap_or_else(|| chrono::Utc::now().to_rfc3339());
                                results.push(BackupFileInfo {
                                    file_name,
                                    file_path,
                                    file_size_bytes,
                                    modified_at,
                                });
                            }
                        }
                    }
                }
            }
        }
        Ok(results)
    }

    fn inspect_backup_file(&self, archive_path: &str) -> Result<MigrationPreview, AppError> {
        let file = File::open(archive_path).map_err(|e| AppError::IoError(e.to_string()))?;
        let backup_file_size_bytes = file.metadata().map(|m| m.len()).unwrap_or(0);
        let mut archive = zip::ZipArchive::new(file)
            .map_err(|_| AppError::MigrationError("Invalid ZIP header".to_string()))?;

        #[derive(serde::Deserialize)]
        struct ManifestData {
            #[serde(alias = "company_name", alias = "companyName")]
            company_name: String,
            #[serde(alias = "version", alias = "schema_version", alias = "schemaVersion")]
            schema_version: serde_json::Value,
            #[serde(alias = "backup_date", alias = "created_at", alias = "createdAt", default)]
            created_at: String,
        }

        let (manifest, schema_version) = {
            let mut manifest_entry = archive
                .by_name("company_manifest.json")
                .map_err(|_| AppError::MigrationError("Missing company_manifest.json in archive".to_string()))?;

            let manifest: ManifestData = serde_json::from_reader(&mut manifest_entry)
                .map_err(|e| AppError::MigrationError(format!("Corrupt manifest: {}", e)))?;

            let version: i32 = match manifest.schema_version {
                serde_json::Value::Number(ref n) => n.as_i64().unwrap_or(1) as i32,
                serde_json::Value::String(ref s) => s.parse::<i32>().unwrap_or_else(|_| {
                    s.split('.').next().and_then(|p| p.parse().ok()).unwrap_or(1)
                }),
                _ => 1,
            };
            (manifest, version)
        };

        let mut db_entry = archive
            .by_name("company.db")
            .map_err(|_| AppError::MigrationError("Missing company.db in archive".to_string()))?;

        let temp_db_path = std::env::temp_dir().join(format!("vedbill_inspect_{}.db", uuid::Uuid::new_v4()));
        {
            let mut out_file = File::create(&temp_db_path)
                .map_err(|e| AppError::IoError(e.to_string()))?;
            std::io::copy(&mut db_entry, &mut out_file)
                .map_err(|e| AppError::IoError(e.to_string()))?;
        }

        let conn = rusqlite::Connection::open_with_flags(&temp_db_path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        let items_count: usize = conn.query_row("SELECT COUNT(*) FROM items", [], |r| r.get(0)).unwrap_or(0);
        let parties_count: usize = conn.query_row("SELECT COUNT(*) FROM parties", [], |r| r.get(0)).unwrap_or(0);
        let sales_count: usize = conn.query_row("SELECT COUNT(*) FROM documents WHERE documentType = 'sale_invoice'", [], |r| r.get(0))
            .or_else(|_| conn.query_row("SELECT COUNT(*) FROM documents", [], |r| r.get(0)))
            .unwrap_or(0);
        let purchases_count: usize = conn.query_row("SELECT COUNT(*) FROM documents WHERE documentType = 'purchase_invoice'", [], |r| r.get(0)).unwrap_or(0);
        let document_items_count: usize = conn.query_row("SELECT COUNT(*) FROM document_items", [], |r| r.get(0)).unwrap_or(0);

        drop(conn);
        let _ = std::fs::remove_file(&temp_db_path);

        Ok(MigrationPreview {
            company_name: manifest.company_name,
            schema_version,
            created_at: manifest.created_at,
            backup_file_path: archive_path.to_string(),
            backup_file_size_bytes,
            table_stats: TableStats {
                items_count,
                parties_count,
                sales_count,
                purchases_count,
                document_items_count,
            },
        })
    }

    fn execute_migration(&self, archive_path: &str) -> Result<MigrationResult, AppError> {
        let start = std::time::Instant::now();
        let file = File::open(archive_path).map_err(|e| AppError::IoError(e.to_string()))?;
        let mut archive = zip::ZipArchive::new(file)
            .map_err(|_| AppError::MigrationError("Invalid ZIP header".to_string()))?;

        let mut db_entry = archive
            .by_name("company.db")
            .map_err(|_| AppError::MigrationError("company.db not found".to_string()))?;

        let temp_db_path = std::env::temp_dir().join(format!("vedbill_exec_{}.db", uuid::Uuid::new_v4()));
        {
            let mut out_file = File::create(&temp_db_path).map_err(|e| AppError::IoError(e.to_string()))?;
            std::io::copy(&mut db_entry, &mut out_file).map_err(|e| AppError::IoError(e.to_string()))?;
        }

        let conn = rusqlite::Connection::open_with_flags(&temp_db_path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        let mut tx = self.uow.begin_transaction()?;

        let mut stmt = conn.prepare("SELECT name, salePrice, mrp, primaryQuantity, COALESCE(purchasePrice, 0) FROM items")
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;
        let item_rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, f64>(1)?,
                row.get::<_, Option<f64>>(2)?,
                row.get::<_, f64>(3)?,
                row.get::<_, f64>(4)?,
            ))
        }).map_err(|e| AppError::DatabaseError(e.to_string()))?;

        let now = chrono::Utc::now();
        // Opening stock lots hang off one synthetic, fully-paid purchase header (migration spec §Opening Stock).
        let opening_tx_id = tx.purchases().create_transaction(&PurchaseTransaction {
            id: 0,
            transaction_code: format!("PUR-OPEN-{}", now.format("%Y%m%d%H%M%S")),
            transaction_date: now.format("%Y-%m-%d").to_string(),
            party_id: None,
            payment_mode: None,
            payment_status: PaymentStatus::Paid,
            total_amount: 0.0,
            paid_amount: 0.0,
            balance_due: 0.0,
            notes: Some("Opening stock migrated from VedBill".to_string()),
            created_at: now.to_rfc3339(),
        })?;

        let mut items_imported = 0;
        let mut created_items = Vec::new();
        for r in item_rows {
            let (name, sale_price, mrp, stock, purchase_price) = r.map_err(|e| AppError::DatabaseError(e.to_string()))?;
            let mut item = Item {
                id: 0,
                name,
                category: Some("General".to_string()),
                barcode: None,
                base_unit: "PCS".to_string(),
                mrp,
                default_sale_price: sale_price,
                current_stock: stock,
                is_low_stock: stock < 5.0,
                is_active: true,
                created_at: chrono::Utc::now().to_rfc3339(),
                updated_at: chrono::Utc::now().to_rfc3339(),
            };
            item.id = tx.items().create(&item)?;
            if stock > 0.0 {
                tx.purchases().create_purchase_item(&PurchaseItem {
                    id: 0,
                    purchase_transaction_id: opening_tx_id,
                    item_id: item.id,
                    bundle_id: None,
                    bundle_quantity: stock,
                    quantity: stock,
                    unit_cost: purchase_price,
                    total_amount: stock * purchase_price,
                    remaining_quantity: stock,
                })?;
            }
            created_items.push(item);
            items_imported += 1;
        }

        let mut party_stmt = conn.prepare("SELECT name, phone, type FROM parties")
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;
        let party_rows = party_stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, String>(2)?,
            ))
        }).map_err(|e| AppError::DatabaseError(e.to_string()))?;

        let mut parties_imported = 0;
        for r in party_rows {
            let (name, phone, p_type) = r.map_err(|e| AppError::DatabaseError(e.to_string()))?;
            let party_type = if p_type == "supplier" { PartyType::Vendor } else { PartyType::Customer };
            let party = Party {
                id: 0,
                name,
                phone,
                party_type,
                notes: Some("Migrated from VedBill".to_string()),
                is_active: true,
                created_at: chrono::Utc::now().to_rfc3339(),
                updated_at: chrono::Utc::now().to_rfc3339(),
            };
            tx.parties().create(&party)?;
            parties_imported += 1;
        }

        tx.commit()?;
        drop(stmt);
        drop(party_stmt);
        drop(conn);
        let _ = std::fs::remove_file(&temp_db_path);

        if let Some(ref indexer) = self.search_indexer {
            for item in created_items {
                let _ = indexer.index_item(&item);
            }
        }

        Ok(MigrationResult {
            success: true,
            items_imported,
            parties_imported,
            sales_imported: 0,
            purchases_imported: 0,
            duration_ms: start.elapsed().as_millis() as u64,
            errors: Vec::new(),
        })
    }
}
