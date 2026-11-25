use crate::core::dtos::{
    BundleSuggestion, CreateBundlePayload, CatalogFilter, CreateItemPayload, DeleteItemResult, DeletedItemRow, ItemDeletePreview, ItemSearchResult, Paginated, SavedBundle,
    ServiceComponentView, UpdateBundlePayload, UpdateItemPayload,
};
use crate::core::error::AppError;

pub trait ItemService: Send + Sync {
    /// Paginated catalog (name order) with server-side `category`, `low_stock_only` and `search`
    /// (FTS); all filters combine. Rows carry their bundles.
    fn get_catalog(&self, filter: CatalogFilter) -> Result<Paginated<ItemSearchResult>, AppError>;
    /// Typeahead search (default limit 8, max 200).
    fn search_items_fts(&self, query: &str, limit: Option<usize>) -> Result<Vec<ItemSearchResult>, AppError>;
    /// Distinct categories of active items.
    /// One item by id with bundles and, for a service, "Can make N". NotFound when missing. Read-only.
    fn get_item(&self, item_id: i64) -> Result<ItemSearchResult, AppError>;
    fn list_categories(&self) -> Result<Vec<String>, AppError>;
    fn create_item(&self, payload: CreateItemPayload) -> Result<i64, AppError>;
    fn update_item(&self, item_id: i64, payload: UpdateItemPayload) -> Result<(), AppError>;
    /// Sets the per-item low stock level (0 = never) and returns the refreshed row with bundles.
    fn update_low_stock_threshold(&self, item_id: i64, threshold: f64) -> Result<ItemSearchResult, AppError>;
    fn create_bundle(&self, payload: CreateBundlePayload) -> Result<SavedBundle, AppError>;
    fn update_bundle(&self, bundle_id: i64, payload: UpdateBundlePayload) -> Result<SavedBundle, AppError>;
    fn delete_bundle(&self, bundle_id: i64) -> Result<(), AppError>;
    fn get_bundle_suggestions(&self) -> Result<Vec<BundleSuggestion>, AppError>;
    /// FR-36: the components of a service with their names, units and current stock.
    fn get_service_components(&self, item_id: i64) -> Result<Vec<ServiceComponentView>, AppError>;
    /// FR-45: read-only dry run of a delete: stock, stock value, blocking services, `can_delete`.
    fn preview_item_delete(&self, item_id: i64) -> Result<ItemDeletePreview, AppError>;
    /// FR-45: soft delete (`is_active = 0`). Blocked while an active service uses it as a component.
    /// A1 (DD-79): a never-used item is removed permanently (`HARD`), any other is soft-deleted.
    fn delete_item(&self, item_id: i64) -> Result<DeleteItemResult, AppError>;
    /// FR-45: reverse of `delete_item`; a service needs all its components active.
    fn restore_item(&self, item_id: i64) -> Result<(), AppError>;
    /// FR-45: deleted items (name `LIKE`, newest delete first), default page size 10.
    fn list_deleted_items(
        &self,
        search: Option<String>,
        page: Option<usize>,
        page_size: Option<usize>,
    ) -> Result<Paginated<DeletedItemRow>, AppError>;
}

impl<T: ItemService + ?Sized> ItemService for std::sync::Arc<T> {
    fn get_catalog(&self, filter: CatalogFilter) -> Result<Paginated<ItemSearchResult>, AppError> {
        (**self).get_catalog(filter)
    }

    fn search_items_fts(&self, query: &str, limit: Option<usize>) -> Result<Vec<ItemSearchResult>, AppError> {
        (**self).search_items_fts(query, limit)
    }

    fn get_item(&self, item_id: i64) -> Result<ItemSearchResult, AppError> {
        (**self).get_item(item_id)
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

    fn update_low_stock_threshold(&self, item_id: i64, threshold: f64) -> Result<ItemSearchResult, AppError> {
        (**self).update_low_stock_threshold(item_id, threshold)
    }

    fn create_bundle(&self, payload: CreateBundlePayload) -> Result<SavedBundle, AppError> {
        (**self).create_bundle(payload)
    }

    fn update_bundle(&self, bundle_id: i64, payload: UpdateBundlePayload) -> Result<SavedBundle, AppError> {
        (**self).update_bundle(bundle_id, payload)
    }

    fn delete_bundle(&self, bundle_id: i64) -> Result<(), AppError> {
        (**self).delete_bundle(bundle_id)
    }

    fn get_bundle_suggestions(&self) -> Result<Vec<BundleSuggestion>, AppError> {
        (**self).get_bundle_suggestions()
    }

    fn get_service_components(&self, item_id: i64) -> Result<Vec<ServiceComponentView>, AppError> {
        (**self).get_service_components(item_id)
    }

    fn preview_item_delete(&self, item_id: i64) -> Result<ItemDeletePreview, AppError> {
        (**self).preview_item_delete(item_id)
    }

    fn delete_item(&self, item_id: i64) -> Result<DeleteItemResult, AppError> {
        (**self).delete_item(item_id)
    }

    fn restore_item(&self, item_id: i64) -> Result<(), AppError> {
        (**self).restore_item(item_id)
    }

    fn list_deleted_items(
        &self,
        search: Option<String>,
        page: Option<usize>,
        page_size: Option<usize>,
    ) -> Result<Paginated<DeletedItemRow>, AppError> {
        (**self).list_deleted_items(search, page, page_size)
    }
}
