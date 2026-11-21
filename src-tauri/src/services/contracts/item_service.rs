use crate::core::dtos::{
    BundleSuggestion, CreateBundlePayload, CreateItemPayload, PaginatedList, UpdateBundlePayload, UpdateItemPayload,
};
use crate::core::entities::{Item, ItemBundle};
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
    fn create_bundle(&self, payload: CreateBundlePayload) -> Result<ItemBundle, AppError>;
    fn update_bundle(&self, bundle_id: i64, payload: UpdateBundlePayload) -> Result<ItemBundle, AppError>;
    fn delete_bundle(&self, bundle_id: i64) -> Result<(), AppError>;
    fn get_bundle_suggestions(&self) -> Result<Vec<BundleSuggestion>, AppError>;
}

impl<T: ItemService + ?Sized> ItemService for std::sync::Arc<T> {
    fn get_catalog(
        &self,
        category: Option<String>,
        low_stock_only: bool,
        page: usize,
        limit: usize,
    ) -> Result<PaginatedList<Item>, AppError> {
        (**self).get_catalog(category, low_stock_only, page, limit)
    }

    fn create_item(&self, payload: CreateItemPayload) -> Result<i64, AppError> {
        (**self).create_item(payload)
    }

    fn update_item(&self, item_id: i64, payload: UpdateItemPayload) -> Result<(), AppError> {
        (**self).update_item(item_id, payload)
    }

    fn create_bundle(&self, payload: CreateBundlePayload) -> Result<ItemBundle, AppError> {
        (**self).create_bundle(payload)
    }

    fn update_bundle(&self, bundle_id: i64, payload: UpdateBundlePayload) -> Result<ItemBundle, AppError> {
        (**self).update_bundle(bundle_id, payload)
    }

    fn delete_bundle(&self, bundle_id: i64) -> Result<(), AppError> {
        (**self).delete_bundle(bundle_id)
    }

    fn get_bundle_suggestions(&self) -> Result<Vec<BundleSuggestion>, AppError> {
        (**self).get_bundle_suggestions()
    }
}
