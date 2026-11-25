//! Shared import core (FR-41, DD-70): file -> rows for CSV / XLSX, header matching with
//! suggestions, number and date parsing, and the report helpers. Every importer (items, service
//! components, parties, expenses) is built on this; the items importer's behaviour is unchanged.

use std::collections::HashMap;
use calamine::{open_workbook_auto, Data, Reader};
use crate::core::dtos::{ExpectedColumn, HeaderIssue, HeaderIssueKind, ImportRowError, ImportValidationReport};
use crate::core::error::AppError;
use crate::core::value_objects::clean_name;

/// One template column: the lower-case `key` is what the header is matched against.
pub(crate) struct ColumnSpec {
    pub key: &'static str,
    pub display: &'static str,
    pub required: bool,
    pub numeric: bool,
    pub rule: &'static str,
    pub example: &'static str,
}

pub(crate) fn vfail(field: &str, message: impl Into<String>) -> AppError {
    AppError::ValidationError { field: field.to_string(), message: message.into() }
}

pub(crate) fn today() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
}

pub(crate) fn expected_columns_of(specs: &[&ColumnSpec]) -> Vec<ExpectedColumn> {
    specs
        .iter()
        .map(|c| ExpectedColumn {
            name: c.display.to_string(),
            required: c.required,
            rule: c.rule.to_string(),
            example: c.example.to_string(),
        })
        .collect()
}

pub(crate) fn expected_list(specs: &[&ColumnSpec]) -> String {
    specs
        .iter()
        .map(|c| if c.required { format!("{}*", c.display) } else { c.display.to_string() })
        .collect::<Vec<_>>()
        .join(", ")
}

pub(crate) fn display_of(specs: &[&ColumnSpec], key: &str) -> &'static str {
    specs.iter().find(|c| c.key == key).map(|c| c.display).expect("known column key")
}

// ---------------------------------------------------------------------------------------------
// File reading
// ---------------------------------------------------------------------------------------------

/// Excel serial day number -> `YYYY-MM-DD` (1900 date system).
pub(crate) fn excel_serial_to_date(serial: f64) -> Option<String> {
    if !serial.is_finite() || !(1.0..2_958_466.0).contains(&serial) {
        return None;
    }
    let base = chrono::NaiveDate::from_ymd_opt(1899, 12, 30)?;
    base.checked_add_signed(chrono::Duration::days(serial.floor() as i64)).map(|d| d.format("%Y-%m-%d").to_string())
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
        Data::DateTime(dt) => excel_serial_to_date(dt.as_f64()).unwrap_or_else(|| dt.to_string()),
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

pub(crate) type Grid = Vec<(usize, Vec<String>)>;

fn range_to_grid(range: &calamine::Range<Data>) -> Grid {
    let (row0, col0) = range.start().map(|(r, c)| (r as usize, c as usize)).unwrap_or((0, 0));
    let mut out = Vec::new();
    for (i, r) in range.rows().enumerate() {
        let mut cells = vec![String::new(); col0];
        cells.extend(r.iter().map(cell_text));
        out.push((row0 + i + 1, cells));
    }
    out
}

/// Reads `.csv` / `.xlsx` / `.xls` into raw string grids: `(sheet_row_number, cells)`.
/// Spreadsheets read their first sheet.
pub(crate) fn read_grid(path: &str) -> Result<Grid, AppError> {
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
        Ok(range_to_grid(&range))
    } else {
        Err(vfail("file", "Unsupported file type. Save the sheet as .xlsx or .csv and pick that file."))
    }
}

/// The grid of the worksheet called `sheet` (case-insensitive), or `None` for a CSV or a workbook
/// without that sheet.
pub(crate) fn read_named_sheet(path: &str, sheet: &str) -> Result<Option<Grid>, AppError> {
    let lower = path.trim().to_lowercase();
    if !(lower.ends_with(".xlsx") || lower.ends_with(".xls")) {
        return Ok(None);
    }
    let mut wb = open_workbook_auto(path).map_err(|e| {
        AppError::IoError(format!("Could not open the workbook: {e}. If it is open in Excel, close it and try again."))
    })?;
    let Some(name) = wb.sheet_names().into_iter().find(|n| n.trim().eq_ignore_ascii_case(sheet)) else {
        return Ok(None);
    };
    let range = wb
        .worksheet_range(&name)
        .map_err(|e| AppError::IoError(format!("The worksheet \"{name}\" could not be read: {e}")))?;
    Ok(Some(range_to_grid(&range)))
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

pub(crate) fn squash(h: &str) -> String {
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

/// The template column a misspelt header most likely meant (ignoring case, spaces, punctuation).
pub(crate) fn suggest_by_spelling(specs: &[&'static ColumnSpec], header: &str) -> Option<&'static str> {
    let sq = squash(header);
    if sq.is_empty() {
        return None;
    }
    let best = specs.iter().map(|c| (levenshtein(&sq, &squash(c.display)), *c)).min_by_key(|(d, _)| *d);
    if let Some((d, c)) = best {
        let limit = (squash(c.display).len() / 3).clamp(1, 3);
        if d <= limit {
            return Some(c.display);
        }
    }
    None
}

/// Maps header cells to column keys, or lists every header problem at once.
/// `suggest` maps an unknown header to the display name of the column it most likely meant.
pub(crate) fn parse_header(
    header: &[String],
    specs: &[&'static ColumnSpec],
    suggest: &dyn Fn(&str) -> Option<&'static str>,
) -> Result<Vec<(usize, &'static str)>, Vec<HeaderIssue>> {
    let mut cols: Vec<(usize, &'static str)> = Vec::new();
    let mut first_pos: HashMap<&'static str, usize> = HashMap::new();
    let mut unknown: Vec<(usize, String)> = Vec::new();
    let mut issues = Vec::new();

    for (idx, h) in header.iter().enumerate() {
        let norm = norm_header(h);
        if norm.is_empty() {
            continue;
        }
        match specs.iter().find(|c| c.key == norm) {
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
        let suggestion = suggest(&h);
        let message = match suggestion {
            Some(s) => {
                let key = specs.iter().find(|c| c.display == s).map(|c| c.key).unwrap_or("");
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
                expected_list(specs)
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

    for c in specs.iter().filter(|c| c.required && !first_pos.contains_key(c.key)) {
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
pub(crate) struct RawRow {
    pub row: usize,
    pub cells: HashMap<&'static str, String>,
}

impl RawRow {
    pub fn text(&self, key: &str) -> String {
        self.cells.get(key).map(String::as_str).unwrap_or("").to_string()
    }
}

pub(crate) enum Parsed {
    Rows(Vec<RawRow>),
    HeaderIssues(Vec<HeaderIssue>),
}

/// Header + rows of an already-read grid. A blank grid is a file-level error.
pub(crate) fn parse_grid(
    grid: Grid,
    specs: &[&'static ColumnSpec],
    suggest: &dyn Fn(&str) -> Option<&'static str>,
) -> Result<Parsed, AppError> {
    let mut iter = grid.into_iter().skip_while(|(_, c)| c.iter().all(|x| x.is_empty()));
    let Some((_, header)) = iter.next() else {
        return Err(vfail("header", format!("The file is empty. Row 1 must hold the column headers: {}", expected_list(specs))));
    };
    let cols = match parse_header(&header, specs, suggest) {
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

pub(crate) fn parse_raw(
    path: &str,
    specs: &[&'static ColumnSpec],
    suggest: &dyn Fn(&str) -> Option<&'static str>,
) -> Result<Parsed, AppError> {
    parse_grid(read_grid(path)?, specs, suggest)
}

// ---------------------------------------------------------------------------------------------
// Cell checks
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

pub(crate) fn push_error(
    errors: &mut Vec<ImportRowError>,
    row: usize,
    name: Option<&str>,
    field: &str,
    message: impl Into<String>,
) {
    errors.push(ImportRowError { row, name: name.map(str::to_string), field: field.to_string(), message: message.into() });
}

/// A number cell that must be 0 or more. `None` for a blank optional cell or after an error.
pub(crate) fn parse_number(
    specs: &[&ColumnSpec],
    raw: &RawRow,
    name: Option<&str>,
    key: &'static str,
    required: bool,
    errors: &mut Vec<ImportRowError>,
) -> Option<f64> {
    let text = raw.cells.get(key).map(String::as_str).unwrap_or("");
    let field = display_of(specs, key);
    let mut err = |message: String| {
        push_error(errors, raw.row, name, field, message);
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

/// Normalises a date cell to `YYYY-MM-DD`: `YYYY-MM-DD`, `DD-MM-YYYY`, `DD/MM/YYYY`,
/// `YYYY/MM/DD` (Excel date cells are already converted to ISO by the reader).
pub(crate) fn parse_date_text(text: &str) -> Option<String> {
    let t = text.trim();
    let fmts = ["%Y-%m-%d", "%d-%m-%Y", "%d/%m/%Y", "%Y/%m/%d", "%d.%m.%Y"];
    // Anything after a date-time "T" / space is dropped (ISO date-times from spreadsheets).
    let date_part = t.split(['T', ' ']).next().unwrap_or(t);
    fmts.iter()
        .find_map(|f| chrono::NaiveDate::parse_from_str(date_part, f).ok())
        .map(|d| d.format("%Y-%m-%d").to_string())
}

pub(crate) fn date_hint() -> &'static str {
    "Use YYYY-MM-DD (e.g. 2026-11-25) or DD-MM-YYYY."
}

pub(crate) fn header_report(issues: Vec<HeaderIssue>, specs: &[&ColumnSpec]) -> ImportValidationReport {
    ImportValidationReport {
        valid_count: 0,
        rows_with_errors: Vec::new(),
        duplicates: Vec::new(),
        header_issues: issues,
        warnings: Vec::new(),
        lots_to_create: 0,
        expected_columns: expected_columns_of(specs),
        info: Vec::new(),
    }
}

pub(crate) fn header_error(issues: &[HeaderIssue]) -> AppError {
    vfail(
        "header",
        format!(
            "The header row has {} problem(s), nothing was imported. {}",
            issues.len(),
            issues.iter().map(|i| i.message.as_str()).collect::<Vec<_>>().join(" ")
        ),
    )
}

/// The error `execute_*` returns when the dry run still has row errors (nothing is written).
pub(crate) fn rows_have_errors(errors: &[ImportRowError]) -> Option<AppError> {
    errors.first().map(|first| {
        vfail(
            "import",
            format!(
                "{} row(s) have errors, nothing was imported. First: row {} ({}): {}",
                errors.len(),
                first.row,
                first.field,
                first.message
            ),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_accept_common_formats() {
        assert_eq!(parse_date_text("2026-11-25").as_deref(), Some("2026-11-25"));
        assert_eq!(parse_date_text("25-11-2026").as_deref(), Some("2026-11-25"));
        assert_eq!(parse_date_text("25/11/2026").as_deref(), Some("2026-11-25"));
        assert_eq!(parse_date_text("2026-11-25T00:00:00").as_deref(), Some("2026-11-25"));
        assert_eq!(parse_date_text("yesterday"), None);
        assert_eq!(parse_date_text("31-02-2026"), None);
    }

    #[test]
    fn excel_serials_become_dates() {
        assert_eq!(excel_serial_to_date(46000.0).as_deref(), Some("2025-12-09"));
        assert_eq!(excel_serial_to_date(-3.0), None);
    }
}
