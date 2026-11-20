use crate::core::dtos::{
    BundleSuggestion, CreateBundlePayload, CreateItemPayload, PaginatedList, UpdateItemPayload,
};
use crate::core::entities::Item;
use crate::core::error::AppError;

pub trait ItemService: Send + Sync {
    fn get_catalog(
        &self,
        category: Option<String>,
        low_stock_only: bool,
        page: usize,
        limit: usize,
    ) -> Result<PaginatedList<Item>, AppError>;
    fn create_item(&self, payload: CreateItemPayload) -> Result<i64, AppError>;
    fn update_item(&self, item_id: i64, payload: UpdateItemPayload) -> Result<(), AppError>;
    fn archive_item(&self, item_id: i64) -> Result<(), AppError>;
    fn create_bundle(&self, payload: CreateBundlePayload) -> Result<i64, AppError>;
    fn get_bundle_suggestions(&self) -> Result<Vec<BundleSuggestion>, AppError>;
}
