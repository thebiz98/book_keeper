use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use calamine::{open_workbook_auto, Data, Reader};
use crate::core::dtos::{
    ConflictStrategy, ImportDuplicate, ImportRowError, ImportSummary, ImportValidationReport,
};
use crate::core::entities::{Item, ItemBundle};
use crate::core::error::AppError;
use crate::core::value_objects::LotSource;
use crate::data::contracts::search_indexer::ItemSearchIndexer;
use crate::data::contracts::unit_of_work::{TransactionContext, UnitOfWork};
use crate::services::contracts::inventory_service::InventoryService;
use crate::services::contracts::item_import_service::ItemImportService;

/// Template A items import (LLD 6.14). All opening stock goes through
/// `InventoryService::create_source_lots` (sole stock writer) under one `OPN-` header.
#[derive(Clone)]
pub struct ItemImportServiceImpl<U: UnitOfWork = Arc<dyn UnitOfWork>, I: InventoryService = Arc<dyn InventoryService>> {
    pub uow: U,
    pub inventory_service: I,
    pub search_indexer: Option<Arc<dyn ItemSearchIndexer>>,
}

impl<U: UnitOfWork, I: InventoryService> ItemImportServiceImpl<U, I> {
    pub fn new(uow: U, inventory_service: I) -> Self {
        Self { uow, inventory_service, search_indexer: None }
    }

    /// Enables FTS reindexing of touched items after a successful import.
    pub fn with_indexer(mut self, indexer: Arc<dyn ItemSearchIndexer>) -> Self {
        self.search_indexer = Some(indexer);
        self
    }
}

const H_NAME: &str = "item name";
const H_CATEGORY: &str = "category";
const H_BARCODE: &str = "barcode";
const H_UNIT: &str = "base unit";
const H_MRP: &str = "mrp";
const H_PRICE: &str = "sale price";
const H_COST: &str = "purchase cost";
const H_STOCK: &str = "opening stock";
const H_BUNDLE: &str = "packaging bundle";
const H_UNITS: &str = "units per bundle";

const KNOWN: [(&str, &str); 10] = [
    (H_NAME, "Item Name"),
    (H_CATEGORY, "Category"),
    (H_BARCODE, "Barcode"),
    (H_UNIT, "Base Unit"),
    (H_MRP, "MRP"),
    (H_PRICE, "Sale Price"),
    (H_COST, "Purchase Cost"),
    (H_STOCK, "Opening Stock"),
    (H_BUNDLE, "Packaging Bundle"),
    (H_UNITS, "Units Per Bundle"),
];
const REQUIRED: [&str; 4] = [H_NAME, H_UNIT, H_PRICE, H_COST];

fn display(key: &str) -> &'static str {
    KNOWN.iter().find(|(k, _)| *k == key).map(|(_, d)| *d).unwrap_or("")
}

/// One non-blank sheet row: real sheet row number (header = 1) and trimmed cell texts by header key.
struct RawRow {
    row: usize,
    cells: HashMap<&'static str, String>,
}

#[derive(Debug, Clone)]
struct ImportRow {
    row: usize,
    name: String,
    category: Option<String>,
    barcode: Option<String>,
    base_unit: String,
    mrp: Option<f64>,
    sale_price: f64,
    cost: f64,
    opening: f64,
    bundle: Option<(String, f64)>,
}

fn vfail(field: &str, message: impl Into<String>) -> AppError {
    AppError::ValidationError { field: field.to_string(), message: message.into() }
}

fn cell_text(cell: &Data) -> String {
    match cell {
        Data::String(s) => s.trim().to_string(),
        Data::Int(i) => i.to_string(),
        Data::Float(f) => {
            if f.is_finite() && f.fract() == 0.0 && f.abs() < 1e15 {
                format!("{}", *f as i64)
            } else {
                f.to_string()
            }
        }
        Data::Bool(b) => b.to_string(),
        Data::Empty => String::new(),
        other => other.to_string().trim().to_string(),
    }
}

/// Reads `.csv` / `.xlsx` into raw string grids: `(sheet_row_number, cells)`.
fn read_grid(path: &str) -> Result<Vec<(usize, Vec<String>)>, AppError> {
    let lower = path.to_lowercase();
    if lower.ends_with(".csv") {
        let mut rdr = csv::ReaderBuilder::new()
            .has_headers(false)
            .flexible(true)
            .from_path(path)
            .map_err(|e| AppError::IoError(format!("Failed to open CSV: {e}")))?;
        let mut out = Vec::new();
        for (i, rec) in rdr.records().enumerate() {
            let rec = rec.map_err(|e| vfail("file", format!("Malformed CSV near row {}: {e}", i + 1)))?;
            let mut cells: Vec<String> = rec.iter().map(|c| c.trim().to_string()).collect();
            if i == 0 {
                if let Some(first) = cells.first_mut() {
                    *first = first.trim_start_matches('\u{feff}').trim().to_string();
                }
            }
            out.push((i + 1, cells));
        }
        Ok(out)
    } else if lower.ends_with(".xlsx") {
        let mut wb = open_workbook_auto(path)
            .map_err(|e| AppError::IoError(format!("Failed to open workbook: {e}")))?;
        let range = wb
            .worksheet_range_at(0)
            .ok_or_else(|| vfail("file", "No sheets found in workbook"))?
            .map_err(|e| AppError::IoError(format!("Corrupt worksheet: {e}")))?;
        let (row0, col0) = range.start().map(|(r, c)| (r as usize, c as usize)).unwrap_or((0, 0));
        let mut out = Vec::new();
        for (i, r) in range.rows().enumerate() {
            let mut cells = vec![String::new(); col0];
            cells.extend(r.iter().map(cell_text));
            out.push((row0 + i + 1, cells));
        }
        Ok(out)
    } else {
        Err(vfail("file", "Unsupported file type: use a .csv or .xlsx file"))
    }
}

/// Parses the file into rows keyed by canonical header; header problems are hard errors.
fn parse_raw(path: &str) -> Result<Vec<RawRow>, AppError> {
    let grid = read_grid(path)?;
    let mut iter = grid.into_iter().skip_while(|(_, c)| c.iter().all(|x| x.is_empty()));
    let (_, header) = iter.next().ok_or_else(|| vfail("header", "The file is empty"))?;

    let mut cols: Vec<(usize, &'static str)> = Vec::new();
    let mut unknown = Vec::new();
    let mut seen = HashSet::new();
    for (idx, h) in header.iter().enumerate() {
        let norm = h.trim().to_lowercase();
        if norm.is_empty() {
            continue;
        }
        match KNOWN.iter().find(|(k, _)| *k == norm) {
            Some((k, _)) => {
                if !seen.insert(*k) {
                    return Err(vfail("header", format!("Duplicate column '{}'", display(k))));
                }
                cols.push((idx, *k));
            }
            None => unknown.push(h.trim().to_string()),
        }
    }
    if !unknown.is_empty() {
        return Err(vfail("header", format!("Unknown column(s): {}", unknown.join(", "))));
    }
    let missing: Vec<&str> = REQUIRED.iter().filter(|k| !seen.contains(*k)).map(|k| display(k)).collect();
    if !missing.is_empty() {
        return Err(vfail("header", format!("Missing required column(s): {}", missing.join(", "))));
    }

    let mut rows = Vec::new();
    for (row, cells) in iter {
        if cells.iter().all(|c| c.is_empty()) {
            continue;
        }
        let mut map = HashMap::new();
        for (idx, key) in &cols {
            map.insert(*key, cells.get(*idx).cloned().unwrap_or_default());
        }
        rows.push(RawRow { row, cells: map });
    }
    Ok(rows)
}

fn parse_number(
    raw: &RawRow,
    key: &'static str,
    required: bool,
    errors: &mut Vec<ImportRowError>,
) -> Option<f64> {
    let text = raw.cells.get(key).map(String::as_str).unwrap_or("");
    let field = display(key);
    let err = |errors: &mut Vec<ImportRowError>, message: String| {
        errors.push(ImportRowError { row: raw.row, field: field.to_string(), message });
        None
    };
    if text.is_empty() {
        if required {
            return err(errors, format!("{field} is required"));
        }
        return None;
    }
    match text.parse::<f64>() {
        Ok(v) if v.is_finite() && v >= 0.0 => Some(v),
        Ok(_) => err(errors, format!("{field} must be a finite number >= 0 (got '{text}')")),
        Err(_) => err(errors, format!("{field} is not a number ('{text}')")),
    }
}

/// Pure per-row checks plus within-file duplicates. Returns the rows that parsed cleanly.
fn check_rows(raws: &[RawRow], errors: &mut Vec<ImportRowError>) -> Vec<ImportRow> {
    let mut out = Vec::new();
    let mut names: HashMap<String, usize> = HashMap::new();
    let mut barcodes: HashMap<String, usize> = HashMap::new();
    for raw in raws {
        let before = errors.len();
        let text = |k: &str| raw.cells.get(k).map(String::as_str).unwrap_or("").to_string();
        let push = |errors: &mut Vec<ImportRowError>, key: &str, message: String| {
            errors.push(ImportRowError { row: raw.row, field: display(key).to_string(), message });
        };

        let name = text(H_NAME);
        if name.is_empty() {
            push(errors, H_NAME, "Item Name is required".into());
        } else if let Some(first) = names.insert(name.to_lowercase(), raw.row) {
            push(errors, H_NAME, format!("Duplicate item name '{name}' (first seen in row {first})"));
        }
        let base_unit = text(H_UNIT);
        if base_unit.is_empty() {
            push(errors, H_UNIT, "Base Unit is required".into());
        }
        let sale_price = parse_number(raw, H_PRICE, true, errors);
        let cost = parse_number(raw, H_COST, true, errors);
        let mrp = parse_number(raw, H_MRP, false, errors);
        if mrp == Some(0.0) {
            push(errors, H_MRP, "MRP must be greater than 0 when given".into());
        }
        let opening = parse_number(raw, H_STOCK, false, errors);

        let barcode = Some(text(H_BARCODE)).filter(|b| !b.is_empty());
        if let Some(b) = &barcode {
            if let Some(first) = barcodes.insert(b.clone(), raw.row) {
                push(errors, H_BARCODE, format!("Barcode '{b}' is repeated in the file (first seen in row {first})"));
            }
        }

        let bname = text(H_BUNDLE);
        let units_text = text(H_UNITS);
        let mut bundle = None;
        match (bname.is_empty(), units_text.is_empty()) {
            (true, true) => {}
            (false, true) => push(errors, H_UNITS, "Units Per Bundle is required when Packaging Bundle is given".into()),
            (true, false) => push(errors, H_BUNDLE, "Packaging Bundle name is required when Units Per Bundle is given".into()),
            (false, false) => match units_text.parse::<f64>() {
                Ok(u) if u.is_finite() && u > 1.0 => bundle = Some((bname, u)),
                _ => push(errors, H_UNITS, format!("Units Per Bundle must be a number greater than 1 (got '{units_text}')")),
            },
        }

        if errors.len() == before {
            out.push(ImportRow {
                row: raw.row,
                name,
                category: Some(text(H_CATEGORY)).filter(|c| !c.is_empty()),
                barcode,
                base_unit,
                mrp,
                sale_price: sale_price.unwrap_or(0.0),
                cost: cost.unwrap_or(0.0),
                opening: opening.unwrap_or(0.0),
                bundle,
            });
        }
    }
    out
}

struct Checked {
    /// Rows without errors, each with the existing item it matches by name (if any).
    rows: Vec<(ImportRow, Option<Item>)>,
    errors: Vec<ImportRowError>,
    duplicates: Vec<ImportDuplicate>,
}

/// File checks plus DB checks (duplicates by name, barcode uniqueness vs items and bundles).
fn check_against_db(tx: &mut dyn TransactionContext, raws: &[RawRow]) -> Result<Checked, AppError> {
    let mut errors = Vec::new();
    let parsed = check_rows(raws, &mut errors);
    let mut rows = Vec::new();
    let mut duplicates = Vec::new();
    for row in parsed {
        let existing = tx.items().find_by_name(&row.name)?;
        if let Some(e) = &existing {
            duplicates.push(ImportDuplicate { row: row.row, name: row.name.clone(), existing_item_id: e.id });
        }
        let mut clash = false;
        if let Some(bc) = &row.barcode {
            let own_id = existing.as_ref().map(|e| e.id);
            let item_hit = tx.items().find_by_barcode(bc)?.filter(|i| Some(i.id) != own_id);
            let bundle_hit = tx.bundles().find_by_barcode(bc)?;
            if item_hit.is_some() || bundle_hit.is_some() {
                errors.push(ImportRowError {
                    row: row.row,
                    field: display(H_BARCODE).to_string(),
                    message: format!("Barcode '{bc}' already exists in the catalog"),
                });
                clash = true;
            }
        }
        if !clash {
            rows.push((row, existing));
        }
    }
    errors.sort_by_key(|e| e.row);
    Ok(Checked { rows, errors, duplicates })
}

impl<U: UnitOfWork, I: InventoryService> ItemImportService for ItemImportServiceImpl<U, I> {
    fn validate_items_import(&self, file_path: &str) -> Result<ImportValidationReport, AppError> {
        let raws = parse_raw(file_path)?;
        let mut tx = self.uow.begin_read()?;
        let checked = check_against_db(&mut *tx, &raws)?;
        Ok(ImportValidationReport {
            valid_count: checked.rows.len(),
            rows_with_errors: checked.errors,
            duplicates: checked.duplicates,
        })
    }

    fn execute_items_import(
        &self,
        file_path: &str,
        strategy: ConflictStrategy,
    ) -> Result<ImportSummary, AppError> {
        let raws = parse_raw(file_path)?;
        let mut tx = self.uow.begin_transaction()?;
        let checked = check_against_db(&mut *tx, &raws)?;
        if let Some(first) = checked.errors.first() {
            return Err(vfail(
                "import",
                format!(
                    "{} row(s) have errors, nothing was imported. First: row {} ({}): {}",
                    checked.errors.len(), first.row, first.field, first.message
                ),
            ));
        }

        let now = chrono::Utc::now().to_rfc3339();
        let mut summary = ImportSummary { created: 0, updated: 0, skipped: 0, stock_lots_created: 0 };
        let mut lots: Vec<(i64, f64, Option<f64>)> = Vec::new();
        let mut touched: Vec<i64> = Vec::new();

        for (row, existing) in checked.rows {
            match existing {
                None => {
                    let item = Item {
                        id: 0,
                        name: row.name.clone(),
                        category: row.category.clone(),
                        barcode: row.barcode.clone(),
                        base_unit: row.base_unit.clone(),
                        mrp: row.mrp,
                        default_sale_price: row.sale_price,
                        // The cache starts at 0; the opening lot raises it.
                        current_stock: 0.0,
                        is_low_stock: true,
                        is_active: true,
                        created_at: now.clone(),
                        updated_at: now.clone(),
                    };
                    let item_id = tx.items().create(&item)?;
                    if let Some((bundle_name, units)) = &row.bundle {
                        tx.bundles().create(&ItemBundle {
                            id: 0,
                            item_id,
                            bundle_name: bundle_name.clone(),
                            units_per_bundle: *units,
                            mrp: None,
                            default_bundle_price: None,
                            barcode: None,
                        })?;
                    }
                    if row.opening > 0.0 {
                        lots.push((item_id, row.opening, Some(row.cost)));
                    }
                    summary.created += 1;
                    touched.push(item_id);
                }
                Some(mut existing) => match strategy {
                    ConflictStrategy::Skip => summary.skipped += 1,
                    ConflictStrategy::Overwrite => {
                        // Master fields only; blank file cells never clear existing values.
                        if row.category.is_some() {
                            existing.category = row.category.clone();
                        }
                        if row.barcode.is_some() {
                            existing.barcode = row.barcode.clone();
                        }
                        if row.mrp.is_some() {
                            existing.mrp = row.mrp;
                        }
                        existing.default_sale_price = row.sale_price;
                        existing.updated_at = now.clone();
                        tx.items().update(&existing)?;
                        summary.updated += 1;
                        touched.push(existing.id);
                    }
                    ConflictStrategy::AddStock => {
                        if row.opening > 0.0 {
                            lots.push((existing.id, row.opening, Some(row.cost)));
                            summary.updated += 1;
                        } else {
                            summary.skipped += 1;
                        }
                    }
                },
            }
        }

        if !lots.is_empty() {
            let today = chrono::Local::now().format("%Y-%m-%d").to_string();
            let ids = self.inventory_service.create_source_lots(
                &mut *tx,
                LotSource::Opening,
                &today,
                Some("Items import opening stock".to_string()),
                &lots,
            )?;
            summary.stock_lots_created = ids.len();
        }
        tx.commit()?;

        if let Some(indexer) = &self.search_indexer {
            if let Ok(mut r) = self.uow.begin_read() {
                for id in touched {
                    let item = r.items().find_by_id(id).ok().flatten();
                    let bundles = r.bundles().find_by_item_id(id).unwrap_or_default();
                    if let Some(item) = item {
                        if let Err(e) = indexer.index_item_with_bundles(&item, &bundles) {
                            eprintln!("[import] FTS reindex failed for item #{id}: {e}");
                        }
                    }
                }
            }
        }
        Ok(summary)
    }
}
