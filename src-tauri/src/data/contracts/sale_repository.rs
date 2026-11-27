use crate::core::dtos::{BillProfitParts, ItemBillRef, SaleHistoryRow, SalesHistoryFilter};
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
    /// Next `OB-` code (FR-35); call inside the write transaction that inserts the bill.
    fn next_opening_code(&self) -> Result<String, AppError>;
    /// The customer's opening-balance bill (FR-35), if any.
    fn find_opening_for_party(&self, party_id: i64) -> Result<Option<SaleTransaction>, AppError>;
    fn list_sales(
        &self,
        start_date: Option<&str>,
        end_date: Option<&str>,
        status: Option<PaymentStatus>,
        party_id: Option<i64>,
        offset: usize,
        limit: usize,
    ) -> Result<Vec<SaleTransaction>, AppError>;
    /// History page: filtered, newest first (`transaction_date DESC, id DESC`), paged in SQL, plus
    /// the total matching count (same `WHERE`). Party name is the header snapshot.
    fn list_history(
        &self,
        filter: &SalesHistoryFilter,
        offset: usize,
        limit: usize,
    ) -> Result<(Vec<SaleHistoryRow>, usize), AppError>;
    /// `(sale_item_id, returned base qty)` for the bill's lines that have customer returns, one query.
    fn get_returned_quantities_for_sale(&self, sale_id: i64) -> Result<Vec<(i64, f64)>, AppError>;
    fn list_items_by_transaction_id(&self, tx_id: i64) -> Result<Vec<SaleItem>, AppError>;
    /// Σ quantity of customer `RETURNED` return lines referencing this sale line.
    fn get_returned_quantity_for_line(&self, sale_item_id: i64) -> Result<f64, AppError>;
    /// FR-34: the components of one bill's profit, from the same SQL as the history `profit` column.
    fn bill_profit_parts(&self, sale_id: i64) -> Result<BillProfitParts, AppError>;
    /// FR-37: miscellaneous service cost per line of one bill, `(sale_item_id, misc)`, from the shared
    /// `misc_cost_sql` fragment (physical lines give 0).
    fn list_line_misc_costs(&self, sale_id: i64) -> Result<Vec<(i64, f64)>, AppError>;

    /// FR-23: distinct bills containing the item, `transaction_date DESC, id DESC`, paged in SQL,
    /// plus the distinct bill count. Served by `idx_sale_items_item_date`.
    fn list_bills_for_item(&self, item_id: i64, offset: usize, limit: usize) -> Result<(Vec<ItemBillRef>, usize), AppError>;

    /// FR-20: rewrites date, party (+ name snapshot), mode, notes, subtotal/charges/discounts,
    /// total and `updated_at`. Never touches the code, `created_at` or the cached money fields
    /// (`PaymentService` recomputes those).
    fn update_header_for_edit(&mut self, h: &SaleTransaction) -> Result<(), AppError>;
    /// DD-89 (change-only edit): rewrites one line's item, snapshots, quantities and price fields
    /// in place (id, bill and line date kept). The service decides which fields change.
    fn update_sale_item(&mut self, line: &SaleItem) -> Result<(), AppError>;
    /// DD-89: removes one line (its allocations must be gone first).
    fn delete_sale_item(&mut self, id: i64) -> Result<(), AppError>;
    /// FR-20: removes the bill's lines (allocations must be gone first); returns the count.
    fn delete_sale_items(&mut self, sale_id: i64) -> Result<usize, AppError>;
    /// FR-20: removes the header (lines, charges and ledger must be gone first).
    fn delete_transaction(&mut self, id: i64) -> Result<(), AppError>;
}
