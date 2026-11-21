use crate::core::entities::{StockOutflowAllocation, SaleStockAllocation};
use crate::core::error::AppError;
use crate::core::value_objects::OutflowSourceType;
use crate::data::contracts::unit_of_work::TransactionContext;
use crate::services::contracts::inventory_allocation_strategy::{
    AdjustmentDepletionResult, DepletionResult, InventoryAllocationStrategy,
};

#[derive(Clone, Copy, Debug, Default)]
pub struct FifoStrategy;

impl InventoryAllocationStrategy for FifoStrategy {
    fn validate_stock_availability(
        &self,
        uow: &mut dyn TransactionContext,
        item_id: i64,
        required_base_qty: f64,
    ) -> Result<(), AppError> {
        let lots = uow.purchases().get_active_lots_for_item(item_id)?;
        let total_available: f64 = lots.iter().map(|l| l.remaining_quantity).sum();
        if total_available < required_base_qty {
            let item_name = uow
                .items()
                .find_by_id(item_id)?
                .map(|i| i.name)
                .unwrap_or_else(|| "Unknown Item".to_string());
            return Err(AppError::InsufficientStockError {
                item_id,
                item_name,
                required: required_base_qty,
                available: total_available,
            });
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
        self.validate_stock_availability(uow, item_id, required_base_qty)?;
        let lots = uow.purchases().get_active_lots_for_item(item_id)?;
        let mut remaining_needed = required_base_qty;
        let mut total_cogs = 0.0;
        let mut allocations = Vec::new();
        for lot in lots {
            if remaining_needed <= 0.0 {
                break;
            }
            let take = lot.remaining_quantity.min(remaining_needed);
            let new_lot_remaining = lot.remaining_quantity - take;
            uow.purchases()
                .update_remaining_quantity(lot.id, new_lot_remaining)?;
            // TODO(F4, DD-05): restore_lots_for_return must bump returned_quantity.
            let allocation = SaleStockAllocation {
                id: 0,
                sale_item_id,
                purchase_item_id: lot.id,
                quantity_depleted: take,
                returned_quantity: 0.0,
                unit_cost: lot.unit_cost,
            };
            let alloc_id = uow.allocations().create_allocation(&allocation)?;
            let mut saved = allocation;
            saved.id = alloc_id;
            allocations.push(saved);
            total_cogs += take * lot.unit_cost;
            remaining_needed -= take;
        }
        Ok(DepletionResult {
            total_cogs,
            allocations,
        })
    }

    fn restore_lots_for_return(
        &self,
        uow: &mut dyn TransactionContext,
        sale_item_id: i64,
        quantity_to_restore: f64,
    ) -> Result<(), AppError> {
        if quantity_to_restore <= 0.0 {
            return Err(AppError::ValidationError {
                field: "quantity_to_restore".to_string(),
                message: "Restoration quantity must be strictly positive".to_string(),
            });
        }

        let mut allocs = uow.allocations().find_by_sale_item_id(sale_item_id)?;
        allocs.sort_by_key(|a| std::cmp::Reverse(a.id)); // Reverse FIFO order: latest depleted first
        let mut remaining_to_restore = quantity_to_restore;

        for alloc in allocs {
            if remaining_to_restore <= 0.0 {
                break;
            }
            let restore_amount = alloc.quantity_depleted.min(remaining_to_restore);

            let lot = uow
                .purchases()
                .find_purchase_item_by_id(alloc.purchase_item_id)?
                .ok_or_else(|| AppError::NotFound {
                    entity: "PurchaseItem".to_string(),
                    id: alloc.purchase_item_id.to_string(),
                })?;

            let new_remaining = lot.remaining_quantity + restore_amount;
            if new_remaining > lot.quantity {
                return Err(AppError::ValidationError {
                    field: "remaining_quantity".to_string(),
                    message: format!(
                        "Restoration of {} units exceeds lot #{} capacity of {} (current: {})",
                        restore_amount, lot.id, lot.quantity, lot.remaining_quantity
                    ),
                });
            }

            uow.purchases()
                .update_remaining_quantity(lot.id, new_remaining)?;
            remaining_to_restore -= restore_amount;
        }

        if remaining_to_restore > 0.0 {
            return Err(AppError::ValidationError {
                field: "quantity_to_restore".to_string(),
                message: format!(
                    "Return quantity {} exceeds total units depleted for sale_item_id {}",
                    quantity_to_restore, sale_item_id
                ),
            });
        }

        Ok(())
    }

    fn deplete_lots_for_adjustment(
        &self,
        uow: &mut dyn TransactionContext,
        stock_adjustment_id: i64,
        item_id: i64,
        write_off_qty: f64,
    ) -> Result<AdjustmentDepletionResult, AppError> {
        self.validate_stock_availability(uow, item_id, write_off_qty)?;
        let lots = uow.purchases().get_active_lots_for_item(item_id)?;
        let mut remaining_needed = write_off_qty;
        let mut total_loss_value = 0.0;
        let mut allocations = Vec::new();
        for lot in lots {
            if remaining_needed <= 0.0 {
                break;
            }
            let take = lot.remaining_quantity.min(remaining_needed);
            let new_lot_remaining = lot.remaining_quantity - take;
            uow.purchases()
                .update_remaining_quantity(lot.id, new_lot_remaining)?;
            let alloc = StockOutflowAllocation {
                id: 0,
                source_type: OutflowSourceType::Adjustment,
                source_id: stock_adjustment_id,
                purchase_item_id: lot.id,
                quantity_depleted: take,
                unit_cost: lot.unit_cost,
            };
            let alloc_id = uow.outflow_allocations().create_allocation(&alloc)?;
            let mut saved = alloc;
            saved.id = alloc_id;
            allocations.push(saved);
            total_loss_value += take * lot.unit_cost;
            remaining_needed -= take;
        }
        Ok(AdjustmentDepletionResult {
            total_loss_value,
            allocations,
        })
    }
}
