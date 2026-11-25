//! VedBill backup -> Book Keeper inventory (item master + opening stock lots).
//!
//! Mapping rules (confirmed with the shop owner):
//! - Every VedBill item becomes an item; a service becomes a SERVICE item (no stock, no components).
//! - Stock = the VedBill lots, copied one to one: each lot with quantity > 0 becomes an OPENING
//!   lot with the same quantity, cost and date (oldest first, so FIFO order is kept).
//! - Lots with quantity <= 0 (oversold) are dropped.
//! - No positive lots but VedBill shows stock > 0: one lot of that quantity at the item's purchase price.
//! - Negative stock on screen with no positive lots: imported with stock 0.
//! - Names are cleaned with `clean_name`; anything changed is reported as a warning.

use std::collections::HashMap;
use std::sync::Arc;
use crate::core::dtos::{ConflictStrategy, ImportRowError, ImportRowWarning, ImportSummary, ImportValidationReport};
use crate::core::error::AppError;
use crate::core::value_objects::{clean_name, ItemType};
use crate::data::contracts::search_indexer::ItemSearchIndexer;
use crate::data::contracts::unit_of_work::UnitOfWork;
use crate::data::contracts::vedbill_reader::{VedbillBackupReader, VedbillLot, VedbillSnapshot};
use crate::data::vedbill::SUPPORTED_SCHEMA_VERSION;
use crate::services::contracts::inventory_service::InventoryService;
use crate::services::contracts::vedbill_import_service::VedbillImportService;
use super::item_import_service_impl::{
    build_report, check_against_db, drop_in_file_duplicates, execute_checked, today, ImportRow, OpeningLot,
};

#[derive(Clone)]
pub struct VedbillImportServiceImpl<
    U: UnitOfWork = Arc<dyn UnitOfWork>,
    I: InventoryService = Arc<dyn InventoryService>,
    R: VedbillBackupReader = Arc<dyn VedbillBackupReader>,
> {
    pub uow: U,
    pub inventory_service: I,
    pub reader: R,
    pub search_indexer: Option<Arc<dyn ItemSearchIndexer>>,
}

impl<U: UnitOfWork, I: InventoryService, R: VedbillBackupReader> VedbillImportServiceImpl<U, I, R> {
    pub fn new(uow: U, inventory_service: I, reader: R) -> Self {
        Self { uow, inventory_service, reader, search_indexer: None }
    }

    pub fn with_indexer(mut self, indexer: Arc<dyn ItemSearchIndexer>) -> Self {
        self.search_indexer = Some(indexer);
        self
    }
}

const FIELD_NAME: &str = "Item Name";

fn qty(v: f64) -> String {
    if v.fract() == 0.0 {
        format!("{}", v as i64)
    } else {
        format!("{v}")
    }
}

/// VedBill dates are `YYYY-MM-DD` or ISO-8601 UTC timestamps; returns the local calendar date.
pub(crate) fn normalize_date(raw: &str) -> Option<String> {
    let raw = raw.trim();
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(raw) {
        return Some(dt.with_timezone(&chrono::Local).format("%Y-%m-%d").to_string());
    }
    let head = raw.get(..10)?;
    chrono::NaiveDate::parse_from_str(head, "%Y-%m-%d").ok().map(|d| d.format("%Y-%m-%d").to_string())
}

/// Maps a backup to import rows plus row errors and warnings (pure; no database access).
pub(crate) fn map_snapshot(snap: &VedbillSnapshot) -> (Vec<ImportRow>, Vec<ImportRowError>, Vec<ImportRowWarning>) {
    let mut errors = Vec::new();
    let mut warnings = Vec::new();
    let today = today();

    if let Some(v) = snap.manifest.schema_version {
        if v != SUPPORTED_SCHEMA_VERSION {
            warnings.push(ImportRowWarning {
                row: 0,
                name: String::new(),
                message: format!(
                    "This backup uses VedBill schema version {v}; the importer was built for version {SUPPORTED_SCHEMA_VERSION}. Check the report carefully."
                ),
            });
        }
    }

    let mut lots_by_item: HashMap<&str, Vec<&VedbillLot>> = HashMap::new();
    for l in &snap.lots {
        lots_by_item.entry(l.item_id.as_str()).or_default().push(l);
    }

    let mut rows = Vec::new();
    for (idx, it) in snap.items.iter().enumerate() {
        let row = idx + 1;
        let name = clean_name(&it.name);
        let mut warn = |message: String| warnings.push(ImportRowWarning { row, name: name.clone(), message });

        if name.is_empty() {
            errors.push(ImportRowError {
                row,
                name: None,
                field: FIELD_NAME.to_string(),
                message: "This VedBill item has no name. Give it a name in VedBill and take a new backup.".into(),
            });
            continue;
        }
        if name != it.name {
            warn(format!("Name cleaned: \"{}\" becomes \"{name}\" (extra spaces removed).", it.name));
        }
        if !it.is_active {
            warn("Inactive in VedBill; imported as an active item.".into());
        }

        let base_unit = it.unit.as_deref().map(clean_name).filter(|u| !u.is_empty()).unwrap_or_else(|| {
            warn("No unit in VedBill; PCS is used.".into());
            "PCS".to_string()
        });
        let sale_price = if it.sale_price.is_finite() && it.sale_price >= 0.0 {
            it.sale_price
        } else {
            warn(format!("Sale price {} is invalid; 0 is used.", it.sale_price));
            0.0
        };
        let mrp = it.mrp.filter(|m| m.is_finite() && *m > 0.0);
        let item_cost = it.purchase_price.filter(|c| c.is_finite() && *c >= 0.0).unwrap_or(0.0);
        let shown = it.primary_quantity.filter(|q| q.is_finite()).unwrap_or(0.0);

        let mut lots = Vec::new();
        let is_service = it.item_type.as_deref().map(|t| t.eq_ignore_ascii_case("SERVICE")).unwrap_or(false);
        if is_service {
            warn("Service in VedBill; imported as a SERVICE item (no stock, no components).".into());
        } else {
            let source = lots_by_item.get(it.legacy_id.as_str()).cloned().unwrap_or_default();
            let negatives: Vec<f64> = source.iter().map(|l| l.remaining_qty).filter(|q| *q < 0.0).collect();
            if !negatives.is_empty() {
                warn(format!(
                    "{} oversold lot(s) in VedBill (total {}) were not carried over.",
                    negatives.len(),
                    qty(negatives.iter().sum())
                ));
            }
            let mut zero_cost = 0;
            for l in source.iter().filter(|l| l.remaining_qty.is_finite() && l.remaining_qty > 0.0) {
                let date = normalize_date(&l.lot_date).unwrap_or_else(|| {
                    warn(format!("Lot date \"{}\" could not be read; today's date is used.", l.lot_date));
                    today.clone()
                });
                let cost = if l.purchase_price.is_finite() && l.purchase_price >= 0.0 { l.purchase_price } else { 0.0 };
                if cost == 0.0 {
                    zero_cost += 1;
                }
                lots.push(OpeningLot { date, qty: l.remaining_qty, cost });
            }
            if zero_cost > 0 {
                warn(format!("{zero_cost} lot(s) have a cost of 0. Profit on these units will look too high; fix the cost later if needed."));
            }

            if !lots.is_empty() {
                let total: f64 = lots.iter().map(|l| l.qty).sum();
                if (total - shown).abs() > 1e-6 {
                    warn(format!(
                        "VedBill shows stock {} but its lots add up to {}. Stock is imported as {} (from the lots).",
                        qty(shown),
                        qty(total),
                        qty(total)
                    ));
                }
            } else if shown > 0.0 {
                let date = it.opening_stock_date.as_deref().and_then(normalize_date).unwrap_or_else(|| today.clone());
                lots.push(OpeningLot { date, qty: shown, cost: item_cost });
                warn(format!(
                    "No stock lots in VedBill, so one opening lot of {} is created from the stock VedBill shows, at cost {}.{}",
                    qty(shown),
                    item_cost,
                    if item_cost == 0.0 { " Fix the cost later if needed." } else { "" }
                ));
            } else if shown < 0.0 {
                warn(format!("VedBill shows stock {} (oversold). Imported with stock 0.", qty(shown)));
            }
        }

        rows.push(ImportRow {
            row,
            name,
            category: None,
            barcode: None,
            base_unit,
            mrp,
            sale_price,
            lots,
            bundle: None,
            low_stock_threshold: None,
            item_type: Some(if is_service { ItemType::Service } else { ItemType::Physical }),
            service_cost: None,
        });
    }

    let rows = drop_in_file_duplicates(rows, &mut errors);
    (rows, errors, warnings)
}

impl<U: UnitOfWork, I: InventoryService, R: VedbillBackupReader> VedbillImportService for VedbillImportServiceImpl<U, I, R> {
    fn validate_vedbill_import(&self, file_path: &str) -> Result<ImportValidationReport, AppError> {
        let snap = self.reader.read(file_path)?;
        let (rows, errors, warnings) = map_snapshot(&snap);
        let mut tx = self.uow.begin_read()?;
        let checked = check_against_db(&mut *tx, rows, errors)?;
        Ok(build_report(checked, warnings))
    }

    fn execute_vedbill_import(&self, file_path: &str, strategy: ConflictStrategy) -> Result<ImportSummary, AppError> {
        let snap = self.reader.read(file_path)?;
        let (rows, errors, _warnings) = map_snapshot(&snap);
        let mut tx = self.uow.begin_transaction()?;
        let checked = check_against_db(&mut *tx, rows, errors)?;
        execute_checked(
            &self.uow,
            &self.inventory_service,
            self.search_indexer.as_ref(),
            tx,
            checked,
            strategy,
            "VedBill migration opening stock",
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::contracts::vedbill_reader::{VedbillItem, VedbillManifest};

    fn item(id: &str, name: &str, qty: f64, cost: f64) -> VedbillItem {
        VedbillItem {
            legacy_id: id.into(),
            name: name.into(),
            item_type: Some("PRODUCT".into()),
            unit: Some("PCS".into()),
            sale_price: 10.0,
            purchase_price: Some(cost),
            mrp: Some(0.0),
            primary_quantity: Some(qty),
            is_active: true,
            opening_stock_date: None,
        }
    }

    fn lot(id: &str, date: &str, cost: f64, q: f64) -> VedbillLot {
        VedbillLot { item_id: id.into(), lot_date: date.into(), purchase_price: cost, remaining_qty: q }
    }

    #[test]
    fn dates() {
        assert_eq!(normalize_date("2026-10-28").as_deref(), Some("2026-10-28"));
        assert!(normalize_date("2026-11-01T00:00:00.000Z").is_some());
        assert_eq!(normalize_date("garbage"), None);
    }

    #[test]
    fn maps_lots_and_edge_cases() {
        let mut svc = item("s", "SOFTWEAR", 0.0, 0.0);
        svc.item_type = Some("SERVICE".into());
        let snap = VedbillSnapshot {
            manifest: VedbillManifest { company_name: None, schema_version: Some(8) },
            items: vec![
                item("g", "WHITE GLUE 20 ", 29.0, 13.0),
                item("d", "DOMS TULI NO 20/-", 14.0, 0.0),
                item("a", "LOOSE A4 SHEET", -106.0, 0.37),
                svc,
                item("z", "ZERO", 0.0, 1.0),
            ],
            lots: vec![
                lot("g", "2026-11-01", 13.5, 2.0),
                lot("g", "2026-11-12", 13.0, 30.0),
                lot("d", "2026-11-21", 0.0, -1.0),
                lot("a", "2026-11-19", 0.37, -14.0),
                lot("a", "2026-11-19", 0.37, 0.0),
            ],
        };
        let (rows, errors, warnings) = map_snapshot(&snap);
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(rows.len(), 5);

        let glue = &rows[0];
        assert_eq!(glue.name, "WHITE GLUE 20");
        assert_eq!(glue.mrp, None);
        assert_eq!(
            glue.lots,
            vec![
                OpeningLot { date: "2026-11-01".into(), qty: 2.0, cost: 13.5 },
                OpeningLot { date: "2026-11-12".into(), qty: 30.0, cost: 13.0 },
            ]
        );
        assert_eq!(rows[1].lots, vec![OpeningLot { date: today(), qty: 14.0, cost: 0.0 }]);
        assert!(rows[2].lots.is_empty() && rows[3].lots.is_empty() && rows[4].lots.is_empty());

        let w = |row: usize, needle: &str| warnings.iter().any(|x| x.row == row && x.message.contains(needle));
        assert!(w(1, "Name cleaned") && w(1, "lots add up to 32"));
        assert!(w(2, "No stock lots") && w(2, "oversold lot"));
        assert!(w(3, "Imported with stock 0"));
        assert!(w(4, "Service"));
        assert!(!warnings.iter().any(|x| x.row == 5));
    }

    #[test]
    fn duplicate_names_after_cleaning_are_errors() {
        let snap = VedbillSnapshot {
            manifest: VedbillManifest::default(),
            items: vec![item("1", "Pen  Blue", 0.0, 0.0), item("2", " pen blue", 0.0, 0.0)],
            lots: vec![],
        };
        let (rows, errors, _) = map_snapshot(&snap);
        assert_eq!(rows.len(), 1);
        assert_eq!(errors[0].row, 2);
    }
}
