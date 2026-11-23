use crate::core::entities::StockAdjustment;
use crate::core::error::AppError;

pub trait AdjustmentRepository {
    fn create_adjustment(&mut self, adj: &StockAdjustment) -> Result<i64, AppError>;
    /// FR-25: rewrites the stored lot cost of a positive adjustment (no-op when the id is absent).
    fn update_unit_cost(&mut self, id: i64, unit_cost: f64) -> Result<(), AppError>;
    /// One page, newest first, plus the total number of matching rows (both in SQL).
    fn list_adjustments(
        &self,
        item_id: Option<i64>,
        offset: usize,
        limit: usize,
    ) -> Result<(Vec<StockAdjustment>, usize), AppError>;
}
