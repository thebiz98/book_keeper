use crate::core::constants::EPS;
use crate::core::entities::{PurchaseItem, SaleStockAllocation, StockOutflowAllocation};
use crate::core::error::AppError;
use crate::core::value_objects::OutflowSourceType;
use crate::data::contracts::unit_of_work::TransactionContext;
use crate::services::contracts::inventory_allocation_strategy::{
    DepletionResult, InventoryAllocationStrategy, OutflowDepletionResult, ReversalResult,
};

/// FIFO (first-in, first-out): depletes the oldest lot first (lowest `purchase_item.id`, any lot
/// source) and reverses the latest-depleted allocation first (highest allocation id).
///
/// The ordering lives here and only here: repositories return lots/allocations in an
/// unspecified order and this strategy sorts them.
#[derive(Clone, Copy, Debug, Default)]
pub struct FifoStrategy;

fn require_positive(field: &str, qty: f64) -> Result<(), AppError> {
    if !qty.is_finite() || qty <= 0.0 {
        return Err(AppError::ValidationError {
            field: field.to_string(),
            message: format!("Quantity must be a finite number greater than zero (got {qty})"),
        });
    }
    Ok(())
}

impl FifoStrategy {
    /// Active lots, oldest first, plus their total remaining quantity. Read-only.
    fn active_lots_oldest_first(
        uow: &mut dyn TransactionContext,
        item_id: i64,
    ) -> Result<(Vec<PurchaseItem>, f64), AppError> {
        let mut lots = uow.purchases().get_active_lots_for_item(item_id)?;
        lots.sort_by_key(|l| l.id);
        let total = lots.iter().map(|l| l.remaining_quantity).sum();
        Ok((lots, total))
    }

    fn insufficient(
        uow: &mut dyn TransactionContext,
        item_id: i64,
        required: f64,
        available: f64,
    ) -> AppError {
        let item_name = uow
            .items()
            .find_by_id(item_id)
            .ok()
            .flatten()
            .map(|i| i.name)
            .unwrap_or_else(|| "Unknown Item".to_string());
        AppError::InsufficientStockError { item_id, item_name, required, available }
    }

    /// Shared validated walk: returns `(lot, take)` pairs covering `qty`, mutating nothing.
    fn plan_depletion(
        uow: &mut dyn TransactionContext,
        item_id: i64,
        qty: f64,
    ) -> Result<Vec<(PurchaseItem, f64)>, AppError> {
        require_positive("quantity", qty)?;
        let (lots, total) = Self::active_lots_oldest_first(uow, item_id)?;
        if total + EPS < qty {
            return Err(Self::insufficient(uow, item_id, qty, total));
        }
        let mut needed = qty;
        let mut plan = Vec::new();
        for lot in lots {
            if needed <= EPS {
                break;
            }
            let take = lot.remaining_quantity.min(needed);
            needed -= take;
            plan.push((lot, take));
        }
        Ok(plan)
    }
}

impl InventoryAllocationStrategy for FifoStrategy {
    fn open_lots_in_draw_order(
        &self,
        uow: &mut dyn TransactionContext,
        item_id: i64,
    ) -> Result<Vec<PurchaseItem>, AppError> {
        let (mut lots, _) = Self::active_lots_oldest_first(uow, item_id)?;
        lots.retain(|l| l.remaining_quantity > EPS);
        Ok(lots)
    }

    fn validate_stock_availability(
        &self,
        uow: &mut dyn TransactionContext,
        item_id: i64,
        required_base_qty: f64,
    ) -> Result<(), AppError> {
        require_positive("quantity", required_base_qty)?;
        let (_, total) = Self::active_lots_oldest_first(uow, item_id)?;
        if total + EPS < required_base_qty {
            return Err(Self::insufficient(uow, item_id, required_base_qty, total));
        }
        Ok(())
    }

    fn deplete_lots_for_sale(
        &self,
        uow: &mut dyn TransactionContext,
        sale_item_id: i64,
        item_id: i64,
        required_base_qty: f64,
    ) -> Result<DepletionResult, AppError> {
        let plan = Self::plan_depletion(uow, item_id, required_base_qty)?;
        let mut total_cogs = 0.0;
        let mut allocations = Vec::new();
        for (lot, take) in plan {
            uow.purchases()
                .update_remaining_quantity(lot.id, lot.remaining_quantity - take)?;
            let mut alloc = SaleStockAllocation {
                id: 0,
                sale_item_id,
                purchase_item_id: lot.id,
                quantity_depleted: take,
                returned_quantity: 0.0,
                unit_cost: lot.unit_cost,
            };
            alloc.id = uow.allocations().create_allocation(&alloc)?;
            total_cogs += take * lot.unit_cost;
            allocations.push(alloc);
        }
        Ok(DepletionResult { total_cogs, allocations })
    }

    fn reverse_sale_allocations(
        &self,
        uow: &mut dyn TransactionContext,
        sale_item_id: i64,
        quantity: f64,
        restock: bool,
    ) -> Result<ReversalResult, AppError> {
        require_positive("quantity", quantity)?;
        let mut allocs = uow.allocations().find_reversible_by_sale_item_id(sale_item_id)?;
        allocs.sort_by_key(|a| std::cmp::Reverse(a.id)); // latest depleted first
        let reversible: f64 = allocs
            .iter()
            .map(|a| a.quantity_depleted - a.returned_quantity)
            .sum();
        if reversible + EPS < quantity {
            return Err(AppError::ValidationError {
                field: "quantity".to_string(),
                message: format!(
                    "Return exceeds sold quantity: {quantity} requested, {reversible} still returnable for sale line #{sale_item_id}"
                ),
            });
        }

        let mut left = quantity;
        let mut reversed_cost = 0.0;
        let mut reversed_quantity = 0.0;
        let mut restocked_quantity = 0.0;
        for a in allocs {
            if left <= EPS {
                break;
            }
            let take = (a.quantity_depleted - a.returned_quantity).min(left);
            uow.allocations()
                .update_returned_quantity(a.id, a.returned_quantity + take)?;
            if restock {
                let lot = uow
                    .purchases()
                    .find_purchase_item_by_id(a.purchase_item_id)?
                    .ok_or_else(|| AppError::not_found("PurchaseItem", a.purchase_item_id.to_string()))?;
                // The repo rejects anything above the lot's original quantity.
                uow.purchases()
                    .update_remaining_quantity(lot.id, lot.remaining_quantity + take)?;
                restocked_quantity += take;
            }
            reversed_cost += take * a.unit_cost;
            reversed_quantity += take;
            left -= take;
        }
        Ok(ReversalResult { reversed_cost, reversed_quantity, restocked_quantity })
    }

    fn deplete_lots_for_outflow(
        &self,
        uow: &mut dyn TransactionContext,
        source_type: OutflowSourceType,
        source_id: i64,
        item_id: i64,
        quantity: f64,
    ) -> Result<OutflowDepletionResult, AppError> {
        // DD-86: a vendor return always names its lot; FIFO is for ADJUSTMENT and REPLACEMENT.
        debug_assert!(source_type != OutflowSourceType::VendorReturn, "vendor returns never draw FIFO");
        let plan = Self::plan_depletion(uow, item_id, quantity)?;
        let mut total_cost = 0.0;
        let mut allocations = Vec::new();
        for (lot, take) in plan {
            uow.purchases()
                .update_remaining_quantity(lot.id, lot.remaining_quantity - take)?;
            let mut alloc = StockOutflowAllocation {
                id: 0,
                source_type,
                source_id,
                purchase_item_id: lot.id,
                quantity_depleted: take,
                unit_cost: lot.unit_cost,
            };
            alloc.id = uow.outflow_allocations().create_allocation(&alloc)?;
            total_cost += take * lot.unit_cost;
            allocations.push(alloc);
        }
        Ok(OutflowDepletionResult { total_cost, allocations })
    }
}
