use crate::core::entities::BillCharge;
use crate::core::error::AppError;
use crate::core::value_objects::TransactionType;

/// Charge/discount lines of sale and purchase bills (FR-19).
pub trait BillChargeRepository {
    fn create(&mut self, c: &BillCharge) -> Result<i64, AppError>;
    /// Lines of one bill, in entry order (`ORDER BY id`).
    fn list_by_transaction(&self, t: TransactionType, id: i64) -> Result<Vec<BillCharge>, AppError>;
    /// Removes every line of one bill (used by bill edit/delete); returns the number removed.
    fn delete_by_transaction(&mut self, t: TransactionType, id: i64) -> Result<usize, AppError>;
}
