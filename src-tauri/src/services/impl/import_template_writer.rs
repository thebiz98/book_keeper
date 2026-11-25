//! Downloadable import templates (FR-11, FR-41), built from each validator's own column list so a
//! template can never drift from what its import accepts.

use std::path::{Path, PathBuf};
use rust_xlsxwriter::{
    Color, DataValidation, DataValidationRule, Format, FormatAlign, FormatBorder, Note, Workbook, Worksheet,
};
use crate::core::dtos::{ImportKind, TemplateFormat};
use crate::core::error::AppError;
use super::import_core::ColumnSpec;
use super::item_import_service_impl::{item_specs, component_specs, COLUMNS};
use super::record_import_service_impl::{expense_specs, party_specs};

/// Rows pre-formatted / validated in the Excel template.
const XLSX_ROWS: u32 = 5000;

fn io(e: impl std::fmt::Display, dest: &Path) -> AppError {
    let name = dest.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let msg = e.to_string();
    if msg.contains("os error 32") || msg.to_lowercase().contains("being used by another process") {
        AppError::IoError(format!("{name} is open in another program (probably Excel). Close it and try again."))
    } else {
        AppError::IoError(format!("Could not save the template: {msg}"))
    }
}

fn xlsx_err(e: rust_xlsxwriter::XlsxError) -> AppError {
    AppError::IoError(format!("Could not build the Excel template: {e}"))
}

/// What one template holds.
struct Layout {
    sheet: &'static str,
    specs: Vec<&'static ColumnSpec>,
    title: &'static str,
    notes: Vec<&'static str>,
    /// Items only: an extra `Components` sheet with the component columns.
    components_sheet: bool,
}

fn layout(kind: ImportKind) -> Layout {
    match kind {
        ImportKind::Items => Layout {
            sheet: "Items",
            specs: item_specs(),
            title: "How to fill the Items sheet",
            notes: vec![
                "Fill one item per row on the first sheet (Items). Do not rename or reorder the header row.",
                "Columns marked Required (yellow) must be filled on every row. Blue columns are optional.",
                "Extra spaces in names are removed automatically. Names must be unique (case does not matter).",
                "Items whose name already exists can be skipped, updated, or have their stock added; you choose when importing.",
                "Opening Stock creates a stock lot dated on the import day, at the Purchase Cost per unit.",
                "Low Stock Level: leave empty for 5; 0 turns the low-stock alert off.",
                "Item Type: leave empty for a physical item, or write SERVICE. A service has no stock or bundles (leave Opening Stock and the bundle columns empty); Service Cost is its own cost per unit.",
                "Components sheet: one line per component of a service (Service Name, Component Name, Quantity Per Unit). Names must be items in your list or on the Items sheet. Updating a service replaces its component list.",
                "The Instructions sheet is ignored by the importer.",
            ],
            components_sheet: true,
        },
        ImportKind::ServiceComponents => Layout {
            sheet: "Service Components",
            specs: component_specs(),
            title: "How to fill the Service Components sheet",
            notes: vec![
                "One line per component of a service. Do not rename or reorder the header row.",
                "The service must already exist as a SERVICE item; each component must be a physical item you already have.",
                "A service named in the file gets exactly the components listed (its old list is replaced).",
                "Quantity Per Unit is in the component's base unit, for one unit of the service.",
            ],
            components_sheet: false,
        },
        ImportKind::Parties => Layout {
            sheet: "Parties",
            specs: party_specs(),
            title: "How to fill the Parties sheet",
            notes: vec![
                "One customer or vendor per row. Do not rename or reorder the header row.",
                "Type must be CUSTOMER or VENDOR. The same name can be both a customer and a vendor.",
                "A party that already exists (same name and type, case ignored) is skipped or updated; you choose when importing.",
                "Opening Balance is what the party owes you (customer) or you owe them (vendor) as of As Of Date. It creates an opening bill (OB-...). As Of Date empty = today; it cannot be in the future.",
            ],
            components_sheet: false,
        },
        ImportKind::Expenses => Layout {
            sheet: "Expenses",
            specs: expense_specs(),
            title: "How to fill the Expenses sheet",
            notes: vec![
                "One expense per row. Do not rename or reorder the header row.",
                "Expense Type matches an existing type (case ignored); a new name is created for you.",
                "Date cannot be in the future. Amount must be greater than 0.",
                "A row identical to another row or to a saved expense is reported as a warning, not an error.",
            ],
            components_sheet: false,
        },
    }
}

/// CSV template: UTF-8 with BOM (so Excel shows ₹ and Hindi correctly), header row only.
fn csv_bytes(specs: &[&ColumnSpec]) -> Vec<u8> {
    let mut out = vec![0xEF, 0xBB, 0xBF];
    let header = specs.iter().map(|c| c.display).collect::<Vec<_>>().join(",");
    out.extend_from_slice(header.as_bytes());
    out.extend_from_slice(b"\r\n");
    out
}

/// Items template (legacy 11 columns), kept for callers and tests that pin it.
pub fn csv_template_bytes() -> Vec<u8> {
    csv_bytes(&COLUMNS.iter().collect::<Vec<_>>())
}

fn write_sheet_headers(
    ws: &mut Worksheet,
    specs: &[&ColumnSpec],
    required: &Format,
    optional: &Format,
    text: &Format,
    number: &Format,
) -> Result<(), AppError> {
    for (i, c) in specs.iter().enumerate() {
        let col = i as u16;
        ws.write_string_with_format(0, col, c.display, if c.required { required } else { optional })
            .map_err(xlsx_err)?;
        let width = (c.display.len() as f64 + 4.0).max(12.0);
        ws.set_column_width(col, if i == 0 { 32.0 } else { width }).map_err(xlsx_err)?;
        ws.set_column_format(col, if c.numeric { number } else { text }).map_err(xlsx_err)?;
        let note = format!("{}{}\nExample: {}", if c.required { "Required. " } else { "Optional. " }, c.rule, c.example);
        ws.insert_note(0, col, &Note::new(note).set_author("Book Keeper")).map_err(xlsx_err)?;
        if c.numeric {
            let rule = match c.display {
                "Units Per Bundle" => DataValidationRule::GreaterThan(1.0),
                "MRP" | "Amount" | "Quantity Per Unit" => DataValidationRule::GreaterThan(0.0),
                _ => DataValidationRule::GreaterThanOrEqualTo(0.0),
            };
            let dv = DataValidation::new()
                .allow_decimal_number(rule)
                .set_error_title(c.display)
                .map_err(xlsx_err)?
                .set_error_message(c.rule)
                .map_err(xlsx_err)?;
            ws.add_data_validation(1, col, XLSX_ROWS, col, &dv).map_err(xlsx_err)?;
        }
    }
    ws.set_freeze_panes(1, 0).map_err(xlsx_err)?;
    Ok(())
}

fn xlsx_bytes(l: &Layout) -> Result<Vec<u8>, AppError> {
    let mut wb = Workbook::new();

    let required = Format::new()
        .set_bold()
        .set_background_color(Color::RGB(0xFFE8A3))
        .set_border(FormatBorder::Thin)
        .set_align(FormatAlign::Center);
    let optional = Format::new()
        .set_bold()
        .set_background_color(Color::RGB(0xE7EEF7))
        .set_border(FormatBorder::Thin)
        .set_align(FormatAlign::Center);
    let text = Format::new().set_num_format("@");
    let number = Format::new().set_num_format("0.####");

    {
        let ws = wb.add_worksheet().set_name(l.sheet).map_err(xlsx_err)?;
        write_sheet_headers(ws, &l.specs, &required, &optional, &text, &number)?;
    }
    if l.components_sheet {
        let ws = wb.add_worksheet().set_name("Components").map_err(xlsx_err)?;
        write_sheet_headers(ws, &component_specs(), &required, &optional, &text, &number)?;
    }

    {
        let ws = wb.add_worksheet().set_name("Instructions").map_err(xlsx_err)?;
        let bold = Format::new().set_bold();
        let wrap = Format::new().set_text_wrap();
        ws.write_string_with_format(0, 0, l.title, &bold).map_err(xlsx_err)?;
        for (i, n) in l.notes.iter().enumerate() {
            ws.write_string_with_format(1 + i as u32, 0, *n, &wrap).map_err(xlsx_err)?;
        }
        let start = 2 + l.notes.len() as u32;
        for (c, h) in ["Column", "Required", "Rule", "Example"].iter().enumerate() {
            ws.write_string_with_format(start, c as u16, *h, &required).map_err(xlsx_err)?;
        }
        for (i, c) in l.specs.iter().enumerate() {
            let r = start + 1 + i as u32;
            ws.write_string(r, 0, c.display).map_err(xlsx_err)?;
            ws.write_string(r, 1, if c.required { "Yes" } else { "No" }).map_err(xlsx_err)?;
            ws.write_string_with_format(r, 2, c.rule, &wrap).map_err(xlsx_err)?;
            ws.write_string(r, 3, c.example).map_err(xlsx_err)?;
        }
        ws.set_column_width(0, 24.0).map_err(xlsx_err)?;
        ws.set_column_width(1, 10.0).map_err(xlsx_err)?;
        ws.set_column_width(2, 70.0).map_err(xlsx_err)?;
        ws.set_column_width(3, 18.0).map_err(xlsx_err)?;
    }

    wb.save_to_buffer().map_err(xlsx_err)
}

/// Items template (legacy 11 columns, sheets Items + Instructions), kept for tests that pin it.
pub fn xlsx_template_bytes() -> Result<Vec<u8>, AppError> {
    let mut l = layout(ImportKind::Items);
    l.specs = COLUMNS.iter().collect();
    l.components_sheet = false;
    l.notes.retain(|n| !n.starts_with("Item Type") && !n.starts_with("Components sheet"));
    xlsx_bytes(&l)
}

fn save(dest: &str, format: TemplateFormat, bytes: impl FnOnce() -> Result<Vec<u8>, AppError>) -> Result<String, AppError> {
    let dest = dest.trim();
    if dest.is_empty() {
        return Err(AppError::ValidationError { field: "dest_path".into(), message: "Choose where to save the template".into() });
    }
    let ext = match format {
        TemplateFormat::Csv => "csv",
        TemplateFormat::Xlsx => "xlsx",
    };
    let mut path = PathBuf::from(dest);
    if path.extension().map(|e| !e.eq_ignore_ascii_case(ext)).unwrap_or(true) {
        path.set_extension(ext);
    }
    let bytes = bytes()?;
    let tmp = path.with_extension(format!("{ext}.tmp-{}", uuid::Uuid::new_v4().simple()));
    std::fs::write(&tmp, &bytes).map_err(|e| io(e, &path))?;
    if let Err(e) = std::fs::rename(&tmp, &path) {
        let _ = std::fs::remove_file(&tmp);
        return Err(io(e, &path));
    }
    Ok(path.to_string_lossy().to_string())
}

/// Writes the legacy items template (11 columns) to `dest` atomically (temp file next to it, then
/// rename). The extension of `dest` is corrected to match `format`.
pub fn write_template(dest: &str, format: TemplateFormat) -> Result<String, AppError> {
    save(dest, format, || match format {
        TemplateFormat::Csv => Ok(csv_template_bytes()),
        TemplateFormat::Xlsx => xlsx_template_bytes(),
    })
}

/// Writes the template of `kind` (FR-41). Items include the optional `Item Type` / `Service Cost`
/// columns and, in Excel, the `Components` sheet.
pub fn write_import_template(kind: ImportKind, dest: &str, format: TemplateFormat) -> Result<String, AppError> {
    let l = layout(kind);
    save(dest, format, || match format {
        TemplateFormat::Csv => Ok(csv_bytes(&l.specs)),
        TemplateFormat::Xlsx => xlsx_bytes(&l),
    })
}
