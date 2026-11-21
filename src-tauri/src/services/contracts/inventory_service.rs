use crate::core::dtos::{PaginatedList, StockAdjustmentPayload};
use crate::core::entities::{PurchaseItem, StockAdjustment};
use crate::core::error::AppError;
use crate::data::contracts::unit_of_work::TransactionContext;
use super::inventory_allocation_strategy::DepletionResult;

pub trait InventoryService: Send + Sync {
    /// Validates that sufficient stock exists for the given item.
    fn validate_stock_availability(
        &self,
        uow: &mut dyn TransactionContext,
        item_id: i64,
        required_base_qty: f64,
    ) -> Result<(), AppError>;

    /// Depletes lots via the allocation strategy and updates ITEM.current_stock.
    fn deplete_stock_for_sale(
        &self,
        uow: &mut dyn TransactionContext,
        sale_item_id: i64,
        item_id: i64,
        required_base_qty: f64,
    ) -> Result<DepletionResult, AppError>;

    /// Creates a new purchase lot and increments ITEM.current_stock.
    fn add_stock_for_purchase(
        &self,
        uow: &mut dyn TransactionContext,
        purchase_item: &PurchaseItem,
    ) -> Result<(), AppError>;

    /// Restores lots via the allocation strategy and increments ITEM.current_stock.
    fn restore_stock_for_return(
        &self,
        uow: &mut dyn TransactionContext,
        sale_item_id: i64,
        item_id: i64,
        quantity_to_restore: f64,
    ) -> Result<(), AppError>;

    /// Depletes lots and decrements ITEM.current_stock for vendor returns.
    fn remove_stock_for_vendor_return(
        &self,
        uow: &mut dyn TransactionContext,
        item_id: i64,
        purchase_item_id: Option<i64>,
        quantity: f64,
    ) -> Result<(), AppError>;

    /// Executes a physical stock adjustment (positive or negative).
    fn create_adjustment(
        &self,
        payload: StockAdjustmentPayload,
    ) -> Result<i64, AppError>;

    /// Lists historical stock adjustments.
    fn list_adjustments(
        &self,
        item_id: Option<i64>,
        page: usize,
        limit: usize,
    ) -> Result<PaginatedList<StockAdjustment>, AppError>;
}

impl<T: InventoryService + ?Sized> InventoryService for std::sync::Arc<T> {
    fn validate_stock_availability(
        &self,
        uow: &mut dyn TransactionContext,
        item_id: i64,
        required_base_qty: f64,
    ) -> Result<(), AppError> {
        (**self).validate_stock_availability(uow, item_id, required_base_qty)
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

    fn restore_stock_for_return(
        &self,
        uow: &mut dyn TransactionContext,
        sale_item_id: i64,
        item_id: i64,
        quantity_to_restore: f64,
    ) -> Result<(), AppError> {
        (**self).restore_stock_for_return(uow, sale_item_id, item_id, quantity_to_restore)
    }

    fn remove_stock_for_vendor_return(
        &self,
        uow: &mut dyn TransactionContext,
        item_id: i64,
        purchase_item_id: Option<i64>,
        quantity: f64,
    ) -> Result<(), AppError> {
        (**self).remove_stock_for_vendor_return(uow, item_id, purchase_item_id, quantity)
    }

    fn create_adjustment(
        &self,
        payload: StockAdjustmentPayload,
    ) -> Result<i64, AppError> {
        (**self).create_adjustment(payload)
    }

    fn list_adjustments(
        &self,
        item_id: Option<i64>,
        page: usize,
        limit: usize,
    ) -> Result<PaginatedList<StockAdjustment>, AppError> {
        (**self).list_adjustments(item_id, page, limit)
    }
}
