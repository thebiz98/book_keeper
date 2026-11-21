use crate::core::entities::SaleStockAllocation;
use crate::core::error::AppError;

pub trait AllocationRepository {
    fn create_allocation(&mut self, allocation: &SaleStockAllocation) -> Result<i64, AppError>;
    fn find_by_sale_item_id(&self, sale_item_id: i64) -> Result<Vec<SaleStockAllocation>, AppError>;
    /// Allocations with `quantity_depleted > returned_quantity`. Order is not part of the contract;
    /// the strategy sorts them into its reversal order.
    fn find_reversible_by_sale_item_id(&self, sale_item_id: i64) -> Result<Vec<SaleStockAllocation>, AppError>;
    /// Sets `returned_quantity`; rejects values < 0 or > `quantity_depleted` (1e-9 tolerance).
    fn update_returned_quantity(&mut self, allocation_id: i64, new_returned_qty: f64) -> Result<(), AppError>;
    /// Every allocation (report COGS; F5 replaces this with SQL aggregates).
    fn list_all(&self) -> Result<Vec<SaleStockAllocation>, AppError>;
}
