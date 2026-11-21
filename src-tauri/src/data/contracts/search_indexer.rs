use crate::core::dtos::ItemSearchResult;
use crate::core::entities::{Item, ItemBundle};
use crate::core::error::AppError;

pub trait ItemSearchIndexer: Send + Sync {
    /// High-speed typeahead lookup matching prefix across name, barcode, and category (< 1ms).
    fn search(&self, query: &str) -> Result<Vec<ItemSearchResult>, AppError> {
        self.search_with_limit(query, 50)
    }

    fn search_with_limit(&self, query: &str, limit: usize) -> Result<Vec<ItemSearchResult>, AppError>;

    /// Indexes or updates an item, reading its bundles from storage.
    fn index_item(&self, item: &Item) -> Result<(), AppError> {
        self.index_item_with_bundles(item, &[])
    }

    /// Indexes or updates an item; an empty `bundles` slice means "load them from storage".
    fn index_item_with_bundles(&self, item: &Item, bundles: &[ItemBundle]) -> Result<(), AppError>;

    /// Deletes an item from the search index.
    fn remove_item(&self, item_id: i64) -> Result<(), AppError>;

    /// Rebuilds the search index entirely from storage (migration, bulk restore).
    fn rebuild_index(&self) -> Result<(), AppError>;
}

impl<T: ItemSearchIndexer + ?Sized> ItemSearchIndexer for std::sync::Arc<T> {
    fn search(&self, query: &str) -> Result<Vec<ItemSearchResult>, AppError> {
        (**self).search(query)
    }

    fn search_with_limit(&self, query: &str, limit: usize) -> Result<Vec<ItemSearchResult>, AppError> {
        (**self).search_with_limit(query, limit)
    }

    fn index_item(&self, item: &Item) -> Result<(), AppError> {
        (**self).index_item(item)
    }

    fn index_item_with_bundles(&self, item: &Item, bundles: &[ItemBundle]) -> Result<(), AppError> {
        (**self).index_item_with_bundles(item, bundles)
    }

    fn remove_item(&self, item_id: i64) -> Result<(), AppError> {
        (**self).remove_item(item_id)
    }

    fn rebuild_index(&self) -> Result<(), AppError> {
        (**self).rebuild_index()
    }

}
