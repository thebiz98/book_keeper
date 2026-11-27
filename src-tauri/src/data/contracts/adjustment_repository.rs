use crate::core::entities::StockAdjustment;
use crate::core::dtos::AdjustmentListRow;
use crate::core::error::AppError;

pub trait AdjustmentRepository {
    fn create_adjustment(&mut self, adj: &StockAdjustment) -> Result<i64, AppError>;
    /// FR-25: rewrites the stored lot cost of a positive adjustment (no-op when the id is absent).
    fn update_unit_cost(&mut self, id: i64, unit_cost: f64) -> Result<(), AppError>;
    fn find_by_id(&self, id: i64) -> Result<Option<StockAdjustment>, AppError>;
    /// FR-52: removes the adjustment row (its lot / allocations are handled by the service).
    fn delete_adjustment(&mut self, id: i64) -> Result<(), AppError>;
    /// One page, newest first, plus the total number of matching rows (both in SQL).
    fn list_adjustments(
        &self,
        item_id: Option<i64>,
        offset: usize,
        limit: usize,
    ) -> Result<(Vec<StockAdjustment>, usize), AppError>;
    /// Same paging as `list_adjustments`, with the item name/unit and the value (loss for a
    /// reduction = FIFO cost from `stock_outflow_allocations`; qty x unit cost for an increase).
    fn list_adjustment_rows(
        &self,
        item_id: Option<i64>,
        offset: usize,
        limit: usize,
    ) -> Result<(Vec<AdjustmentListRow>, usize), AppError>;
}
