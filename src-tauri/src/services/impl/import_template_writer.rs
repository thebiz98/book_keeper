//! Downloadable items-import templates, built from the validator's own column list so the
//! template can never drift from what the import accepts.

use std::path::{Path, PathBuf};
use rust_xlsxwriter::{
    Color, DataValidation, DataValidationRule, Format, FormatAlign, FormatBorder, Note, Workbook,
};
use crate::core::dtos::TemplateFormat;
use crate::core::error::AppError;
use super::item_import_service_impl::COLUMNS;

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

/// CSV template: UTF-8 with BOM (so Excel shows ₹ and Hindi correctly), header row only.
pub fn csv_template_bytes() -> Vec<u8> {
    let mut out = vec![0xEF, 0xBB, 0xBF];
    let header = COLUMNS.iter().map(|c| c.display).collect::<Vec<_>>().join(",");
    out.extend_from_slice(header.as_bytes());
    out.extend_from_slice(b"\r\n");
    out
}

fn xlsx_err(e: rust_xlsxwriter::XlsxError) -> AppError {
    AppError::IoError(format!("Could not build the Excel template: {e}"))
}

/// Excel template: "Items" sheet (headers only, required columns shaded, number validation)
/// plus an "Instructions" sheet that the importer ignores (it reads only the first sheet).
pub fn xlsx_template_bytes() -> Result<Vec<u8>, AppError> {
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
        let ws = wb.add_worksheet().set_name("Items").map_err(xlsx_err)?;
        for (i, c) in COLUMNS.iter().enumerate() {
            let col = i as u16;
            ws.write_string_with_format(0, col, c.display, if c.required { &required } else { &optional })
                .map_err(xlsx_err)?;
            let width = (c.display.len() as f64 + 4.0).max(12.0);
            ws.set_column_width(col, if i == 0 { 32.0 } else { width }).map_err(xlsx_err)?;
            ws.set_column_format(col, if c.numeric { &number } else { &text }).map_err(xlsx_err)?;
            let note = format!("{}{}\nExample: {}", if c.required { "Required. " } else { "Optional. " }, c.rule, c.example);
            ws.insert_note(0, col, &Note::new(note).set_author("Book Keeper")).map_err(xlsx_err)?;
            if c.numeric {
                let rule = if c.display == "Units Per Bundle" {
                    DataValidationRule::GreaterThan(1.0)
                } else if c.display == "MRP" {
                    DataValidationRule::GreaterThan(0.0)
                } else {
                    DataValidationRule::GreaterThanOrEqualTo(0.0)
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
    }

    {
        let ws = wb.add_worksheet().set_name("Instructions").map_err(xlsx_err)?;
        let bold = Format::new().set_bold();
        let wrap = Format::new().set_text_wrap();
        ws.write_string_with_format(0, 0, "How to fill the Items sheet", &bold).map_err(xlsx_err)?;
        let notes = [
            "Fill one item per row on the first sheet (Items). Do not rename or reorder the header row.",
            "Columns marked Required (yellow) must be filled on every row. Blue columns are optional.",
            "Extra spaces in names are removed automatically. Names must be unique (case does not matter).",
            "Items whose name already exists can be skipped, updated, or have their stock added; you choose when importing.",
            "Opening Stock creates a stock lot dated on the import day, at the Purchase Cost per unit.",
            "This sheet is ignored by the importer.",
        ];
        for (i, n) in notes.iter().enumerate() {
            ws.write_string_with_format(1 + i as u32, 0, *n, &wrap).map_err(xlsx_err)?;
        }
        let start = 2 + notes.len() as u32;
        for (c, h) in ["Column", "Required", "Rule", "Example"].iter().enumerate() {
            ws.write_string_with_format(start, c as u16, *h, &required).map_err(xlsx_err)?;
        }
        for (i, c) in COLUMNS.iter().enumerate() {
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

/// Writes the template to `dest` atomically (temp file next to it, then rename).
/// The extension of `dest` is corrected to match `format`.
pub fn write_template(dest: &str, format: TemplateFormat) -> Result<String, AppError> {
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
    let bytes = match format {
        TemplateFormat::Csv => csv_template_bytes(),
        TemplateFormat::Xlsx => xlsx_template_bytes()?,
    };
    let tmp = path.with_extension(format!("{ext}.tmp-{}", uuid::Uuid::new_v4().simple()));
    std::fs::write(&tmp, &bytes).map_err(|e| io(e, &path))?;
    if let Err(e) = std::fs::rename(&tmp, &path) {
        let _ = std::fs::remove_file(&tmp);
        return Err(io(e, &path));
    }
    Ok(path.to_string_lossy().to_string())
}
