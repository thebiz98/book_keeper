use crate::core::entities::StockOutflowAllocation;
use crate::core::error::AppError;
use crate::core::value_objects::OutflowSourceType;

/// FIFO audit rows for non-sale stock outflows (adjustments, replacements, vendor returns).
pub trait OutflowAllocationRepository {
    fn create_allocation(&mut self, alloc: &StockOutflowAllocation) -> Result<i64, AppError>;
    fn create(&mut self, alloc: &StockOutflowAllocation) -> Result<i64, AppError> {
        self.create_allocation(alloc)
    }
    fn find_by_source(
        &self,
        source_type: OutflowSourceType,
        source_id: i64,
    ) -> Result<Vec<StockOutflowAllocation>, AppError>;
}
