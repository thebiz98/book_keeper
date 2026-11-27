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
    /// DD-89 (change-only edit): sets `quantity_depleted`; rejects a value that is not finite, is
    /// <= 0 (delete the row instead) or is below `returned_quantity`.
    fn update_quantity_depleted(&mut self, allocation_id: i64, quantity: f64) -> Result<(), AppError>;
    /// DD-89: removes one allocation row.
    fn delete_allocation(&mut self, allocation_id: i64) -> Result<(), AppError>;
    /// FR-20: removes every allocation of a sale line; returns the count.
    fn delete_by_sale_item_id(&mut self, sale_item_id: i64) -> Result<usize, AppError>;
    /// DD-95: Σ `quantity_depleted` of every sale allocation on the lot (returned units included).
    fn sum_depleted_for_lot(&self, lot_id: i64) -> Result<f64, AppError>;
    /// Distinct sale lines that drew from the lot.
    fn sale_item_ids_for_lot(&self, lot_id: i64) -> Result<Vec<i64>, AppError>;
    /// FR-20 re-cost: sets `unit_cost` on every allocation of the lot.
    fn update_unit_cost_for_lot(&mut self, lot_id: i64, unit_cost: f64) -> Result<usize, AppError>;
}
