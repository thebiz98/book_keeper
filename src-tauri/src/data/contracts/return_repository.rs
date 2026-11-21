use crate::core::entities::{ReturnItem, ReturnTransaction};
use crate::core::error::AppError;
use crate::core::value_objects::ReturnType;

pub trait ReturnRepository {
    fn create_return_transaction(&mut self, tx: &ReturnTransaction) -> Result<i64, AppError>;
    fn create_return_item(&mut self, item: &ReturnItem) -> Result<i64, AppError>;
    fn update_return_item_cost(&mut self, return_item_id: i64, cost_amount: f64) -> Result<(), AppError>;
    fn find_return_by_id(&self, id: i64) -> Result<Option<ReturnTransaction>, AppError>;
    fn find_items_by_return_id(&self, return_id: i64) -> Result<Vec<ReturnItem>, AppError>;
    /// Next `RET-` code, one sequence shared by customer and vendor returns. Call inside the
    /// write transaction that inserts the return; from a read context it is a display preview.
    fn get_next_return_code(&self) -> Result<String, AppError>;
    fn list_returns(
        &self,
        return_type: Option<ReturnType>,
        offset: usize,
        limit: usize,
    ) -> Result<Vec<ReturnTransaction>, AppError>;
}
