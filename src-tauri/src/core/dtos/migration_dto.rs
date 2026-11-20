use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TableStats {
    pub items_count: usize,
    pub parties_count: usize,
    pub sales_count: usize,
    pub purchases_count: usize,
    pub document_items_count: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MigrationPreview {
    pub company_name: String,
    pub schema_version: i32,
    pub created_at: String,
    pub backup_file_path: String,
    pub backup_file_size_bytes: u64,
    pub table_stats: TableStats,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MigrationResult {
    pub success: bool,
    pub items_imported: usize,
    pub parties_imported: usize,
    pub sales_imported: usize,
    pub purchases_imported: usize,
    pub duration_ms: u64,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BackupFileInfo {
    pub file_name: String,
    pub file_path: String,
    pub file_size_bytes: u64,
    pub modified_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImportSummary {
    pub total_rows: usize,
    pub imported_count: usize,
    pub skipped_count: usize,
    pub errors: Vec<String>,
}
