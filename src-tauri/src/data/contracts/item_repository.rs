use crate::core::entities::Item;
use crate::core::error::AppError;

pub trait ItemRepository {
    fn find_by_id(&self, id: i64) -> Result<Option<Item>, AppError>;
    fn find_by_name(&self, name: &str) -> Result<Option<Item>, AppError>;
    fn find_by_barcode(&self, barcode: &str) -> Result<Option<Item>, AppError>;
    fn list_catalog(
        &self,
        category: Option<&str>,
        low_stock_only: bool,
        offset: usize,
        limit: usize,
    ) -> Result<Vec<Item>, AppError>;
    /// Row count matching the same filters as [`Self::list_catalog`].
    fn count_catalog(&self, category: Option<&str>, low_stock_only: bool) -> Result<usize, AppError>;
    fn create(&mut self, item: &Item) -> Result<i64, AppError>;
    /// Writes master fields only (name, category, barcode, unit, prices, active flag, updated_at).
    /// The stock cache and `created_at` are never touched.
    fn update(&mut self, item: &Item) -> Result<(), AppError>;
    fn update_stock_cache(
        &mut self,
        item_id: i64,
        new_stock: f64,
        is_low_stock: bool,
    ) -> Result<(), AppError>;
    /// Applies `delta` to `current_stock` and recomputes `is_low_stock` against
    /// [`crate::core::LOW_STOCK_THRESHOLD`] in one statement. No clamping: the cache must equal
    /// the sum of lot remainders (F3/T4 routes every stock write through this).
    fn apply_stock_delta(&mut self, item_id: i64, delta: f64) -> Result<(), AppError>;
    /// One row per item: `(item_id, cached current_stock, Σ lot remaining_quantity)`. Read-only;
    /// the stock-cache invariant check compares the two (NFR-02 cache audit).
    fn stock_cache_audit(&self) -> Result<Vec<(i64, f64, f64)>, AppError>;
}
