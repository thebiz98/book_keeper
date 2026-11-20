use crate::core::entities::StockAdjustment;
use crate::core::error::AppError;

pub trait AdjustmentRepository {
    fn create_adjustment(&mut self, adj: &StockAdjustment) -> Result<i64, AppError> {
        self.create(adj)
    }
    fn create(&mut self, adj: &StockAdjustment) -> Result<i64, AppError> {
        self.create_adjustment(adj)
    }
    fn list_adjustments(
        &self,
        item_id: Option<i64>,
        offset: usize,
        limit: usize,
    ) -> Result<Vec<StockAdjustment>, AppError> {
        if let Some(id) = item_id {
            let list = self.list_by_item_id(id)?;
            Ok(list.into_iter().skip(offset).take(limit).collect())
        } else {
            Ok(Vec::new())
        }
    }
    fn list_by_item_id(&self, item_id: i64) -> Result<Vec<StockAdjustment>, AppError> {
        self.list_adjustments(Some(item_id), 0, usize::MAX)
    }
}
