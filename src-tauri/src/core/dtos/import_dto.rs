use serde::{Deserialize, Serialize};

/// How an item already in the database (matched by case-insensitive name) is handled (LLD 6.14).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ConflictStrategy {
    Skip,
    Overwrite,
    AddStock,
}

/// One validation problem. `row` is the real sheet row number (header = row 1).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImportRowError {
    pub row: usize,
    pub field: String,
    pub message: String,
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
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImportSummary {
    pub created: usize,
    pub updated: usize,
    pub skipped: usize,
    pub stock_lots_created: usize,
}
