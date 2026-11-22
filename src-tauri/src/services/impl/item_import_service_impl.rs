use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use calamine::{open_workbook_auto, Data, Reader};
use crate::core::dtos::{
    ConflictStrategy, ExpectedColumn, HeaderIssue, HeaderIssueKind, ImportDuplicate, ImportRowError,
    ImportRowWarning, ImportSummary, ImportValidationReport,
};
use crate::core::entities::{Item, ItemBundle};
use crate::core::error::AppError;
use crate::core::value_objects::{clean_name, clean_optional, LotSource};
use crate::data::contracts::search_indexer::ItemSearchIndexer;
use crate::data::contracts::unit_of_work::{TransactionContext, UnitOfWork};
use crate::services::contracts::inventory_service::InventoryService;
use crate::services::contracts::item_import_service::ItemImportService;

/// Template A items import (LLD 6.14). All opening stock goes through
/// `InventoryService::create_source_lots` (sole stock writer) under `OPN-` headers.
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

// ---------------------------------------------------------------------------------------------
// Template columns (single source of truth for the validator and the downloadable templates)
// ---------------------------------------------------------------------------------------------

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

pub(crate) struct ColumnSpec {
    pub key: &'static str,
    pub display: &'static str,
    pub required: bool,
    pub numeric: bool,
    pub rule: &'static str,
    pub example: &'static str,
}

/// Template A, in file order.
pub(crate) const COLUMNS: [ColumnSpec; 10] = [
    ColumnSpec { key: H_NAME, display: "Item Name", required: true, numeric: false,
        rule: "Text. Must be unique in the file (case does not matter). Extra spaces are removed.", example: "Pen Blue" },
    ColumnSpec { key: H_CATEGORY, display: "Category", required: false, numeric: false,
        rule: "Text. Optional.", example: "Stationery" },
    ColumnSpec { key: H_BARCODE, display: "Barcode", required: false, numeric: false,
        rule: "Text. Optional. Must not repeat in the file or match an existing item.", example: "8901234567890" },
    ColumnSpec { key: H_UNIT, display: "Base Unit", required: true, numeric: false,
        rule: "Text, e.g. PCS, KG, BOX.", example: "PCS" },
    ColumnSpec { key: H_MRP, display: "MRP", required: false, numeric: true,
        rule: "Number greater than 0. Optional.", example: "12" },
    ColumnSpec { key: H_PRICE, display: "Sale Price", required: true, numeric: true,
        rule: "Number, 0 or more.", example: "10" },
    ColumnSpec { key: H_COST, display: "Purchase Cost", required: true, numeric: true,
        rule: "Number, 0 or more. Cost per unit of the opening stock.", example: "6.5" },
    ColumnSpec { key: H_STOCK, display: "Opening Stock", required: false, numeric: true,
        rule: "Number, 0 or more. More than 0 creates an opening stock lot dated today.", example: "100" },
    ColumnSpec { key: H_BUNDLE, display: "Packaging Bundle", required: false, numeric: false,
        rule: "Text, e.g. Box. Give it together with Units Per Bundle.", example: "Box" },
    ColumnSpec { key: H_UNITS, display: "Units Per Bundle", required: false, numeric: true,
        rule: "Number greater than 1. Give it together with Packaging Bundle.", example: "10" },
];

fn spec(key: &str) -> &'static ColumnSpec {
    COLUMNS.iter().find(|c| c.key == key).expect("known column key")
}

fn display(key: &str) -> &'static str {
    spec(key).display
}

pub(crate) fn expected_columns() -> Vec<ExpectedColumn> {
    COLUMNS
        .iter()
        .map(|c| ExpectedColumn {
            name: c.display.to_string(),
            required: c.required,
            rule: c.rule.to_string(),
            example: c.example.to_string(),
        })
        .collect()
}

fn expected_list() -> String {
    COLUMNS
        .iter()
        .map(|c| if c.required { format!("{}*", c.display) } else { c.display.to_string() })
        .collect::<Vec<_>>()
        .join(", ")
}

// ---------------------------------------------------------------------------------------------
// Shared row model (CSV/XLSX and VedBill both produce these)
// ---------------------------------------------------------------------------------------------

/// One opening stock lot to create for a row.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct OpeningLot {
    /// `YYYY-MM-DD`
    pub date: String,
    pub qty: f64,
    pub cost: f64,
}

#[derive(Debug, Clone)]
pub(crate) struct ImportRow {
    pub row: usize,
    pub name: String,
    pub category: Option<String>,
    pub barcode: Option<String>,
    pub base_unit: String,
    pub mrp: Option<f64>,
    pub sale_price: f64,
    /// In FIFO order (oldest first).
    pub lots: Vec<OpeningLot>,
    pub bundle: Option<(String, f64)>,
}

pub(crate) fn vfail(field: &str, message: impl Into<String>) -> AppError {
    AppError::ValidationError { field: field.to_string(), message: message.into() }
}

pub(crate) fn today() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
}

// ---------------------------------------------------------------------------------------------
// File reading
// ---------------------------------------------------------------------------------------------

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

/// Decodes CSV bytes: UTF-8 (BOM optional), else Windows-1252 (Excel "CSV" on Windows).
pub(crate) fn decode_csv_bytes(bytes: &[u8]) -> String {
    let bytes = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
    match std::str::from_utf8(bytes) {
        Ok(s) => s.to_string(),
        Err(_) => encoding_rs::WINDOWS_1252.decode(bytes).0.into_owned(),
    }
}

/// Reads `.csv` / `.xlsx` / `.xls` into raw string grids: `(sheet_row_number, cells)`.
fn read_grid(path: &str) -> Result<Vec<(usize, Vec<String>)>, AppError> {
    let lower = path.trim().to_lowercase();
    if lower.ends_with(".csv") {
        let bytes = std::fs::read(path).map_err(|e| AppError::IoError(format!("Could not open the CSV file: {e}")))?;
        let text = decode_csv_bytes(&bytes);
        let mut rdr = csv::ReaderBuilder::new().has_headers(false).flexible(true).from_reader(text.as_bytes());
        let mut out = Vec::new();
        for (i, rec) in rdr.records().enumerate() {
            let rec = rec.map_err(|e| {
                vfail("file", format!("The CSV is malformed near row {}: {e}. Check for an unclosed quote (\").", i + 1))
            })?;
            let mut cells: Vec<String> = rec.iter().map(|c| c.trim().to_string()).collect();
            if i == 0 {
                if let Some(first) = cells.first_mut() {
                    *first = first.trim_start_matches('\u{feff}').trim().to_string();
                }
            }
            out.push((i + 1, cells));
        }
        Ok(out)
    } else if lower.ends_with(".xlsx") || lower.ends_with(".xls") {
        let mut wb = open_workbook_auto(path).map_err(|e| {
            AppError::IoError(format!("Could not open the workbook: {e}. If it is open in Excel, close it and try again."))
        })?;
        let range = wb
            .worksheet_range_at(0)
            .ok_or_else(|| vfail("file", "The workbook has no sheets"))?
            .map_err(|e| AppError::IoError(format!("The first worksheet could not be read: {e}")))?;
        let (row0, col0) = range.start().map(|(r, c)| (r as usize, c as usize)).unwrap_or((0, 0));
        let mut out = Vec::new();
        for (i, r) in range.rows().enumerate() {
            let mut cells = vec![String::new(); col0];
            cells.extend(r.iter().map(cell_text));
            out.push((row0 + i + 1, cells));
        }
        Ok(out)
    } else {
        Err(vfail("file", "Unsupported file type. Save the sheet as .xlsx or .csv and pick that file."))
    }
}

// ---------------------------------------------------------------------------------------------
// Header validation with suggestions
// ---------------------------------------------------------------------------------------------

/// Spreadsheet column letter for a 1-based position (1 -> A, 27 -> AA).
pub(crate) fn column_letter(mut pos: usize) -> String {
    let mut s = Vec::new();
    while pos > 0 {
        let rem = (pos - 1) % 26;
        s.push((b'A' + rem as u8) as char);
        pos = (pos - 1) / 26;
    }
    s.iter().rev().collect()
}

fn norm_header(h: &str) -> String {
    clean_name(h).to_lowercase()
}

fn squash(h: &str) -> String {
    h.chars().filter(|c| c.is_alphanumeric()).flat_map(char::to_lowercase).collect()
}

fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for i in 1..=a.len() {
        let mut cur = vec![i; b.len() + 1];
        for j in 1..=b.len() {
            let cost = usize::from(a[i - 1] != b[j - 1]);
            cur[j] = (prev[j] + 1).min(cur[j - 1] + 1).min(prev[j - 1] + cost);
        }
        prev = cur;
    }
    prev[b.len()]
}

/// The template column a wrong header most likely meant.
pub(crate) fn suggest_column(header: &str) -> Option<&'static str> {
    let sq = squash(header);
    if sq.is_empty() {
        return None;
    }
    // 1. Spelling: closest template name, ignoring case, spaces and punctuation.
    let best = COLUMNS
        .iter()
        .map(|c| (levenshtein(&sq, &squash(c.display)), c))
        .min_by_key(|(d, _)| *d);
    if let Some((d, c)) = best {
        let limit = (squash(c.display).len() / 3).clamp(1, 3);
        if d <= limit {
            return Some(c.display);
        }
    }
    // 2. Meaning: common words used for the same thing in other software.
    let has = |w: &[&str]| w.iter().any(|x| sq.contains(x));
    let key = if has(&["perbundle", "perbox", "perpack", "conversion", "pcsper", "unitsper", "packsize"]) {
        H_UNITS
    } else if has(&["mrp", "maxretail", "maximumretail"]) {
        H_MRP
    } else if has(&["purchase", "cost", "buy"]) {
        H_COST
    } else if has(&["qty", "quantity", "stock", "opening", "balance"]) {
        H_STOCK
    } else if has(&["sale", "sell", "rate", "price"]) {
        H_PRICE
    } else if has(&["unit", "uom"]) {
        H_UNIT
    } else if has(&["bundle", "pack", "box", "carton"]) {
        H_BUNDLE
    } else if has(&["barcode", "ean", "upc", "sku", "code"]) {
        H_BARCODE
    } else if has(&["category", "group", "type", "department"]) {
        H_CATEGORY
    } else if has(&["name", "item", "product", "description", "particular"]) {
        H_NAME
    } else {
        return None;
    };
    Some(display(key))
}

/// Maps header cells to column keys, or lists every header problem at once.
fn parse_header(header: &[String]) -> Result<Vec<(usize, &'static str)>, Vec<HeaderIssue>> {
    let mut cols: Vec<(usize, &'static str)> = Vec::new();
    let mut first_pos: HashMap<&'static str, usize> = HashMap::new();
    let mut unknown: Vec<(usize, String)> = Vec::new();
    let mut issues = Vec::new();

    for (idx, h) in header.iter().enumerate() {
        let norm = norm_header(h);
        if norm.is_empty() {
            continue;
        }
        match COLUMNS.iter().find(|c| c.key == norm) {
            Some(c) => {
                if let Some(&first) = first_pos.get(c.key) {
                    issues.push(HeaderIssue {
                        kind: HeaderIssueKind::Duplicate,
                        column: clean_name(h),
                        position: Some(idx + 1),
                        column_letter: Some(column_letter(idx + 1)),
                        message: format!(
                            "Column \"{}\" appears twice (columns {} and {}). Keep one and delete the other.",
                            c.display,
                            column_letter(first + 1),
                            column_letter(idx + 1)
                        ),
                        suggestion: None,
                    });
                } else {
                    first_pos.insert(c.key, idx);
                    cols.push((idx, c.key));
                }
            }
            None => unknown.push((idx, clean_name(h))),
        }
    }

    for (idx, h) in unknown {
        let letter = column_letter(idx + 1);
        let suggestion = suggest_column(&h);
        let message = match suggestion {
            Some(s) => {
                let key = COLUMNS.iter().find(|c| c.display == s).map(|c| c.key).unwrap_or("");
                match first_pos.get(key) {
                    Some(&at) => format!(
                        "Column \"{h}\" (column {letter}) is not a template column. \"{s}\" is already in column {}, so delete column {letter}.",
                        column_letter(at + 1)
                    ),
                    None => format!("Column \"{h}\" (column {letter}) is not a template column. Rename it to \"{s}\"."),
                }
            }
            None => format!(
                "Column \"{h}\" (column {letter}) is not a template column. Delete it, or rename it to one of: {}.",
                expected_list()
            ),
        };
        issues.push(HeaderIssue {
            kind: HeaderIssueKind::Unknown,
            column: h,
            position: Some(idx + 1),
            column_letter: Some(letter),
            message,
            suggestion: suggestion.map(str::to_string),
        });
    }

    for c in COLUMNS.iter().filter(|c| c.required && !first_pos.contains_key(c.key)) {
        issues.push(HeaderIssue {
            kind: HeaderIssueKind::Missing,
            column: c.display.to_string(),
            position: None,
            column_letter: None,
            message: format!(
                "Required column \"{}\" is missing. Add a column with exactly this header in row 1.",
                c.display
            ),
            suggestion: Some(c.display.to_string()),
        });
    }

    if issues.is_empty() {
        Ok(cols)
    } else {
        Err(issues)
    }
}

/// One non-blank sheet row: real sheet row number (header = 1) and trimmed cell texts by header key.
struct RawRow {
    row: usize,
    cells: HashMap<&'static str, String>,
}

enum Parsed {
    Rows(Vec<RawRow>),
    HeaderIssues(Vec<HeaderIssue>),
}

fn parse_raw(path: &str) -> Result<Parsed, AppError> {
    let grid = read_grid(path)?;
    let mut iter = grid.into_iter().skip_while(|(_, c)| c.iter().all(|x| x.is_empty()));
    let Some((_, header)) = iter.next() else {
        return Err(vfail("header", format!("The file is empty. Row 1 must hold the column headers: {}", expected_list())));
    };
    let cols = match parse_header(&header) {
        Ok(c) => c,
        Err(issues) => return Ok(Parsed::HeaderIssues(issues)),
    };

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
    Ok(Parsed::Rows(rows))
}

// ---------------------------------------------------------------------------------------------
// Row checks
// ---------------------------------------------------------------------------------------------

fn number_hint(text: &str) -> &'static str {
    if text.contains(',') {
        " Use a dot for decimals and no thousands separator, e.g. 1250.50"
    } else if text.chars().any(|c| matches!(c, '₹' | '$' | '%')) || text.to_lowercase().starts_with("rs") {
        " Remove the currency or % sign and keep only the number."
    } else {
        " Use digits only, e.g. 12.5"
    }
}

fn parse_number(
    raw: &RawRow,
    name: Option<&str>,
    key: &'static str,
    required: bool,
    errors: &mut Vec<ImportRowError>,
) -> Option<f64> {
    let text = raw.cells.get(key).map(String::as_str).unwrap_or("");
    let field = display(key);
    let mut err = |message: String| {
        errors.push(ImportRowError { row: raw.row, name: name.map(str::to_string), field: field.to_string(), message });
        None
    };
    if text.is_empty() {
        if required {
            return err(format!("{field} is empty. Enter a number (0 is allowed)."));
        }
        return None;
    }
    match text.parse::<f64>() {
        Ok(v) if v.is_finite() && v >= 0.0 => Some(v),
        Ok(_) => err(format!("{field} \"{text}\" must be a number that is 0 or more.")),
        Err(_) => err(format!("{field} \"{text}\" is not a number.{}", number_hint(text))),
    }
}

/// Pure per-row checks for spreadsheet rows. Returns the rows that parsed cleanly.
fn check_rows(raws: &[RawRow], errors: &mut Vec<ImportRowError>) -> Vec<ImportRow> {
    let mut out = Vec::new();
    let date = today();
    for raw in raws {
        let before = errors.len();
        let text = |k: &str| raw.cells.get(k).map(String::as_str).unwrap_or("").to_string();

        let name = clean_name(&text(H_NAME));
        let nm = Some(name.as_str()).filter(|n| !n.is_empty());
        let push = |errors: &mut Vec<ImportRowError>, key: &str, message: String| {
            errors.push(ImportRowError {
                row: raw.row,
                name: nm.map(str::to_string),
                field: display(key).to_string(),
                message,
            });
        };

        if name.is_empty() {
            push(errors, H_NAME, "Item Name is empty. Every row needs a name (or delete the row).".into());
        }
        let base_unit = clean_name(&text(H_UNIT));
        if base_unit.is_empty() {
            push(errors, H_UNIT, "Base Unit is empty. Enter a unit such as PCS, KG or BOX.".into());
        }
        let sale_price = parse_number(raw, nm, H_PRICE, true, errors);
        let cost = parse_number(raw, nm, H_COST, true, errors);
        let mrp = parse_number(raw, nm, H_MRP, false, errors);
        if mrp == Some(0.0) {
            push(errors, H_MRP, "MRP is 0. Leave it empty for no MRP, or enter a number greater than 0.".into());
        }
        let opening = parse_number(raw, nm, H_STOCK, false, errors);

        let barcode = Some(text(H_BARCODE).trim().to_string()).filter(|b| !b.is_empty());

        let bname = clean_name(&text(H_BUNDLE));
        let units_text = text(H_UNITS);
        let mut bundle = None;
        match (bname.is_empty(), units_text.is_empty()) {
            (true, true) => {}
            (false, true) => push(errors, H_UNITS, format!("Packaging Bundle \"{bname}\" is given, so Units Per Bundle is needed too (how many units are in one {bname}).")),
            (true, false) => push(errors, H_BUNDLE, "Units Per Bundle is given, so Packaging Bundle is needed too (e.g. Box). Or clear both.".into()),
            (false, false) => match units_text.parse::<f64>() {
                Ok(u) if u.is_finite() && u > 1.0 => bundle = Some((bname, u)),
                _ => push(errors, H_UNITS, format!("Units Per Bundle \"{units_text}\" must be a number greater than 1.")),
            },
        }

        if errors.len() == before {
            let opening = opening.unwrap_or(0.0);
            let lots = if opening > 0.0 {
                vec![OpeningLot { date: date.clone(), qty: opening, cost: cost.unwrap_or(0.0) }]
            } else {
                Vec::new()
            };
            out.push(ImportRow {
                row: raw.row,
                name,
                category: clean_optional(Some(&text(H_CATEGORY))),
                barcode,
                base_unit,
                mrp,
                sale_price: sale_price.unwrap_or(0.0),
                lots,
                bundle,
            });
        }
    }
    out
}

/// Within-source duplicates (names case-insensitive after cleaning, barcodes exact).
pub(crate) fn drop_in_file_duplicates(rows: Vec<ImportRow>, errors: &mut Vec<ImportRowError>) -> Vec<ImportRow> {
    let mut names: HashMap<String, usize> = HashMap::new();
    let mut barcodes: HashMap<String, usize> = HashMap::new();
    let mut out = Vec::new();
    for r in rows {
        let mut bad = false;
        if let Some(first) = names.get(&r.name.to_lowercase()) {
            errors.push(ImportRowError {
                row: r.row,
                name: Some(r.name.clone()),
                field: display(H_NAME).to_string(),
                message: format!(
                    "Item name \"{}\" is repeated (first seen in row {first}). Names must be unique; rename one or merge the rows.",
                    r.name
                ),
            });
            bad = true;
        }
        if let Some(b) = &r.barcode {
            if let Some(first) = barcodes.get(b) {
                errors.push(ImportRowError {
                    row: r.row,
                    name: Some(r.name.clone()),
                    field: display(H_BARCODE).to_string(),
                    message: format!("Barcode \"{b}\" is repeated in the file (first seen in row {first}). Each barcode must be unique."),
                });
                bad = true;
            }
        }
        names.entry(r.name.to_lowercase()).or_insert(r.row);
        if let Some(b) = &r.barcode {
            barcodes.entry(b.clone()).or_insert(r.row);
        }
        if !bad {
            out.push(r);
        }
    }
    out
}

pub(crate) struct Checked {
    /// Rows without errors, each with the existing item it matches by name (if any).
    pub rows: Vec<(ImportRow, Option<Item>)>,
    pub errors: Vec<ImportRowError>,
    pub duplicates: Vec<ImportDuplicate>,
}

/// DB checks (duplicates by name, barcode uniqueness vs items and bundles).
pub(crate) fn check_against_db(
    tx: &mut dyn TransactionContext,
    parsed: Vec<ImportRow>,
    mut errors: Vec<ImportRowError>,
) -> Result<Checked, AppError> {
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
            if let Some(owner) = item_hit.map(|i| i.name).or(bundle_hit.map(|b| format!("a pack of item #{}", b.item_id))) {
                errors.push(ImportRowError {
                    row: row.row,
                    name: Some(row.name.clone()),
                    field: display(H_BARCODE).to_string(),
                    message: format!("Barcode \"{bc}\" already belongs to \"{owner}\". Clear it or use a different barcode."),
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

pub(crate) fn build_report(checked: Checked, mut warnings: Vec<ImportRowWarning>) -> ImportValidationReport {
    warnings.sort_by_key(|w| w.row);
    let lots_to_create = checked.rows.iter().filter(|(_, e)| e.is_none()).map(|(r, _)| r.lots.len()).sum();
    ImportValidationReport {
        valid_count: checked.rows.len(),
        rows_with_errors: checked.errors,
        duplicates: checked.duplicates,
        header_issues: Vec::new(),
        warnings,
        lots_to_create,
        expected_columns: expected_columns(),
    }
}

/// Imports already-checked rows inside `tx` and commits, then reindexes FTS.
/// Lots are grouped per date (one `OPN-` header per date, oldest first) so FIFO order is kept.
pub(crate) fn execute_checked<U: UnitOfWork, I: InventoryService>(
    uow: &U,
    inventory_service: &I,
    search_indexer: Option<&Arc<dyn ItemSearchIndexer>>,
    mut tx: Box<dyn TransactionContext>,
    checked: Checked,
    strategy: ConflictStrategy,
    lot_notes: &str,
) -> Result<ImportSummary, AppError> {
    if let Some(first) = checked.errors.first() {
        return Err(vfail(
            "import",
            format!(
                "{} row(s) have errors, nothing was imported. First: row {} ({}): {}",
                checked.errors.len(),
                first.row,
                first.field,
                first.message
            ),
        ));
    }

    let now = chrono::Utc::now().to_rfc3339();
    let mut summary = ImportSummary { created: 0, updated: 0, skipped: 0, stock_lots_created: 0 };
    let mut by_date: BTreeMap<String, Vec<(i64, f64, Option<f64>)>> = BTreeMap::new();
    let mut add_lots = |item_id: i64, lots: &[OpeningLot]| {
        for l in lots {
            by_date.entry(l.date.clone()).or_default().push((item_id, l.qty, Some(l.cost)));
        }
    };
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
                    // The cache starts at 0; the opening lots raise it.
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
                add_lots(item_id, &row.lots);
                summary.created += 1;
                touched.push(item_id);
            }
            Some(mut existing) => match strategy {
                ConflictStrategy::Skip => summary.skipped += 1,
                ConflictStrategy::Overwrite => {
                    // Master fields only; blank source cells never clear existing values.
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
                    if row.lots.is_empty() {
                        summary.skipped += 1;
                    } else {
                        add_lots(existing.id, &row.lots);
                        summary.updated += 1;
                    }
                }
            },
        }
    }

    for (date, lines) in &by_date {
        let ids = inventory_service.create_source_lots(
            &mut *tx,
            LotSource::Opening,
            date,
            Some(lot_notes.to_string()),
            lines,
        )?;
        summary.stock_lots_created += ids.len();
    }
    tx.commit()?;

    if let Some(indexer) = search_indexer {
        if let Ok(mut r) = uow.begin_read() {
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

fn header_report(issues: Vec<HeaderIssue>) -> ImportValidationReport {
    ImportValidationReport {
        valid_count: 0,
        rows_with_errors: Vec::new(),
        duplicates: Vec::new(),
        header_issues: issues,
        warnings: Vec::new(),
        lots_to_create: 0,
        expected_columns: expected_columns(),
    }
}

fn header_error(issues: &[HeaderIssue]) -> AppError {
    vfail(
        "header",
        format!(
            "The header row has {} problem(s), nothing was imported. {}",
            issues.len(),
            issues.iter().map(|i| i.message.as_str()).collect::<Vec<_>>().join(" ")
        ),
    )
}

fn parse_rows(raws: &[RawRow]) -> (Vec<ImportRow>, Vec<ImportRowError>) {
    let mut errors = Vec::new();
    let rows = check_rows(raws, &mut errors);
    let rows = drop_in_file_duplicates(rows, &mut errors);
    (rows, errors)
}

impl<U: UnitOfWork, I: InventoryService> ItemImportService for ItemImportServiceImpl<U, I> {
    fn validate_items_import(&self, file_path: &str) -> Result<ImportValidationReport, AppError> {
        let raws = match parse_raw(file_path)? {
            Parsed::Rows(r) => r,
            Parsed::HeaderIssues(issues) => return Ok(header_report(issues)),
        };
        let (rows, errors) = parse_rows(&raws);
        let mut tx = self.uow.begin_read()?;
        let checked = check_against_db(&mut *tx, rows, errors)?;
        Ok(build_report(checked, Vec::new()))
    }

    fn execute_items_import(
        &self,
        file_path: &str,
        strategy: ConflictStrategy,
    ) -> Result<ImportSummary, AppError> {
        let raws = match parse_raw(file_path)? {
            Parsed::Rows(r) => r,
            Parsed::HeaderIssues(issues) => return Err(header_error(&issues)),
        };
        let (rows, errors) = parse_rows(&raws);
        let mut tx = self.uow.begin_transaction()?;
        let checked = check_against_db(&mut *tx, rows, errors)?;
        execute_checked(
            &self.uow,
            &self.inventory_service,
            self.search_indexer.as_ref(),
            tx,
            checked,
            strategy,
            "Items import opening stock",
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hdr(cols: &[&str]) -> Vec<String> {
        cols.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn column_letters() {
        assert_eq!(column_letter(1), "A");
        assert_eq!(column_letter(26), "Z");
        assert_eq!(column_letter(27), "AA");
    }

    #[test]
    fn suggestions_for_common_mistakes() {
        assert_eq!(suggest_column("Sale Rate"), Some("Sale Price"));
        assert_eq!(suggest_column("Item_Name"), Some("Item Name"));
        assert_eq!(suggest_column("ItemNme"), Some("Item Name"));
        assert_eq!(suggest_column("Qty"), Some("Opening Stock"));
        assert_eq!(suggest_column("Purchase Price"), Some("Purchase Cost"));
        assert_eq!(suggest_column("Unit"), Some("Base Unit"));
        assert_eq!(suggest_column("Pcs per box"), Some("Units Per Bundle"));
        assert_eq!(suggest_column("M.R.P."), Some("MRP"));
        assert_eq!(suggest_column("Colour"), None);
    }

    #[test]
    fn header_lists_every_problem_at_once() {
        let issues = parse_header(&hdr(&["Name", "Base Unit", "Sale Rate", "MRP", "mrp", "Colour"])).unwrap_err();
        let kinds: Vec<_> = issues.iter().map(|i| (i.kind, i.column.as_str())).collect();
        assert!(kinds.contains(&(HeaderIssueKind::Duplicate, "mrp")), "{issues:?}");
        assert!(kinds.contains(&(HeaderIssueKind::Unknown, "Name")));
        assert!(kinds.contains(&(HeaderIssueKind::Unknown, "Sale Rate")));
        assert!(kinds.contains(&(HeaderIssueKind::Unknown, "Colour")));
        assert!(kinds.contains(&(HeaderIssueKind::Missing, "Purchase Cost")));
        let sale = issues.iter().find(|i| i.column == "Sale Rate").unwrap();
        assert_eq!(sale.suggestion.as_deref(), Some("Sale Price"));
        assert_eq!(sale.column_letter.as_deref(), Some("C"));
        assert!(sale.message.contains("Rename it to \"Sale Price\""), "{}", sale.message);
    }

    #[test]
    fn header_extra_spaces_are_fine() {
        assert!(parse_header(&hdr(&["  Item   Name ", "BASE UNIT", "sale\u{00A0}price", "Purchase Cost"])).is_ok());
    }

    #[test]
    fn decodes_windows_1252() {
        assert_eq!(decode_csv_bytes(b"Caf\xe9"), "Café");
        assert_eq!(decode_csv_bytes("\u{feff}₹".as_bytes()), "₹");
    }
}
