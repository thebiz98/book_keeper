use crate::core::dtos::{ConflictStrategy, ImportSummary, ImportValidationReport};
use crate::core::error::AppError;

pub trait ItemImportService: Send + Sync {
    /// Dry run: parses `.csv` / `.xlsx` (by extension), validates every row and detects
    /// duplicates against the database. No writes.
    fn validate_items_import(&self, file_path: &str) -> Result<ImportValidationReport, AppError>;

    /// Re-parses and re-validates, then imports all-or-nothing in one write transaction.
    fn execute_items_import(
        &self,
        file_path: &str,
        strategy: ConflictStrategy,
    ) -> Result<ImportSummary, AppError>;
}

impl<T: ItemImportService + ?Sized> ItemImportService for std::sync::Arc<T> {
    fn validate_items_import(&self, file_path: &str) -> Result<ImportValidationReport, AppError> {
        (**self).validate_items_import(file_path)
    }

    fn execute_items_import(
        &self,
        file_path: &str,
        strategy: ConflictStrategy,
    ) -> Result<ImportSummary, AppError> {
        (**self).execute_items_import(file_path, strategy)
    }
}
