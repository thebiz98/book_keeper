use serde::{Deserialize, Serialize};

/// How an item already in the database (matched by case-insensitive name) is handled (LLD 6.14).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ConflictStrategy {
    Skip,
    /// "Update" in the UI (parties and items); also accepted as `"UPDATE"`.
    #[serde(alias = "UPDATE")]
    Overwrite,
    AddStock,
}

/// Downloadable import template format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TemplateFormat {
    Csv,
    Xlsx,
}

/// What an import file holds (FR-41). Picks the template and the importer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ImportKind {
    Items,
    ServiceComponents,
    Parties,
    Expenses,
}

/// One validation problem. `row` is the real sheet row number (header = row 1); for a VedBill
/// backup it is the item's 1-based position in the backup.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImportRowError {
    pub row: usize,
    /// Item name on that row, when the row has one.
    #[serde(default)]
    pub name: Option<String>,
    pub field: String,
    pub message: String,
}

/// Something the import will do differently from the source (not an error; the row still imports).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImportRowWarning {
    pub row: usize,
    pub name: String,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum HeaderIssueKind {
    Unknown,
    Missing,
    Duplicate,
}

/// A problem with the header row. Any header issue blocks the row checks and the import.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HeaderIssue {
    pub kind: HeaderIssueKind,
    /// The header text as found in the file (Unknown/Duplicate) or the expected name (Missing).
    pub column: String,
    /// 1-based column position in the file, with its spreadsheet letter (e.g. "C").
    pub position: Option<usize>,
    pub column_letter: Option<String>,
    /// Actionable, user-facing sentence.
    pub message: String,
    /// The template column the user most likely meant.
    pub suggestion: Option<String>,
}

/// A template column as the validator expects it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExpectedColumn {
    pub name: String,
    pub required: bool,
    pub rule: String,
    pub example: String,
}

/// A file row whose name already exists in the database (reported, not an error).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImportDuplicate {
    pub row: usize,
    pub name: String,
    pub existing_item_id: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImportValidationReport {
    pub valid_count: usize,
    pub rows_with_errors: Vec<ImportRowError>,
    pub duplicates: Vec<ImportDuplicate>,
    /// Non-empty means the header row must be fixed first; rows were not checked.
    pub header_issues: Vec<HeaderIssue>,
    pub warnings: Vec<ImportRowWarning>,
    /// Opening stock lots the import would create for new items (ADD_STOCK may add more for duplicates).
    pub lots_to_create: usize,
    pub expected_columns: Vec<ExpectedColumn>,
    /// Backend-written summary sentences ("2 opening bills will be created"); shown as is.
    #[serde(default)]
    pub info: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImportSummary {
    pub created: usize,
    pub updated: usize,
    pub skipped: usize,
    pub stock_lots_created: usize,
}
