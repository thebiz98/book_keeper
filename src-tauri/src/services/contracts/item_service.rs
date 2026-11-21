use crate::core::dtos::{
    BundleSuggestion, CreateBundlePayload, CatalogFilter, CreateItemPayload, ItemSearchResult, Paginated, UpdateBundlePayload,
    UpdateItemPayload,
};
use crate::core::entities::ItemBundle;
use crate::core::error::AppError;

pub trait ItemService: Send + Sync {
    /// Paginated catalog (name order) with server-side `category`, `low_stock_only` and `search`
    /// (FTS); all filters combine. Rows carry their bundles.
    fn get_catalog(&self, filter: CatalogFilter) -> Result<Paginated<ItemSearchResult>, AppError>;
    /// Typeahead search (default limit 8, max 200).
    fn search_items_fts(&self, query: &str, limit: Option<usize>) -> Result<Vec<ItemSearchResult>, AppError>;
    /// Distinct categories of active items.
    fn list_categories(&self) -> Result<Vec<String>, AppError>;
    fn create_item(&self, payload: CreateItemPayload) -> Result<i64, AppError>;
    fn update_item(&self, item_id: i64, payload: UpdateItemPayload) -> Result<(), AppError>;
    fn create_bundle(&self, payload: CreateBundlePayload) -> Result<ItemBundle, AppError>;
    fn update_bundle(&self, bundle_id: i64, payload: UpdateBundlePayload) -> Result<ItemBundle, AppError>;
    fn delete_bundle(&self, bundle_id: i64) -> Result<(), AppError>;
    fn get_bundle_suggestions(&self) -> Result<Vec<BundleSuggestion>, AppError>;
}

impl<T: ItemService + ?Sized> ItemService for std::sync::Arc<T> {
    fn get_catalog(&self, filter: CatalogFilter) -> Result<Paginated<ItemSearchResult>, AppError> {
        (**self).get_catalog(filter)
    }

    fn search_items_fts(&self, query: &str, limit: Option<usize>) -> Result<Vec<ItemSearchResult>, AppError> {
        (**self).search_items_fts(query, limit)
    }

    fn list_categories(&self) -> Result<Vec<String>, AppError> {
        (**self).list_categories()
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
