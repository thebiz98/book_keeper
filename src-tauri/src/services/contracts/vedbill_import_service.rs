use crate::core::dtos::{ConflictStrategy, ImportSummary, ImportValidationReport};
use crate::core::error::AppError;

/// Inventory-only migration from a VedBill backup (`.vb`): item master + open stock lots.
pub trait VedbillImportService: Send + Sync {
    /// Dry run: reads the backup, maps items and lots, validates against the database. No writes.
    fn validate_vedbill_import(&self, file_path: &str) -> Result<ImportValidationReport, AppError>;

    /// Re-reads and re-validates, then imports all-or-nothing in one write transaction.
    fn execute_vedbill_import(&self, file_path: &str, strategy: ConflictStrategy) -> Result<ImportSummary, AppError>;
}

impl<T: VedbillImportService + ?Sized> VedbillImportService for std::sync::Arc<T> {
    fn validate_vedbill_import(&self, file_path: &str) -> Result<ImportValidationReport, AppError> {
        (**self).validate_vedbill_import(file_path)
    }

    fn execute_vedbill_import(&self, file_path: &str, strategy: ConflictStrategy) -> Result<ImportSummary, AppError> {
        (**self).execute_vedbill_import(file_path, strategy)
    }
}
