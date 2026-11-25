use crate::core::dtos::{ConflictStrategy, ImportSummary, ImportValidationReport};
use crate::core::error::AppError;

/// FR-41 imports besides items: service components (CSV/XLSX), parties, expenses.
/// `validate_*` is a dry run and never writes; `execute_*` re-validates and imports
/// all-or-nothing in one write transaction.
pub trait RecordImportService: Send + Sync {
    fn validate_service_components_import(&self, file_path: &str) -> Result<ImportValidationReport, AppError>;
    fn execute_service_components_import(&self, file_path: &str) -> Result<ImportSummary, AppError>;

    /// `strategy` decides what happens to a party that exists already (same name and type,
    /// ignoring case): `Skip`, or `Overwrite` (the UI's "Update").
    fn validate_parties_import(&self, file_path: &str, strategy: ConflictStrategy) -> Result<ImportValidationReport, AppError>;
    fn execute_parties_import(&self, file_path: &str, strategy: ConflictStrategy) -> Result<ImportSummary, AppError>;

    fn validate_expenses_import(&self, file_path: &str) -> Result<ImportValidationReport, AppError>;
    fn execute_expenses_import(&self, file_path: &str) -> Result<ImportSummary, AppError>;
}

impl<T: RecordImportService + ?Sized> RecordImportService for std::sync::Arc<T> {
    fn validate_service_components_import(&self, file_path: &str) -> Result<ImportValidationReport, AppError> {
        (**self).validate_service_components_import(file_path)
    }
    fn execute_service_components_import(&self, file_path: &str) -> Result<ImportSummary, AppError> {
        (**self).execute_service_components_import(file_path)
    }
    fn validate_parties_import(&self, file_path: &str, strategy: ConflictStrategy) -> Result<ImportValidationReport, AppError> {
        (**self).validate_parties_import(file_path, strategy)
    }
    fn execute_parties_import(&self, file_path: &str, strategy: ConflictStrategy) -> Result<ImportSummary, AppError> {
        (**self).execute_parties_import(file_path, strategy)
    }
    fn validate_expenses_import(&self, file_path: &str) -> Result<ImportValidationReport, AppError> {
        (**self).validate_expenses_import(file_path)
    }
    fn execute_expenses_import(&self, file_path: &str) -> Result<ImportSummary, AppError> {
        (**self).execute_expenses_import(file_path)
    }
}
