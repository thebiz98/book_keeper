use crate::core::dtos::{ItemLot, Paginated, StockAdjustmentPayload, StockAdjustmentResult};
use crate::core::entities::{PurchaseItem, StockAdjustment};
use crate::core::error::AppError;
use crate::core::value_objects::{LotSource, OutflowSourceType};
use crate::data::contracts::unit_of_work::TransactionContext;
use super::inventory_allocation_strategy::DepletionResult;

/// Sole writer of `items.current_stock` / `is_low_stock`, lot `remaining_quantity`, allocation
/// `returned_quantity` and the outflow/allocation rows (NFR-05). Invariant after every call:
/// `current_stock == Σ lot.remaining_quantity` per item (± EPS).
pub trait InventoryService: Send + Sync {
    /// Validates sufficient stock for every item. `requirements` holds the required base qty
    /// per item already summed across all lines of the operation.
    fn validate_stock_availability(
        &self,
        uow: &mut dyn TransactionContext,
        requirements: &[(i64 /*item_id*/, f64 /*total_base_qty*/)],
    ) -> Result<(), AppError>;

    /// Depletes lots via the strategy and decrements the cache by the quantity moved.
    fn deplete_stock_for_sale(
        &self,
        uow: &mut dyn TransactionContext,
        sale_item_id: i64,
        item_id: i64,
        required_base_qty: f64,
    ) -> Result<DepletionResult, AppError>;

    /// Creates a purchase lot and increments the cache by the lot's remaining quantity.
    fn add_stock_for_purchase(
        &self,
        uow: &mut dyn TransactionContext,
        purchase_item: &PurchaseItem,
    ) -> Result<(), AppError>;

    /// Linked customer return: reverses sale allocations via the strategy and, when `restock`,
    /// increments the cache. Returns the reversed FIFO cost.
    fn reverse_sale_line(
        &self,
        uow: &mut dyn TransactionContext,
        sale_item_id: i64,
        item_id: i64,
        quantity: f64,
        restock: bool,
    ) -> Result<f64, AppError>;

    /// Creates a non-purchase lot (Adjustment / Opening / Return) under its own zero-value
    /// header (own code sequence, party None, PAID, no ledger row) and increments the cache.
    /// Cost: explicit, else the item's latest lot cost, else `ValidationError(unit_cost)`.
    /// Never derived from a sale price. Returns the lot id.
    fn create_source_lot(
        &self,
        uow: &mut dyn TransactionContext,
        source: LotSource,
        date: &str,
        item_id: i64,
        quantity: f64,
        unit_cost: Option<f64>,
        notes: Option<String>,
    ) -> Result<i64, AppError>;

    /// Batch form of [`Self::create_source_lot`]: ONE header for all `(item_id, quantity,
    /// unit_cost)` lines (used by the items import: one `OPN-` header per import). All lines are
    /// validated before anything is written. Returns the lot ids in line order.
    fn create_source_lots(
        &self,
        uow: &mut dyn TransactionContext,
        source: LotSource,
        date: &str,
        notes: Option<String>,
        lines: &[(i64, f64, Option<f64>)],
    ) -> Result<Vec<i64>, AppError>;

    /// Strategy-ordered outflow (negative adjustment, replacement issue): lots depleted,
    /// `StockOutflowAllocation` rows logged, cache decremented. Returns total cost.
    fn issue_stock_outflow(
        &self,
        uow: &mut dyn TransactionContext,
        source_type: OutflowSourceType,
        source_id: i64,
        item_id: i64,
        quantity: f64,
    ) -> Result<f64, AppError>;

    /// Vendor return. A named lot must be PURCHASE-source, of `item_id`, with qty <= remaining;
    /// `None` falls back to `issue_stock_outflow(VendorReturn, ..)`. Always logs outflow rows and
    /// decrements the cache. Returns Σ qty × lot unit_cost.
    fn remove_stock_for_vendor_return(
        &self,
        uow: &mut dyn TransactionContext,
        return_id: i64,
        item_id: i64,
        purchase_item_id: Option<i64>,
        quantity: f64,
    ) -> Result<f64, AppError>;

    /// Physical stock adjustment in its own write transaction.
    fn create_adjustment(
        &self,
        payload: StockAdjustmentPayload,
    ) -> Result<StockAdjustmentResult, AppError>;

    /// Lot inspector (all sources, oldest first).
    fn get_item_lots(&self, item_id: i64) -> Result<Vec<ItemLot>, AppError>;

    /// Historical stock adjustments, newest first (`adjustment_date DESC, id DESC`), paged in SQL.
    /// Defaults: page 1, limit 50; out-of-range values are rejected.
    fn list_adjustments(
        &self,
        item_id: Option<i64>,
        page: Option<usize>,
        limit: Option<usize>,
    ) -> Result<Paginated<StockAdjustment>, AppError>;

    /// Every item whose cached `current_stock` differs from Σ lot remainders (beyond EPS), as
    /// `(item_id, cached, lots_sum)`.
    fn verify_stock_cache(
        &self,
        uow: &mut dyn TransactionContext,
    ) -> Result<Vec<(i64, f64, f64)>, AppError>;

    /// Rewrites the cache of every mismatching item from its lots (via stock deltas); returns
    /// the repaired rows as `(item_id, cached_before, lots_sum)`.
    fn repair_stock_cache(
        &self,
        uow: &mut dyn TransactionContext,
    ) -> Result<Vec<(i64, f64, f64)>, AppError>;
}

impl<T: InventoryService + ?Sized> InventoryService for std::sync::Arc<T> {
    fn validate_stock_availability(
        &self,
        uow: &mut dyn TransactionContext,
        requirements: &[(i64, f64)],
    ) -> Result<(), AppError> {
        (**self).validate_stock_availability(uow, requirements)
    }

    fn deplete_stock_for_sale(
        &self,
        uow: &mut dyn TransactionContext,
        sale_item_id: i64,
        item_id: i64,
        required_base_qty: f64,
    ) -> Result<DepletionResult, AppError> {
        (**self).deplete_stock_for_sale(uow, sale_item_id, item_id, required_base_qty)
    }

    fn add_stock_for_purchase(
        &self,
        uow: &mut dyn TransactionContext,
        purchase_item: &PurchaseItem,
    ) -> Result<(), AppError> {
        (**self).add_stock_for_purchase(uow, purchase_item)
    }

    fn reverse_sale_line(
        &self,
        uow: &mut dyn TransactionContext,
        sale_item_id: i64,
        item_id: i64,
        quantity: f64,
        restock: bool,
    ) -> Result<f64, AppError> {
        (**self).reverse_sale_line(uow, sale_item_id, item_id, quantity, restock)
    }

    fn create_source_lot(
        &self,
        uow: &mut dyn TransactionContext,
        source: LotSource,
        date: &str,
        item_id: i64,
        quantity: f64,
        unit_cost: Option<f64>,
        notes: Option<String>,
    ) -> Result<i64, AppError> {
        (**self).create_source_lot(uow, source, date, item_id, quantity, unit_cost, notes)
    }

    fn create_source_lots(
        &self,
        uow: &mut dyn TransactionContext,
        source: LotSource,
        date: &str,
        notes: Option<String>,
        lines: &[(i64, f64, Option<f64>)],
    ) -> Result<Vec<i64>, AppError> {
        (**self).create_source_lots(uow, source, date, notes, lines)
    }

    fn issue_stock_outflow(
        &self,
        uow: &mut dyn TransactionContext,
        source_type: OutflowSourceType,
        source_id: i64,
        item_id: i64,
        quantity: f64,
    ) -> Result<f64, AppError> {
        (**self).issue_stock_outflow(uow, source_type, source_id, item_id, quantity)
    }

    fn remove_stock_for_vendor_return(
        &self,
        uow: &mut dyn TransactionContext,
        return_id: i64,
        item_id: i64,
        purchase_item_id: Option<i64>,
        quantity: f64,
    ) -> Result<f64, AppError> {
        (**self).remove_stock_for_vendor_return(uow, return_id, item_id, purchase_item_id, quantity)
    }

    fn create_adjustment(
        &self,
        payload: StockAdjustmentPayload,
    ) -> Result<StockAdjustmentResult, AppError> {
        (**self).create_adjustment(payload)
    }

    fn get_item_lots(&self, item_id: i64) -> Result<Vec<ItemLot>, AppError> {
        (**self).get_item_lots(item_id)
    }

    fn list_adjustments(
        &self,
        item_id: Option<i64>,
        page: Option<usize>,
        limit: Option<usize>,
    ) -> Result<Paginated<StockAdjustment>, AppError> {
        (**self).list_adjustments(item_id, page, limit)
    }

    fn verify_stock_cache(
        &self,
        uow: &mut dyn TransactionContext,
    ) -> Result<Vec<(i64, f64, f64)>, AppError> {
        (**self).verify_stock_cache(uow)
    }

    fn repair_stock_cache(
        &self,
        uow: &mut dyn TransactionContext,
    ) -> Result<Vec<(i64, f64, f64)>, AppError> {
        (**self).repair_stock_cache(uow)
    }
}
