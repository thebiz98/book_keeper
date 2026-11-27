use crate::core::constants::EPS;
use crate::core::entities::{PurchaseItem, SaleStockAllocation, StockOutflowAllocation};
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

    /// DD-89 (change-only sale edit): gives back `quantity` units of a sale line that were drawn
    /// from `item_id`'s lots (a quantity cut or a removed line, never a customer return). Walks the
    /// line's allocations on that item's lots latest-depleted first (the order of
    /// `reverse_sale_allocations`), **shrinks `quantity_depleted`** (deleting a row that reaches 0)
    /// and puts the units back on the lot. `returned_quantity` is never touched: a line with linked
    /// returns cannot be edited (`LINKED_RETURNS`), so it is 0 here, and a non-zero one is refused
    /// (`STOCK_INVARIANT`). More than the line drew from the item is a `ValidationError`.
    fn release_sale_allocations(
        &self,
        uow: &mut dyn TransactionContext,
        sale_item_id: i64,
        item_id: i64,
        quantity: f64,
    ) -> Result<ReversalResult, AppError> {
        if !quantity.is_finite() || quantity <= 0.0 {
            return Err(AppError::ValidationError {
                field: "quantity".to_string(),
                message: format!("Quantity must be a finite number greater than zero (got {quantity})"),
            });
        }
        let mut allocs: Vec<(SaleStockAllocation, PurchaseItem)> = Vec::new();
        for a in uow.allocations().find_by_sale_item_id(sale_item_id)? {
            let lot = uow
                .purchases()
                .find_purchase_item_by_id(a.purchase_item_id)?
                .ok_or_else(|| AppError::not_found("PurchaseItem", a.purchase_item_id.to_string()))?;
            if lot.item_id != item_id {
                continue;
            }
            if a.returned_quantity > EPS {
                return Err(AppError::BillLocked {
                    code: "STOCK_INVARIANT".to_string(),
                    message: format!("Sale line #{sale_item_id} has returned units, so its stock cannot be released"),
                });
            }
            allocs.push((a, lot));
        }
        allocs.sort_by_key(|(a, _)| std::cmp::Reverse(a.id)); // latest depleted first
        let drawn: f64 = allocs.iter().map(|(a, _)| a.quantity_depleted).sum();
        if drawn + EPS < quantity {
            return Err(AppError::ValidationError {
                field: "quantity".to_string(),
                message: format!(
                    "Cannot release {quantity}: sale line #{sale_item_id} drew only {drawn} of item #{item_id}"
                ),
            });
        }
        let mut left = quantity;
        let (mut reversed_cost, mut reversed_quantity) = (0.0, 0.0);
        for (a, lot) in allocs {
            if left <= EPS {
                break;
            }
            // Within EPS of the whole row: release it all, so no dust row stays behind.
            let take = if left + EPS >= a.quantity_depleted { a.quantity_depleted } else { left };
            let keep = a.quantity_depleted - take;
            if keep <= EPS {
                uow.allocations().delete_allocation(a.id)?;
            } else {
                uow.allocations().update_quantity_depleted(a.id, keep)?;
            }
            // Re-read: two rows of the line may sit on one lot. The repo refuses anything above the
            // lot's own quantity (no clamp).
            let remaining = uow
                .purchases()
                .find_purchase_item_by_id(lot.id)?
                .ok_or_else(|| AppError::not_found("PurchaseItem", lot.id.to_string()))?
                .remaining_quantity;
            uow.purchases().update_remaining_quantity(lot.id, remaining + take)?;
            reversed_cost += take * a.unit_cost;
            reversed_quantity += take;
            left -= take;
        }
        Ok(ReversalResult { reversed_cost, reversed_quantity, restocked_quantity: reversed_quantity })
    }

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

    /// FR-31: lots with `remaining_quantity > EPS` in the order this strategy would deplete them
    /// (FIFO = oldest first). Read-only.
    fn open_lots_in_draw_order(
        &self,
        uow: &mut dyn TransactionContext,
        item_id: i64,
    ) -> Result<Vec<PurchaseItem>, AppError>;

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

    fn release_sale_allocations(
        &self,
        uow: &mut dyn TransactionContext,
        sale_item_id: i64,
        item_id: i64,
        quantity: f64,
    ) -> Result<ReversalResult, AppError> {
        (**self).release_sale_allocations(uow, sale_item_id, item_id, quantity)
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

    fn open_lots_in_draw_order(
        &self,
        uow: &mut dyn TransactionContext,
        item_id: i64,
    ) -> Result<Vec<PurchaseItem>, AppError> {
        (**self).open_lots_in_draw_order(uow, item_id)
    }

    fn replay_reversals(&self, allocs: &[SaleStockAllocation], qtys: &[f64]) -> Vec<Vec<(i64, f64)>> {
        (**self).replay_reversals(allocs, qtys)
    }
}
