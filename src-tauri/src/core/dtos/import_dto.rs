use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImportSummary {
    pub total_rows: usize,
    pub imported_count: usize,
    pub skipped_count: usize,
    pub errors: Vec<String>,
}
