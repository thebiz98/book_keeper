use crate::core::dtos::ReturnsFilter;
use crate::core::entities::{PaymentSettlement, ReturnItem, ReturnTransaction};
use crate::core::error::AppError;
use crate::core::value_objects::ReturnType;

pub trait ReturnRepository {
    fn create_return_transaction(&mut self, tx: &ReturnTransaction) -> Result<i64, AppError>;
    fn create_return_item(&mut self, item: &ReturnItem) -> Result<i64, AppError>;
    fn update_return_item_cost(&mut self, return_item_id: i64, cost_amount: f64) -> Result<(), AppError>;
    fn update_return_total(&mut self, return_id: i64, total_amount: f64) -> Result<(), AppError>;
    /// Ledger rows booked by this return (RETURN_CREDIT / REFUND_*), oldest first.
    fn find_ledger_by_return_id(&self, return_id: i64) -> Result<Vec<PaymentSettlement>, AppError>;
    /// Filtered page, newest first (`return_date DESC, id DESC`), paginated in SQL; also returns
    /// the total matching count. `offset`/`limit` are row-based.
    fn list_returns_page(
        &self,
        filter: &ReturnsFilter,
        offset: usize,
        limit: usize,
    ) -> Result<(Vec<ReturnTransaction>, usize), AppError>;
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
