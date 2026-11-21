use crate::core::entities::SaleStockAllocation;
use crate::core::error::AppError;

pub trait AllocationRepository {
    fn create_allocation(&mut self, allocation: &SaleStockAllocation) -> Result<i64, AppError> {
        self.create(allocation)
    }
    fn create(&mut self, allocation: &SaleStockAllocation) -> Result<i64, AppError> {
        self.create_allocation(allocation)
    }
    fn find_by_sale_item_id(&self, sale_item_id: i64) -> Result<Vec<SaleStockAllocation>, AppError> {
        self.list_by_sale_item_id(sale_item_id)
    }
    fn list_by_sale_item_id(&self, sale_item_id: i64) -> Result<Vec<SaleStockAllocation>, AppError> {
        self.find_by_sale_item_id(sale_item_id)
    }
    fn list_all(&self) -> Result<Vec<SaleStockAllocation>, AppError> {
        Ok(Vec::new())
    }
}
