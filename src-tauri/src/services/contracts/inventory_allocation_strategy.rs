use crate::core::constants::EPS;
use crate::core::entities::{SaleStockAllocation, StockOutflowAllocation};
use crate::core::error::AppError;
use crate::core::value_objects::OutflowSourceType;
use crate::data::contracts::unit_of_work::TransactionContext;

#[derive(Debug, Clone, PartialEq)]
pub struct DepletionResult {
    pub total_cogs: f64,
    pub allocations: Vec<SaleStockAllocation>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OutflowDepletionResult {
    /// Σ qty × lot cost (loss value / vendor-return value).
    pub total_cost: f64,
    pub allocations: Vec<StockOutflowAllocation>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReversalResult {
    /// Σ reversed qty × allocation unit_cost.
    pub reversed_cost: f64,
    /// Units whose sale allocation was reversed (== requested quantity within EPS).
    pub reversed_quantity: f64,
    /// Units actually added back to lot `remaining_quantity` (0 when `restock` is false).
    /// `InventoryService` applies exactly this to the stock cache.
    pub restocked_quantity: f64,
}

/// Strategy interface for inventory lot allocation algorithms (NFR-05).
///
/// Implementations are pure algorithms over lots and allocation rows through the
/// `TransactionContext` repositories. They own lot ordering (repositories return lots and
/// allocations in an unspecified order), never touch the item stock cache, and validate before
/// mutating. Every quantity must be finite and > 0, else `ValidationError`. All tolerances are
/// [`crate::core::EPS`].
pub trait InventoryAllocationStrategy: Send + Sync {
    /// Total active stock across lots + EPS >= `required_base_qty`, else `InsufficientStockError`.
    fn validate_stock_availability(
        &self,
        uow: &mut dyn TransactionContext,
        item_id: i64,
        required_base_qty: f64,
    ) -> Result<(), AppError>;

    /// Depletes lots in strategy-defined order (FIFO = oldest first), decrementing lot
    /// `remaining_quantity` and logging `SaleStockAllocation` rows.
    fn deplete_lots_for_sale(
        &self,
        uow: &mut dyn TransactionContext,
        sale_item_id: i64,
        item_id: i64,
        required_base_qty: f64,
    ) -> Result<DepletionResult, AppError>;

    /// Reverses `quantity` units of a sale line (customer return). Capacity per allocation is
    /// `quantity_depleted - returned_quantity`, walked latest-depleted first; increments
    /// `returned_quantity` and, when `restock`, adds the units back to the lot.
    fn reverse_sale_allocations(
        &self,
        uow: &mut dyn TransactionContext,
        sale_item_id: i64,
        quantity: f64,
        restock: bool,
    ) -> Result<ReversalResult, AppError>;

    /// Depletes lots for a non-sale outflow (negative adjustment, replacement issue, vendor
    /// return) and logs `StockOutflowAllocation` rows tagged `(source_type, source_id)`.
    fn deplete_lots_for_outflow(
        &self,
        uow: &mut dyn TransactionContext,
        source_type: OutflowSourceType,
        source_id: i64,
        item_id: i64,
        quantity: f64,
    ) -> Result<OutflowDepletionResult, AppError>;

    /// FR-20: pure replay of the customer-return reversal walk. `allocs` are a sale line's
    /// allocations (any order), `qtys` the quantities of its `RETURNED` lines in id order. Returns,
    /// per quantity, the `(allocation_id, take)` pairs the real walk took (latest allocation id
    /// first, capacity `quantity_depleted`). Valid while allocations are immutable and returns are
    /// deleted newest first; used to undo a return and to recompute `RETURN_ITEM.cost_amount`.
    fn replay_reversals(&self, allocs: &[SaleStockAllocation], qtys: &[f64]) -> Vec<Vec<(i64, f64)>> {
        let mut caps: Vec<(i64, f64)> = allocs.iter().map(|a| (a.id, a.quantity_depleted)).collect();
        caps.sort_by_key(|(id, _)| std::cmp::Reverse(*id));
        let mut out = Vec::with_capacity(qtys.len());
        for &q in qtys {
            let mut left = q;
            let mut takes = Vec::new();
            for (id, cap) in caps.iter_mut() {
                if left <= EPS {
                    break;
                }
                let take = cap.min(left);
                if take > EPS {
                    *cap -= take;
                    left -= take;
                    takes.push((*id, take));
                }
            }
            out.push(takes);
        }
        out
    }
}

impl<T: InventoryAllocationStrategy + ?Sized> InventoryAllocationStrategy for std::sync::Arc<T> {
    fn validate_stock_availability(
        &self,
        uow: &mut dyn TransactionContext,
        item_id: i64,
        required_base_qty: f64,
    ) -> Result<(), AppError> {
        (**self).validate_stock_availability(uow, item_id, required_base_qty)
    }

    fn deplete_lots_for_sale(
        &self,
        uow: &mut dyn TransactionContext,
        sale_item_id: i64,
        item_id: i64,
        required_base_qty: f64,
    ) -> Result<DepletionResult, AppError> {
        (**self).deplete_lots_for_sale(uow, sale_item_id, item_id, required_base_qty)
    }

    fn reverse_sale_allocations(
        &self,
        uow: &mut dyn TransactionContext,
        sale_item_id: i64,
        quantity: f64,
        restock: bool,
    ) -> Result<ReversalResult, AppError> {
        (**self).reverse_sale_allocations(uow, sale_item_id, quantity, restock)
    }

    fn deplete_lots_for_outflow(
        &self,
        uow: &mut dyn TransactionContext,
        source_type: OutflowSourceType,
        source_id: i64,
        item_id: i64,
        quantity: f64,
    ) -> Result<OutflowDepletionResult, AppError> {
        (**self).deplete_lots_for_outflow(uow, source_type, source_id, item_id, quantity)
    }

    fn replay_reversals(&self, allocs: &[SaleStockAllocation], qtys: &[f64]) -> Vec<Vec<(i64, f64)>> {
        (**self).replay_reversals(allocs, qtys)
    }
}
