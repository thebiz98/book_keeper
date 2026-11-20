use crate::core::entities::{AdjustmentStockAllocation, SaleStockAllocation};
use crate::core::error::AppError;
use crate::data::contracts::unit_of_work::TransactionContext;

#[derive(Debug, Clone, PartialEq)]
pub struct DepletionResult {
    pub total_cogs: f64,
    pub allocations: Vec<SaleStockAllocation>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AdjustmentDepletionResult {
    pub total_loss_value: f64,
    pub allocations: Vec<AdjustmentStockAllocation>,
}

/// Strategy interface for inventory lot allocation algorithms.
pub trait InventoryAllocationStrategy: Send + Sync {
    /// Validates that total active stock across lots >= required_base_qty.
    fn validate_stock_availability(
        &self,
        uow: &mut dyn TransactionContext,
        item_id: i64,
        required_base_qty: f64,
    ) -> Result<(), AppError>;

    /// Depletes lots in strategy-defined order (FIFO = oldest first).
    fn deplete_lots_for_sale(
        &self,
        uow: &mut dyn TransactionContext,
        sale_item_id: i64,
        item_id: i64,
        required_base_qty: f64,
    ) -> Result<DepletionResult, AppError>;

    /// Restores lots in reverse strategy order for customer returns.
    fn restore_lots_for_return(
        &self,
        uow: &mut dyn TransactionContext,
        sale_item_id: i64,
        quantity_to_restore: f64,
    ) -> Result<(), AppError>;

    /// Depletes lots for negative stock adjustments (damaged/expired write-offs).
    fn deplete_lots_for_adjustment(
        &self,
        uow: &mut dyn TransactionContext,
        stock_adjustment_id: i64,
        item_id: i64,
        write_off_qty: f64,
    ) -> Result<AdjustmentDepletionResult, AppError>;
}
