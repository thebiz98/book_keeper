use crate::core::dtos::ItemSearchResult;
use crate::core::entities::{Item, ItemBundle};
use crate::core::error::AppError;

pub trait ItemSearchIndexer: Send + Sync {
    /// High-speed typeahead lookup matching prefix across name, barcode, and category (< 1ms).
    fn search(&self, query: &str) -> Result<Vec<ItemSearchResult>, AppError> {
        self.search_with_limit(query, 50)
    }

    fn search_with_limit(&self, query: &str, _limit: usize) -> Result<Vec<ItemSearchResult>, AppError> {
        self.search(query)
    }

    /// Indexes or updates an item in the search catalog.
    fn index_item(&self, item: &Item) -> Result<(), AppError> {
        self.index_item_with_bundles(item, &[])
    }

    fn index_item_with_bundles(&self, item: &Item, _bundles: &[ItemBundle]) -> Result<(), AppError> {
        self.index_item(item)
    }

    /// Deletes an item from the search index (or marks it inactive).
    fn remove_item(&self, item_id: i64) -> Result<(), AppError>;

    /// Rebuilds the search index entirely from storage (used in migration & bulk restore).
    fn rebuild_index(&self) -> Result<(), AppError> {
        self.rebuild_index_with_bundles(&[])
    }

    fn rebuild_index_with_bundles(&self, _items_with_bundles: &[(Item, Vec<ItemBundle>)]) -> Result<(), AppError> {
        self.rebuild_index()
    }
}
