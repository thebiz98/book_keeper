use crate::core::dtos::ImportSummary;
use crate::core::error::AppError;

pub trait ItemImportService: Send + Sync {
    fn import_items_from_excel(&self, input_path: &str) -> Result<ImportSummary, AppError>;
}

impl<T: ItemImportService + ?Sized> ItemImportService for std::sync::Arc<T> {
    fn import_items_from_excel(&self, input_path: &str) -> Result<ImportSummary, AppError> {
        (**self).import_items_from_excel(input_path)
    }
}
