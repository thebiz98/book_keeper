use crate::core::dtos::ImportSummary;
use crate::core::error::AppError;

pub trait DataExportImportService: Send + Sync {
    fn export_items_to_excel(&self, output_path: &str) -> Result<(), AppError>;
    fn import_items_from_excel(&self, input_path: &str) -> Result<ImportSummary, AppError>;
    fn export_transactions_to_excel(
        &self,
        from_date: &str,
        to_date: &str,
        output_path: &str,
    ) -> Result<(), AppError>;
}

pub type ImportExportService = dyn DataExportImportService;

impl<T: DataExportImportService + ?Sized> DataExportImportService for std::sync::Arc<T> {
    fn export_items_to_excel(&self, output_path: &str) -> Result<(), AppError> {
        (**self).export_items_to_excel(output_path)
    }

    fn import_items_from_excel(&self, input_path: &str) -> Result<ImportSummary, AppError> {
        (**self).import_items_from_excel(input_path)
    }

    fn export_transactions_to_excel(
        &self,
        from_date: &str,
        to_date: &str,
        output_path: &str,
    ) -> Result<(), AppError> {
        (**self).export_transactions_to_excel(from_date, to_date, output_path)
    }
}
