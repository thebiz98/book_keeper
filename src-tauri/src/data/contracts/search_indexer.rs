use crate::core::dtos::ItemSearchResult;
use crate::core::entities::{Item, ItemBundle};
use crate::core::error::AppError;

/// One FTS5 table mirroring a master table. All writes use the indexer's own pooled
/// connection: call them only AFTER the owning transaction has committed (DD-24).
pub trait FtsIndex: Send + Sync {
    fn table(&self) -> &'static str;
    /// Re-reads the source rows for `key` and replaces its index rows. A missing or inactive
    /// source row means the key is removed.
    fn reindex(&self, key: i64) -> Result<(), AppError>;
    fn reindex_many(&self, keys: &[i64]) -> Result<(), AppError>;
    fn remove(&self, key: i64) -> Result<(), AppError>;
    /// Delete all + insert from the source, one transaction.
    fn rebuild(&self) -> Result<(), AppError>;
    /// `COUNT(DISTINCT key)` in the FTS table.
    fn indexed_count(&self) -> Result<usize, AppError>;
    /// Number of keys that should be indexed.
    fn source_count(&self) -> Result<usize, AppError>;
    /// Distinct keys, best rank first, then key; empty when the query has no token.
    fn search_keys(&self, query: &str, limit: usize) -> Result<Vec<i64>, AppError>;
}

impl<T: FtsIndex + ?Sized> FtsIndex for std::sync::Arc<T> {
    fn table(&self) -> &'static str {
        (**self).table()
    }
    fn reindex(&self, key: i64) -> Result<(), AppError> {
        (**self).reindex(key)
    }
    fn reindex_many(&self, keys: &[i64]) -> Result<(), AppError> {
        (**self).reindex_many(keys)
    }
    fn remove(&self, key: i64) -> Result<(), AppError> {
        (**self).remove(key)
    }
    fn rebuild(&self) -> Result<(), AppError> {
        (**self).rebuild()
    }
    fn indexed_count(&self) -> Result<usize, AppError> {
        (**self).indexed_count()
    }
    fn source_count(&self) -> Result<usize, AppError> {
        (**self).source_count()
    }
    fn search_keys(&self, query: &str, limit: usize) -> Result<Vec<i64>, AppError> {
        (**self).search_keys(query, limit)
    }
}

pub trait ItemSearchIndexer: Send + Sync {
    /// High-speed typeahead lookup matching prefix across name, barcode, and category (< 1ms).
    fn search(&self, query: &str) -> Result<Vec<ItemSearchResult>, AppError> {
        self.search_with_limit(query, 8)
    }

    fn search_with_limit(&self, query: &str, limit: usize) -> Result<Vec<ItemSearchResult>, AppError>;

    /// One page of search hits with `category` / `low_stock_only` applied in SQL, plus the total
    /// number of matching items. Same ranking as [`Self::search_with_limit`]. A query with no
    /// searchable token matches nothing.
    fn search_page(
        &self,
        query: &str,
        category: Option<&str>,
        low_stock_only: bool,
        offset: usize,
        limit: usize,
    ) -> Result<(Vec<ItemSearchResult>, usize), AppError>;

    /// Indexes or updates an item, reading its bundles from storage.
    fn index_item(&self, item: &Item) -> Result<(), AppError> {
        self.index_item_with_bundles(item, &[])
    }

    /// Indexes or updates an item; an empty `bundles` slice means "load them from storage".
    fn index_item_with_bundles(&self, item: &Item, bundles: &[ItemBundle]) -> Result<(), AppError>;

    /// Deletes an item from the search index.
    fn remove_item(&self, item_id: i64) -> Result<(), AppError>;

    /// Number of distinct items currently in the index (startup consistency check).
    fn indexed_item_count(&self) -> Result<usize, AppError>;

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

    fn search_page(
        &self,
        query: &str,
        category: Option<&str>,
        low_stock_only: bool,
        offset: usize,
        limit: usize,
    ) -> Result<(Vec<ItemSearchResult>, usize), AppError> {
        (**self).search_page(query, category, low_stock_only, offset, limit)
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

    fn indexed_item_count(&self) -> Result<usize, AppError> {
        (**self).indexed_item_count()
    }

}
