use crate::core::dtos::{
    ItemBillRef, ItemLot, LotDraw, OpeningQuantityResult, Paginated, PreviewServiceCostPayload, PriorLine, ServiceCostPreview, SetItemStockPayload, SetItemStockResult, StockAdjustmentPayload,
    StockAdjustmentPreview, StockAdjustmentResult, UpdateOpeningQuantityPayload, AdjustmentUndoGuard,
};
use crate::core::entities::{PurchaseItem, StockAdjustment};
use crate::core::error::AppError;
use crate::core::value_objects::{LotSource, OutflowSourceType};
use crate::data::contracts::unit_of_work::TransactionContext;
use super::inventory_allocation_strategy::DepletionResult;
use crate::core::entities::ReturnTransaction;

/// New values of a purchase lot during a bill edit (FR-20). Quantities are base units.
#[derive(Debug, Clone, PartialEq)]
pub struct LotEdit {
    pub item_id: i64,
    pub item_name: String,
    pub pack_label: Option<String>,
    pub pack_quantity: f64,
    pub quantity: f64,
    pub unit_cost: f64,
    pub total_amount: f64,
}

/// One stock effect of a return to undo (FR-20), produced by a dry run and applied after the
/// return's rows are deleted.
#[derive(Debug, Clone, PartialEq)]
pub enum ReturnUndoStep {
    /// `returned_quantity -= take` per allocation; when `restock`, the same units leave the lots.
    Unreverse { sale_item_id: i64, item_id: i64, takes: Vec<(i64, f64)>, restock: bool },
    /// Delete the `RETURN` lot of an unlinked restockable line (and its header when empty).
    RemoveReturnLot { lot_id: i64 },
    /// Put the units of a replacement / vendor-return outflow back on their lots.
    RestoreOutflow { source_type: OutflowSourceType, source_id: i64 },
}

/// What a vendor-return line removes (DD-87): `gross_cost` = qty × lot cost (the stock value
/// leaving), `net_value` = qty × lot cost × the lot bill's net factor (the value credited).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VendorReturnCost {
    pub gross_cost: f64,
    pub net_value: f64,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct ReturnUndoPlan {
    pub steps: Vec<ReturnUndoStep>,
}

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

    /// DD-89 change-only sale edit: gives back `quantity` units the sale line drew from `item_id`
    /// (the line's item, or one component of a service line), newest allocation first, shrinking
    /// the allocations and restocking the lots and the cache. Returns the released FIFO cost.
    fn release_sale_line(
        &self,
        uow: &mut dyn TransactionContext,
        sale_item_id: i64,
        item_id: i64,
        quantity: f64,
    ) -> Result<f64, AppError>;

    /// DD-89: gives back everything the sale line drew (an item change or a removed line); the
    /// line keeps no allocation. Returns the released FIFO cost.
    fn release_whole_sale_line(&self, uow: &mut dyn TransactionContext, sale_item_id: i64) -> Result<f64, AppError>;

    /// Creates a non-purchase lot (Adjustment / Opening / Return) under its own zero-value
    /// header (own code sequence, party None, PAID, no ledger row) and increments the cache.
    /// Cost: explicit, else the item's latest lot cost, else `ValidationError(unit_cost)`.
    /// Never derived from a sale price. Returns the lot id.
    #[allow(clippy::too_many_arguments)]
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

    /// Vendor return of an exact lot (DD-86; there is no FIFO fall-back). The lot must be
    /// PURCHASE-source, of `item_id`, with qty <= remaining. Logs one `VENDOR_RETURN` outflow row
    /// at the gross lot cost and decrements the cache. Party / bill eligibility is the caller's
    /// (`vendor_return_ineligibility`). Returns the gross cost and the value net of the lot's
    /// bill discount share (DD-87), both unrounded.
    fn remove_stock_for_vendor_return(
        &self,
        uow: &mut dyn TransactionContext,
        return_id: i64,
        item_id: i64,
        purchase_item_id: i64,
        quantity: f64,
    ) -> Result<VendorReturnCost, AppError>;

    /// Physical stock adjustment in its own write transaction.
    fn create_adjustment(
        &self,
        payload: StockAdjustmentPayload,
    ) -> Result<StockAdjustmentResult, AppError>;

    /// Body of [`Self::create_adjustment`] inside the caller's transaction (same checks, same FIFO
    /// outflow for a reduction). Used by the item delete write-off (FR-58, DD-93).
    fn create_adjustment_in_tx(
        &self,
        uow: &mut dyn TransactionContext,
        payload: &StockAdjustmentPayload,
    ) -> Result<StockAdjustmentResult, AppError>;

    /// Set stock on hand: own BEGIN IMMEDIATE; delta = counted - current_stock. delta>0 records an
    /// UNRECORDED_STOCK lot, delta<0 a COUNT_CORRECTION FIFO write-off, 0 is a no-op.
    fn set_item_stock(&self, payload: SetItemStockPayload) -> Result<SetItemStockResult, AppError>;

    /// DD-56: dry run of `create_adjustment` in a transaction that is dropped: the unit cost used
    /// (the latest-lot default), the FIFO loss and the value change. Same errors as the command.
    fn preview_adjustment(&self, payload: StockAdjustmentPayload) -> Result<StockAdjustmentPreview, AppError>;

    /// DD-56: dry run of `set_item_stock` in a transaction that is dropped. Same errors as the command.
    fn preview_set_item_stock(&self, payload: SetItemStockPayload) -> Result<SetItemStockResult, AppError>;

    /// FR-51 / DD-84: correct the quantity of an OPENING lot directly (own transaction). Same floor as a lot
    /// edit (`LOT_BELOW_CONSUMED`); 0 removes an unused lot. Records no stock adjustment, so no loss.
    fn update_opening_lot_quantity(&self, payload: UpdateOpeningQuantityPayload) -> Result<OpeningQuantityResult, AppError>;

    /// DD-56: dry run of `update_opening_lot_quantity` in a transaction that is dropped. Same errors.
    fn preview_opening_lot_quantity(&self, payload: UpdateOpeningQuantityPayload) -> Result<OpeningQuantityResult, AppError>;

    /// FR-52 / DD-85: removes a stock adjustment and its effect (own transaction). A reduction gives its units
    /// back to the lots they came from; an increase removes the lot it created (refused once sold or used).
    /// Unknown id -> NotFound; a deleted item or used stock -> `BillLocked`.
    fn undo_stock_adjustment(&self, adjustment_id: i64) -> Result<AdjustmentUndoGuard, AppError>;

    /// DD-56: dry run of `undo_stock_adjustment` in a transaction that is dropped. A refusal is returned as
    /// `can_undo = false` with the reason, not as an error; an unknown id is NotFound.
    fn get_adjustment_undo_guard(&self, adjustment_id: i64) -> Result<AdjustmentUndoGuard, AppError>;

    /// FR-31 / DD-60 / FR-37: read-only draw of `quantity` base units after the `prior_lines` already on the
    /// bill (service lines count as their components), over the open lots in the strategy order. A SERVICE
    /// item returns per-component draws and its components/misc/line cost. Unknown item -> NotFound.
    fn preview_lot_draw(&self, item_id: i64, quantity: f64, prior_lines: &[PriorLine]) -> Result<LotDraw, AppError>;

    /// FR-36 / DD-56: read-only cost of one service unit from the unsaved component list and Service cost.
    fn preview_service_cost(&self, payload: PreviewServiceCostPayload) -> Result<ServiceCostPreview, AppError>;

    /// Lot inspector (all sources, oldest first).
    fn get_item_lots(&self, item_id: i64) -> Result<Vec<ItemLot>, AppError>;

    /// FR-23: bills (sales) containing the item, newest first, each bill once. Defaults: page 1,
    /// limit 10 (max 50). Unknown item -> NotFound.
    fn get_item_sales(&self, item_id: i64, page: Option<usize>, limit: Option<usize>) -> Result<Paginated<ItemBillRef>, AppError>;

    /// FR-23: `source = PURCHASE` bills containing the item; same paging as `get_item_sales`.
    fn get_item_purchases(&self, item_id: i64, page: Option<usize>, limit: Option<usize>) -> Result<Paginated<ItemBillRef>, AppError>;

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

    /// FR-20: undoes the stock effect of a sale bill: every allocation's `depleted - returned`
    /// goes back to its lot, the allocations are deleted and the cache is raised per item.
    fn reverse_sale_bill_stock(&self, uow: &mut dyn TransactionContext, sale_id: i64) -> Result<(), AppError>;

    /// FR-20: updates an existing purchase lot in place (id kept). Enforces the quantity floor and
    /// the item / vendor-return rules and re-costs dependants on a cost change (`BillLocked`).
    fn edit_purchase_lot(&self, uow: &mut dyn TransactionContext, lot_id: i64, edit: &LotEdit) -> Result<(), AppError>;

    /// FR-20: deletes an unreferenced lot and lowers the cache by its remaining quantity.
    fn remove_purchase_lot(&self, uow: &mut dyn TransactionContext, lot_id: i64) -> Result<(), AppError>;

    /// FR-20 dry run: what deleting the return would undo; `BillLocked` (`RESTOCKED_CONSUMED`,
    /// `RETURN_LOT_USED`) when impossible. Writes nothing.
    fn plan_return_undo(&self, uow: &mut dyn TransactionContext, r: &ReturnTransaction) -> Result<ReturnUndoPlan, AppError>;

    /// FR-20: applies a plan from [`Self::plan_return_undo`]. Call after the return's lines are deleted.
    fn apply_return_undo(&self, uow: &mut dyn TransactionContext, plan: &ReturnUndoPlan) -> Result<(), AppError>;
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

    fn release_sale_line(
        &self,
        uow: &mut dyn TransactionContext,
        sale_item_id: i64,
        item_id: i64,
        quantity: f64,
    ) -> Result<f64, AppError> {
        (**self).release_sale_line(uow, sale_item_id, item_id, quantity)
    }

    fn release_whole_sale_line(&self, uow: &mut dyn TransactionContext, sale_item_id: i64) -> Result<f64, AppError> {
        (**self).release_whole_sale_line(uow, sale_item_id)
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
        purchase_item_id: i64,
        quantity: f64,
    ) -> Result<VendorReturnCost, AppError> {
        (**self).remove_stock_for_vendor_return(uow, return_id, item_id, purchase_item_id, quantity)
    }

    fn create_adjustment(
        &self,
        payload: StockAdjustmentPayload,
    ) -> Result<StockAdjustmentResult, AppError> {
        (**self).create_adjustment(payload)
    }

    fn create_adjustment_in_tx(
        &self,
        uow: &mut dyn TransactionContext,
        payload: &StockAdjustmentPayload,
    ) -> Result<StockAdjustmentResult, AppError> {
        (**self).create_adjustment_in_tx(uow, payload)
    }

    fn set_item_stock(&self, payload: SetItemStockPayload) -> Result<SetItemStockResult, AppError> {
        (**self).set_item_stock(payload)
    }

    fn preview_adjustment(&self, payload: StockAdjustmentPayload) -> Result<StockAdjustmentPreview, AppError> {
        (**self).preview_adjustment(payload)
    }

    fn preview_set_item_stock(&self, payload: SetItemStockPayload) -> Result<SetItemStockResult, AppError> {
        (**self).preview_set_item_stock(payload)
    }

    fn update_opening_lot_quantity(&self, payload: UpdateOpeningQuantityPayload) -> Result<OpeningQuantityResult, AppError> {
        (**self).update_opening_lot_quantity(payload)
    }

    fn preview_opening_lot_quantity(&self, payload: UpdateOpeningQuantityPayload) -> Result<OpeningQuantityResult, AppError> {
        (**self).preview_opening_lot_quantity(payload)
    }

    fn undo_stock_adjustment(&self, adjustment_id: i64) -> Result<AdjustmentUndoGuard, AppError> {
        (**self).undo_stock_adjustment(adjustment_id)
    }

    fn get_adjustment_undo_guard(&self, adjustment_id: i64) -> Result<AdjustmentUndoGuard, AppError> {
        (**self).get_adjustment_undo_guard(adjustment_id)
    }

    fn preview_lot_draw(&self, item_id: i64, quantity: f64, prior_lines: &[PriorLine]) -> Result<LotDraw, AppError> {
        (**self).preview_lot_draw(item_id, quantity, prior_lines)
    }

    fn preview_service_cost(&self, payload: PreviewServiceCostPayload) -> Result<ServiceCostPreview, AppError> {
        (**self).preview_service_cost(payload)
    }

    fn get_item_lots(&self, item_id: i64) -> Result<Vec<ItemLot>, AppError> {
        (**self).get_item_lots(item_id)
    }

    fn get_item_sales(&self, item_id: i64, page: Option<usize>, limit: Option<usize>) -> Result<Paginated<ItemBillRef>, AppError> {
        (**self).get_item_sales(item_id, page, limit)
    }

    fn get_item_purchases(&self, item_id: i64, page: Option<usize>, limit: Option<usize>) -> Result<Paginated<ItemBillRef>, AppError> {
        (**self).get_item_purchases(item_id, page, limit)
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

    fn reverse_sale_bill_stock(&self, uow: &mut dyn TransactionContext, sale_id: i64) -> Result<(), AppError> {
        (**self).reverse_sale_bill_stock(uow, sale_id)
    }

    fn edit_purchase_lot(&self, uow: &mut dyn TransactionContext, lot_id: i64, edit: &LotEdit) -> Result<(), AppError> {
        (**self).edit_purchase_lot(uow, lot_id, edit)
    }

    fn remove_purchase_lot(&self, uow: &mut dyn TransactionContext, lot_id: i64) -> Result<(), AppError> {
        (**self).remove_purchase_lot(uow, lot_id)
    }

    fn plan_return_undo(&self, uow: &mut dyn TransactionContext, r: &ReturnTransaction) -> Result<ReturnUndoPlan, AppError> {
        (**self).plan_return_undo(uow, r)
    }

    fn apply_return_undo(&self, uow: &mut dyn TransactionContext, plan: &ReturnUndoPlan) -> Result<(), AppError> {
        (**self).apply_return_undo(uow, plan)
    }
}
