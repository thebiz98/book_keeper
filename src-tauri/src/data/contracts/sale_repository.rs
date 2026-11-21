use crate::core::dtos::{PaginatedList, SalesFilter};
use crate::core::entities::{SaleItem, SaleTransaction};
use crate::core::error::AppError;
use crate::core::value_objects::PaymentStatus;

/// Methods taking `&mut self` write and need a `begin_transaction()` context; `&self` methods
/// are valid on a `begin_read()` context too.
pub trait SaleRepository {
    fn create_transaction(&mut self, tx: &SaleTransaction) -> Result<i64, AppError>;
    fn create_sale_item(&mut self, line: &SaleItem) -> Result<i64, AppError>;
    fn find_transaction_by_id(&self, id: i64) -> Result<Option<SaleTransaction>, AppError>;
    fn find_sale_item_by_id(&self, id: i64) -> Result<Option<SaleItem>, AppError>;
    /// Next `SAL-` code. Call inside the write transaction that inserts the sale; from a read
    /// context it is a display preview.
    fn get_next_transaction_code(&self) -> Result<String, AppError>;
    /// Writes the cached money fields only; called exclusively by `PaymentService`.
    fn update_money_fields(
        &mut self,
        tx_id: i64,
        paid_amount: f64,
        balance_due: f64,
        status: PaymentStatus,
    ) -> Result<(), AppError>;
    /// Open bills for FIFO settlement / credit: `balance_due > 0`, `ORDER BY transaction_date, id`.
    fn list_open_for_party(&self, party_id: i64) -> Result<Vec<SaleTransaction>, AppError>;
    fn list_sales(
        &self,
        start_date: Option<&str>,
        end_date: Option<&str>,
        status: Option<PaymentStatus>,
        party_id: Option<i64>,
        offset: usize,
        limit: usize,
    ) -> Result<Vec<SaleTransaction>, AppError>;
    fn list_transactions(&self, filter: &SalesFilter) -> Result<PaginatedList<SaleTransaction>, AppError>;
    fn list_items_by_transaction_id(&self, tx_id: i64) -> Result<Vec<SaleItem>, AppError>;
    /// Σ quantity of customer `RETURNED` return lines referencing this sale line.
    fn get_returned_quantity_for_line(&self, sale_item_id: i64) -> Result<f64, AppError>;
}
