use crate::core::entities::Item;
use crate::core::error::AppError;

pub trait ItemRepository {
    fn find_by_id(&self, id: i64) -> Result<Option<Item>, AppError>;
    fn find_by_name(&self, name: &str) -> Result<Option<Item>, AppError>;
    fn find_by_barcode(&self, barcode: &str) -> Result<Option<Item>, AppError>;
    /// One page of the active catalog by name (`name COLLATE NOCASE, id`); category match is
    /// case-insensitive. Pair with [`Self::count_catalog`] for the total.
    fn list_catalog(
        &self,
        category: Option<&str>,
        low_stock_only: bool,
        offset: usize,
        limit: usize,
    ) -> Result<Vec<Item>, AppError>;
    /// Row count matching the same filters as [`Self::list_catalog`].
    fn count_catalog(&self, category: Option<&str>, low_stock_only: bool) -> Result<usize, AppError>;
    /// Distinct non-blank categories of active items, case-insensitively sorted.
    fn list_categories(&self) -> Result<Vec<String>, AppError>;
    fn create(&mut self, item: &Item) -> Result<i64, AppError>;
    /// Writes master fields only (name, category, barcode, unit, prices, active flag, updated_at).
    /// The stock cache and `created_at` are never touched.
    fn update(&mut self, item: &Item) -> Result<(), AppError>;
    /// Sets `current_stock`; `is_low_stock` is derived in the same statement from the item's own
    /// `low_stock_threshold` ([`crate::core::LOW_STOCK_SQL`]).
    fn update_stock_cache(&mut self, item_id: i64, new_stock: f64) -> Result<(), AppError>;
    /// Sets the per-item low stock level and recomputes its flag. NotFound when no such row.
    fn update_low_stock_threshold(&mut self, item_id: i64, threshold: f64, updated_at: &str) -> Result<(), AppError>;
    /// Re-derives every `is_low_stock` flag; returns the number of rows corrected.
    fn recompute_low_stock_flags(&mut self) -> Result<usize, AppError>;
    /// Applies `delta` to `current_stock` and recomputes `is_low_stock` against the item's own
    /// `low_stock_threshold` in one statement. No clamping: the cache must equal
    /// the sum of lot remainders (F3/T4 routes every stock write through this).
    fn apply_stock_delta(&mut self, item_id: i64, delta: f64) -> Result<(), AppError>;
    /// One row per item: `(item_id, cached current_stock, Σ lot remaining_quantity)`. Read-only;
    /// the stock-cache invariant check compares the two (NFR-02 cache audit).
    fn stock_cache_audit(&self) -> Result<Vec<(i64, f64, f64)>, AppError>;
}
