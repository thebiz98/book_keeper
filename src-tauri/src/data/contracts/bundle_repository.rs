use crate::core::dtos::BundleSuggestion;
use crate::core::entities::ItemBundle;
use crate::core::error::AppError;

pub trait BundleRepository {
    fn find_by_id(&self, id: i64) -> Result<Option<ItemBundle>, AppError>;
    fn find_by_item_id(&self, item_id: i64) -> Result<Vec<ItemBundle>, AppError>;
    fn find_by_barcode(&self, barcode: &str) -> Result<Option<ItemBundle>, AppError>;
    fn create(&mut self, bundle: &ItemBundle) -> Result<i64, AppError>;
    fn update(&mut self, bundle: &ItemBundle) -> Result<(), AppError>;
    fn delete(&mut self, bundle_id: i64) -> Result<(), AppError>;
    fn get_bundle_suggestions(&self) -> Result<Vec<BundleSuggestion>, AppError>;
}
