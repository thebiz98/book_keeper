use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Arc;
use crate::core::dtos::{
    ConflictStrategy, ExpectedColumn, HeaderIssue, ImportDuplicate, ImportRowError, ImportRowWarning,
    ImportSummary, ImportValidationReport, ServiceComponentInput,
};
use crate::core::entities::{Item, ItemBundle};
use crate::core::error::AppError;
use crate::core::stock_rules::{is_low_stock, DEFAULT_LOW_STOCK_THRESHOLD};
use crate::core::value_objects::{clean_name, clean_optional, ItemType, LotSource};
use crate::data::contracts::search_indexer::ItemSearchIndexer;
use crate::data::contracts::unit_of_work::{TransactionContext, UnitOfWork};
use crate::services::contracts::inventory_service::InventoryService;
use crate::services::contracts::item_import_service::ItemImportService;
use super::import_core as core;
pub(crate) use super::import_core::today;
use super::import_core::{ColumnSpec, Parsed, RawRow};
use super::item_service_impl::{component_rule_violation, validate_components, validate_service_cost};

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
const H_LOW: &str = "low stock level";
const H_TYPE: &str = "item type";
const H_SCOST: &str = "service cost";

/// Template A, in file order (the legacy 11 columns; pinned by the v1 tests).
pub(crate) static COLUMNS: [ColumnSpec; 11] = [
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
        rule: "Number, 0 or more. Cost per unit of the opening stock. Optional for a SERVICE row.", example: "6.5" },
    ColumnSpec { key: H_STOCK, display: "Opening Stock", required: false, numeric: true,
        rule: "Number, 0 or more. More than 0 creates an opening stock lot dated today. Must be empty for a SERVICE row.", example: "100" },
    ColumnSpec { key: H_BUNDLE, display: "Packaging Bundle", required: false, numeric: false,
        rule: "Text, e.g. Box. Give it together with Units Per Bundle.", example: "Box" },
    ColumnSpec { key: H_UNITS, display: "Units Per Bundle", required: false, numeric: true,
        rule: "Number greater than 1. Give it together with Packaging Bundle.", example: "10" },
    ColumnSpec { key: H_LOW, display: "Low Stock Level", required: false, numeric: true,
        rule: "Number, 0 or more. Low-stock alert level in base units. Empty = 5; 0 = never.", example: "10" },
];

/// FR-41 optional columns: accepted by the parser, added to the new template.
pub(crate) static EXTRA_COLUMNS: [ColumnSpec; 2] = [
    ColumnSpec { key: H_TYPE, display: "Item Type", required: false, numeric: false,
        rule: "PHYSICAL (default when empty) or SERVICE. A service has no stock, bundles or purchases.", example: "SERVICE" },
    ColumnSpec { key: H_SCOST, display: "Service Cost", required: false, numeric: true,
        rule: "SERVICE rows only. Number, 0 or more: your cost per unit of service, a floor over the components' cost.", example: "5" },
];

pub(crate) const H_COMP_SERVICE: &str = "service name";
pub(crate) const H_COMP_ITEM: &str = "component name";
pub(crate) const H_COMP_QTY: &str = "quantity per unit";

/// Components sheet / service components CSV columns (FR-41).
pub(crate) static COMPONENT_COLUMNS: [ColumnSpec; 3] = [
    ColumnSpec { key: H_COMP_SERVICE, display: "Service Name", required: true, numeric: false,
        rule: "Text. The service these components belong to (an item of type SERVICE).", example: "Photocopy A4" },
    ColumnSpec { key: H_COMP_ITEM, display: "Component Name", required: true, numeric: false,
        rule: "Text. A physical item (in your items or in this file).", example: "A4 Paper" },
    ColumnSpec { key: H_COMP_QTY, display: "Quantity Per Unit", required: true, numeric: true,
        rule: "Number greater than 0. Base units of the component used by one unit of the service.", example: "1" },
];

/// Every column the items parser accepts, template columns first.
pub(crate) fn item_specs() -> Vec<&'static ColumnSpec> {
    COLUMNS.iter().chain(EXTRA_COLUMNS.iter()).collect()
}

pub(crate) fn component_specs() -> Vec<&'static ColumnSpec> {
    COMPONENT_COLUMNS.iter().collect()
}

fn display(key: &str) -> &'static str {
    core::display_of(&item_specs(), key)
}

/// The legacy template columns as the report lists them.
pub(crate) fn expected_columns() -> Vec<ExpectedColumn> {
    core::expected_columns_of(&COLUMNS.iter().collect::<Vec<_>>())
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
    /// `None` = blank cell (new item: 5; OVERWRITE: keep the current level).
    pub low_stock_threshold: Option<f64>,
    /// `None` = blank cell (new item: PHYSICAL; an existing item keeps its type).
    pub item_type: Option<ItemType>,
    /// SERVICE rows only.
    pub service_cost: Option<f64>,
}

// ---------------------------------------------------------------------------------------------
// Header validation with suggestions
// ---------------------------------------------------------------------------------------------

/// The template column a wrong header most likely meant.
pub(crate) fn suggest_column(header: &str) -> Option<&'static str> {
    let specs = item_specs();
    // 1. Spelling: closest template name, ignoring case, spaces and punctuation.
    if let Some(s) = core::suggest_by_spelling(&specs, header) {
        return Some(s);
    }
    let sq = core::squash(header);
    if sq.is_empty() {
        return None;
    }
    // 2. Meaning: common words used for the same thing in other software.
    let has = |w: &[&str]| w.iter().any(|x| sq.contains(x));
    let key = if has(&["lowstock", "reorder", "minstock", "minimum", "minqty", "alertlevel"]) {
        H_LOW
    } else if has(&["servicecost", "servicecharge"]) {
        H_SCOST
    } else if has(&["itemtype", "producttype"]) {
        H_TYPE
    } else if has(&["perbundle", "perbox", "perpack", "conversion", "pcsper", "unitsper", "packsize"]) {
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

#[cfg(test)]
fn parse_header(header: &[String]) -> Result<Vec<(usize, &'static str)>, Vec<HeaderIssue>> {
    core::parse_header(header, &item_specs(), &suggest_column)
}

fn suggest_component_column(header: &str) -> Option<&'static str> {
    let specs = component_specs();
    if let Some(s) = core::suggest_by_spelling(&specs, header) {
        return Some(s);
    }
    let sq = core::squash(header);
    let has = |w: &[&str]| w.iter().any(|x| sq.contains(x));
    let key = if has(&["qty", "quantity", "perunit"]) {
        H_COMP_QTY
    } else if has(&["service"]) {
        H_COMP_SERVICE
    } else if has(&["component", "material", "item", "ingredient"]) {
        H_COMP_ITEM
    } else {
        return None;
    };
    Some(core::display_of(&specs, key))
}

fn parse_raw(path: &str) -> Result<Parsed, AppError> {
    core::parse_raw(path, &item_specs(), &suggest_column)
}

// ---------------------------------------------------------------------------------------------
// Row checks
// ---------------------------------------------------------------------------------------------

fn parse_number(raw: &RawRow, name: Option<&str>, key: &'static str, required: bool, errors: &mut Vec<ImportRowError>) -> Option<f64> {
    core::parse_number(&item_specs(), raw, name, key, required, errors)
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
        let type_text = text(H_TYPE);
        let item_type = match type_text.trim().to_uppercase().as_str() {
            "" => None,
            "PHYSICAL" => Some(ItemType::Physical),
            "SERVICE" => Some(ItemType::Service),
            _ => {
                push(errors, H_TYPE, format!("Item Type \"{type_text}\" is not valid. Use PHYSICAL or SERVICE, or leave it empty for PHYSICAL."));
                None
            }
        };
        let is_service = item_type == Some(ItemType::Service);
        let base_unit = clean_name(&text(H_UNIT));
        if base_unit.is_empty() {
            push(errors, H_UNIT, "Base Unit is empty. Enter a unit such as PCS, KG or BOX.".into());
        }
        let sale_price = parse_number(raw, nm, H_PRICE, true, errors);
        let cost = parse_number(raw, nm, H_COST, !is_service, errors);
        let mrp = parse_number(raw, nm, H_MRP, false, errors);
        if mrp == Some(0.0) {
            push(errors, H_MRP, "MRP is 0. Leave it empty for no MRP, or enter a number greater than 0.".into());
        }
        let opening = parse_number(raw, nm, H_STOCK, false, errors);
        let low = parse_number(raw, nm, H_LOW, false, errors);
        let mut service_cost = parse_number(raw, nm, H_SCOST, false, errors);
        if let Some(c) = service_cost {
            if is_service {
                // The item service's own rule (finite, >= 0, rounded).
                service_cost = validate_service_cost(Some(c)).ok().flatten();
            } else {
                push(errors, H_SCOST, "Service Cost is only for rows with Item Type SERVICE. Clear it, or set Item Type to SERVICE.".into());
            }
        }

        let barcode = Some(text(H_BARCODE).trim().to_string()).filter(|b| !b.is_empty());

        let bname = clean_name(&text(H_BUNDLE));
        let units_text = text(H_UNITS);
        let mut bundle = None;
        if is_service {
            // X5: a service has no stock and no bundles.
            if !text(H_STOCK).is_empty() {
                push(errors, H_STOCK, "A SERVICE row cannot have Opening Stock (a service has no stock). Clear it.".into());
            }
            if !bname.is_empty() || !units_text.is_empty() {
                push(errors, H_BUNDLE, "A SERVICE row cannot have a Packaging Bundle (a service has no bundles). Clear both bundle columns.".into());
            }
        } else {
            match (bname.is_empty(), units_text.is_empty()) {
                (true, true) => {}
                (false, true) => push(errors, H_UNITS, format!("Packaging Bundle \"{bname}\" is given, so Units Per Bundle is needed too (how many units are in one {bname}).")),
                (true, false) => push(errors, H_BUNDLE, "Units Per Bundle is given, so Packaging Bundle is needed too (e.g. Box). Or clear both.".into()),
                (false, false) => match units_text.parse::<f64>() {
                    Ok(u) if u.is_finite() && u > 1.0 => bundle = Some((bname, u)),
                    _ => push(errors, H_UNITS, format!("Units Per Bundle \"{units_text}\" must be a number greater than 1.")),
                },
            }
        }

        if errors.len() == before {
            let opening = opening.unwrap_or(0.0);
            let lots = if opening > 0.0 && !is_service {
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
                low_stock_threshold: low,
                item_type,
                service_cost,
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

/// One service and the components the file gives it (FR-41).
#[derive(Debug, Clone)]
pub(crate) struct ServicePlan {
    /// Name as written in the file.
    pub service_name: String,
    /// The file's Items sheet has this service (new or existing)? `false` = a service already in the DB.
    pub in_file: bool,
    /// The service exists in the database already.
    pub existing: bool,
    pub components: Vec<(String, f64)>,
}

pub(crate) struct Checked {
    /// Rows without errors, each with the existing item it matches by name (if any).
    pub rows: Vec<(ImportRow, Option<Item>)>,
    pub errors: Vec<ImportRowError>,
    pub duplicates: Vec<ImportDuplicate>,
    /// Component lists to set once the items exist (items import / service components import).
    pub components: Vec<ServicePlan>,
}

fn type_word(t: ItemType) -> &'static str {
    match t {
        ItemType::Physical => "a physical item",
        ItemType::Service => "a service",
    }
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
        let mut fail = |field: &str, message: String| {
            errors.push(ImportRowError { row: row.row, name: Some(row.name.clone()), field: field.to_string(), message });
            true
        };
        if let Some(e) = &existing {
            if let Some(t) = row.item_type {
                if t != e.item_type {
                    clash = fail(
                        display(H_TYPE),
                        format!("\"{}\" already exists as {}. An item's type cannot change; use another name.", e.name, type_word(e.item_type)),
                    );
                }
            }
            if e.item_type == ItemType::Service && (!row.lots.is_empty() || row.bundle.is_some()) {
                clash = fail(
                    display(H_STOCK),
                    format!("\"{}\" is a service, so it cannot get opening stock or a bundle.", e.name),
                );
            }
        }
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
    Ok(Checked { rows, errors, duplicates, components: Vec::new() })
}

// ---------------------------------------------------------------------------------------------
// Components (FR-41): Components sheet and the service components CSV
// ---------------------------------------------------------------------------------------------

/// One line of a component list as written in the file.
#[derive(Debug, Clone)]
pub(crate) struct ComponentRow {
    pub row: usize,
    pub service: String,
    pub component: String,
    pub qty: f64,
}

pub(crate) enum ParsedComponents {
    Rows(Vec<ComponentRow>),
    HeaderIssues(Vec<HeaderIssue>),
}

/// Cell-level checks of a components grid. Header problems are returned as such.
pub(crate) fn parse_component_grid(
    grid: core::Grid,
    errors: &mut Vec<ImportRowError>,
) -> Result<ParsedComponents, AppError> {
    let specs = component_specs();
    let raws = match core::parse_grid(grid, &specs, &suggest_component_column)? {
        Parsed::Rows(r) => r,
        Parsed::HeaderIssues(i) => return Ok(ParsedComponents::HeaderIssues(i)),
    };
    let mut out = Vec::new();
    for raw in &raws {
        let service = clean_name(&raw.text(H_COMP_SERVICE));
        let component = clean_name(&raw.text(H_COMP_ITEM));
        let nm = Some(service.as_str()).filter(|n| !n.is_empty());
        let before = errors.len();
        if service.is_empty() {
            core::push_error(errors, raw.row, nm, "Service Name", "Service Name is empty.");
        }
        if component.is_empty() {
            core::push_error(errors, raw.row, nm, "Component Name", "Component Name is empty.");
        }
        let qty = core::parse_number(&specs, raw, nm, H_COMP_QTY, true, errors);
        if qty == Some(0.0) {
            core::push_error(errors, raw.row, nm, "Quantity Per Unit", "Quantity Per Unit must be greater than 0.");
        }
        if errors.len() == before {
            out.push(ComponentRow { row: raw.row, service, component, qty: qty.unwrap_or(0.0) });
        }
    }
    Ok(ParsedComponents::Rows(out))
}

/// Prefixes header problems of the Components sheet so the user knows which sheet to fix.
fn components_sheet_issues(issues: Vec<HeaderIssue>) -> Vec<HeaderIssue> {
    issues
        .into_iter()
        .map(|mut i| {
            i.message = format!("Components sheet: {}", i.message);
            i
        })
        .collect()
}

fn virtual_item(id: i64, name: &str, item_type: ItemType) -> Item {
    Item {
        id,
        name: name.to_string(),
        item_type,
        category: None,
        barcode: None,
        base_unit: String::new(),
        mrp: None,
        default_sale_price: 0.0,
        current_stock: 0.0,
        is_low_stock: false,
        is_active: true,
        created_at: String::new(),
        updated_at: String::new(),
        low_stock_threshold: DEFAULT_LOW_STOCK_THRESHOLD,
        service_cost: None,
    }
}

/// Resolves the component lines against the database and the (valid) rows of the same file and
/// checks each line with the item service's own rule (`component_rule_violation`).
/// `file` is `None` for the service components CSV (services must already exist).
/// `file_names` = every name on the Items sheet, valid or not (a row that has its own error is
/// not reported a second time).
pub(crate) fn plan_components(
    tx: &mut dyn TransactionContext,
    comps: &[ComponentRow],
    file: Option<&[(ImportRow, Option<Item>)]>,
    file_names: &HashSet<String>,
    errors: &mut Vec<ImportRowError>,
) -> Result<Vec<ServicePlan>, AppError> {
    // name (lower) -> the item as the import will leave it (real id, or a virtual negative id)
    let mut from_file: HashMap<String, Item> = HashMap::new();
    if let Some(rows) = file {
        for (row, existing) in rows {
            let item = match existing {
                Some(e) => {
                    let mut e = e.clone();
                    if let Some(t) = row.item_type {
                        e.item_type = t;
                    }
                    e
                }
                None => virtual_item(-(row.row as i64), &row.name, row.item_type.unwrap_or(ItemType::Physical)),
            };
            from_file.insert(row.name.to_lowercase(), item);
        }
    }

    let mut order: Vec<String> = Vec::new();
    let mut groups: HashMap<String, Vec<&ComponentRow>> = HashMap::new();
    for c in comps {
        let key = c.service.to_lowercase();
        if !groups.contains_key(&key) {
            order.push(key.clone());
        }
        groups.entry(key).or_default().push(c);
    }

    let mut plans = Vec::new();
    for key in order {
        let lines = &groups[&key];
        let first = lines[0];
        let (service, in_file) = match from_file.get(&key) {
            Some(i) => (Some(i.clone()), true),
            None if file_names.contains(&key) => continue, // that Items row has its own error
            None => (tx.items().find_by_name(&first.service)?, false),
        };
        let Some(service) = service else {
            core::push_error(
                errors,
                first.row,
                Some(&first.service),
                "Service Name",
                format!("Service \"{}\" was not found{}. Add it as a SERVICE item first.", first.service, if file.is_some() { " in the Items sheet or in your items" } else { " in your items" }),
            );
            continue;
        };
        if service.item_type != ItemType::Service {
            core::push_error(
                errors,
                first.row,
                Some(&first.service),
                "Service Name",
                format!("\"{}\" is {}, not a service, so it cannot have components.", service.name, type_word(service.item_type)),
            );
            continue;
        }
        let mut listed: Vec<i64> = Vec::new();
        let mut components: Vec<(String, f64)> = Vec::new();
        for c in lines {
            let comp = match from_file.get(&c.component.to_lowercase()) {
                Some(i) => Some(i.clone()),
                None if file_names.contains(&c.component.to_lowercase()) => continue,
                None => tx.items().find_by_name(&c.component)?,
            };
            let Some(comp) = comp else {
                core::push_error(
                    errors,
                    c.row,
                    Some(&c.service),
                    "Component Name",
                    format!("Component \"{}\" was not found{}.", c.component, if file.is_some() { " in the Items sheet or in your items" } else { " in your items" }),
                );
                continue;
            };
            match component_rule_violation(Some(service.id), &comp, c.qty, listed.contains(&comp.id)) {
                Some(message) => core::push_error(errors, c.row, Some(&c.service), "Component Name", message),
                None => {
                    listed.push(comp.id);
                    components.push((comp.name.clone(), c.qty));
                }
            }
        }
        plans.push(ServicePlan { service_name: service.name.clone(), in_file, existing: service.id > 0, components });
    }
    Ok(plans)
}

/// Replaces a service's component list inside `tx`, through the item service's own validation.
pub(crate) fn apply_component_plan(
    tx: &mut dyn TransactionContext,
    plan: &ServicePlan,
) -> Result<(), AppError> {
    let service = tx
        .items()
        .find_by_name(&plan.service_name)?
        .ok_or_else(|| AppError::NotFound { entity: "Item".into(), id: plan.service_name.clone() })?;
    let mut inputs = Vec::with_capacity(plan.components.len());
    for (name, qty) in &plan.components {
        let comp = tx
            .items()
            .find_by_name(name)?
            .ok_or_else(|| AppError::NotFound { entity: "Item".into(), id: name.clone() })?;
        inputs.push(ServiceComponentInput { component_item_id: comp.id, quantity_per_unit: *qty });
    }
    let rows = validate_components(tx, Some(service.id), &inputs)?;
    tx.service_components().replace_for_service(service.id, &rows)
}

pub(crate) fn build_report(checked: Checked, mut warnings: Vec<ImportRowWarning>) -> ImportValidationReport {
    warnings.sort_by_key(|w| w.row);
    let lots_to_create = checked.rows.iter().filter(|(_, e)| e.is_none()).map(|(r, _)| r.lots.len()).sum();
    let services = checked.rows.iter().filter(|(r, e)| e.is_none() && r.item_type == Some(ItemType::Service)).count();
    let mut info = Vec::new();
    if services > 0 {
        info.push(format!("{services} new service item(s) will be created (no stock)."));
    }
    let lists = checked.components.iter().filter(|p| !p.components.is_empty()).count();
    if lists > 0 {
        info.push(format!("{lists} service component list(s) will be set."));
    }
    ImportValidationReport {
        valid_count: checked.rows.len(),
        rows_with_errors: checked.errors,
        duplicates: checked.duplicates,
        header_issues: Vec::new(),
        warnings,
        lots_to_create,
        expected_columns: expected_columns(),
        info,
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
    if let Some(e) = core::rows_have_errors(&checked.errors) {
        return Err(e);
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
                let item_type = row.item_type.unwrap_or(ItemType::Physical);
                let threshold = row.low_stock_threshold.unwrap_or(DEFAULT_LOW_STOCK_THRESHOLD);
                let item = Item {
                    id: 0,
                    name: row.name.clone(),
                    item_type,
                    category: row.category.clone(),
                    barcode: row.barcode.clone(),
                    base_unit: row.base_unit.clone(),
                    mrp: row.mrp,
                    default_sale_price: row.sale_price,
                    // The cache starts at 0; the opening lots raise it.
                    current_stock: 0.0,
                    is_low_stock: is_low_stock(item_type, 0.0, threshold),
                    is_active: true,
                    created_at: now.clone(),
                    updated_at: now.clone(),
                    low_stock_threshold: threshold,
                    service_cost: if item_type == ItemType::Service { row.service_cost } else { None },
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
                    if let Some(t) = row.low_stock_threshold {
                        existing.low_stock_threshold = t;
                    }
                    if existing.item_type == ItemType::Service && row.service_cost.is_some() {
                        existing.service_cost = row.service_cost;
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

    // FR-41: component lists. A service that already existed is only touched by OVERWRITE.
    for plan in &checked.components {
        if plan.existing && strategy != ConflictStrategy::Overwrite {
            continue;
        }
        apply_component_plan(&mut *tx, plan)?;
        if plan.existing && !plan.in_file {
            summary.updated += 1;
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

/// Header issues of the Items sheet and (XLSX) the Components sheet, rows of both, errors.
struct ParsedFile {
    rows: Vec<ImportRow>,
    errors: Vec<ImportRowError>,
    /// Every name on the Items sheet (lower-case), valid or not.
    file_names: HashSet<String>,
    comps: Vec<ComponentRow>,
}

enum Prepared {
    Ok(ParsedFile),
    Header(Vec<HeaderIssue>),
}

fn prepare(path: &str) -> Result<Prepared, AppError> {
    let raws = match parse_raw(path)? {
        Parsed::Rows(r) => r,
        Parsed::HeaderIssues(issues) => return Ok(Prepared::Header(issues)),
    };
    let file_names: HashSet<String> = raws
        .iter()
        .map(|r| clean_name(&r.text(H_NAME)).to_lowercase())
        .filter(|n| !n.is_empty())
        .collect();
    let mut errors = Vec::new();
    let rows = check_rows(&raws, &mut errors);
    let rows = drop_in_file_duplicates(rows, &mut errors);

    let mut comps = Vec::new();
    if let Some(grid) = core::read_named_sheet(path, "Components")? {
        let mut comp_errors = Vec::new();
        match parse_component_grid(grid, &mut comp_errors) {
            Ok(ParsedComponents::Rows(r)) => comps = r,
            Ok(ParsedComponents::HeaderIssues(i)) => return Ok(Prepared::Header(components_sheet_issues(i))),
            // An empty Components sheet is fine: nothing to set.
            Err(_) => {}
        }
        for e in &mut comp_errors {
            e.message = format!("Components sheet, row {}: {}", e.row, e.message);
        }
        errors.extend(comp_errors);
    }
    Ok(Prepared::Ok(ParsedFile { rows, errors, file_names, comps }))
}

fn check_all(tx: &mut dyn TransactionContext, f: ParsedFile) -> Result<Checked, AppError> {
    let ParsedFile { rows, errors, file_names, comps } = f;
    let mut checked = check_against_db(tx, rows, errors)?;
    if !comps.is_empty() {
        let mut errs = std::mem::take(&mut checked.errors);
        let before = errs.len();
        let plans = plan_components(tx, &comps, Some(&checked.rows), &file_names, &mut errs)?;
        for e in &mut errs[before..] {
            e.message = format!("Components sheet, row {}: {}", e.row, e.message);
        }
        errs.sort_by_key(|e| e.row);
        checked.errors = errs;
        checked.components = plans;
    }
    Ok(checked)
}

impl<U: UnitOfWork, I: InventoryService> ItemImportService for ItemImportServiceImpl<U, I> {
    fn validate_items_import(&self, file_path: &str) -> Result<ImportValidationReport, AppError> {
        let file = match prepare(file_path)? {
            Prepared::Ok(f) => f,
            Prepared::Header(issues) => return Ok(core::header_report(issues, &COLUMNS.iter().collect::<Vec<_>>())),
        };
        let mut tx = self.uow.begin_read()?;
        let checked = check_all(&mut *tx, file)?;
        Ok(build_report(checked, Vec::new()))
    }

    fn execute_items_import(
        &self,
        file_path: &str,
        strategy: ConflictStrategy,
    ) -> Result<ImportSummary, AppError> {
        let file = match prepare(file_path)? {
            Prepared::Ok(f) => f,
            Prepared::Header(issues) => return Err(core::header_error(&issues)),
        };
        let mut tx = self.uow.begin_transaction()?;
        let checked = check_all(&mut *tx, file)?;
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
    use crate::core::dtos::HeaderIssueKind;
    use super::super::import_core::{column_letter, decode_csv_bytes};

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
    fn optional_service_columns_are_accepted() {
        assert!(parse_header(&hdr(&["Item Name", "Base Unit", "Sale Price", "Purchase Cost", "Item Type", "Service Cost"])).is_ok());
        assert_eq!(suggest_column("Service charge"), Some("Service Cost"));
    }

    #[test]
    fn decodes_windows_1252() {
        assert_eq!(decode_csv_bytes(b"Caf\xe9"), "Café");
        assert_eq!(decode_csv_bytes("\u{feff}₹".as_bytes()), "₹");
    }
}
