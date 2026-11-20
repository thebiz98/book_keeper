use crate::core::entities::AdjustmentStockAllocation;
use crate::core::error::AppError;

pub trait AdjustmentAllocationRepository {
    fn create_allocation(&mut self, alloc: &AdjustmentStockAllocation) -> Result<i64, AppError> {
        self.create(alloc)
    }
    fn create(&mut self, alloc: &AdjustmentStockAllocation) -> Result<i64, AppError> {
        self.create_allocation(alloc)
    }
    fn find_by_adjustment_id(&self, adjustment_id: i64) -> Result<Vec<AdjustmentStockAllocation>, AppError> {
        self.list_by_adjustment_id(adjustment_id)
    }
    fn list_by_adjustment_id(&self, adj_id: i64) -> Result<Vec<AdjustmentStockAllocation>, AppError> {
        self.find_by_adjustment_id(adj_id)
    }
}
