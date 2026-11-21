use crate::core::dtos::{ItemLot, PaginatedList, PurchaseFilter};
use crate::core::entities::{PurchaseItem, PurchaseTransaction};
use crate::core::error::AppError;
use crate::core::value_objects::{LotSource, PaymentStatus};

/// Lot headers (`purchase_transactions`, any `source`) and lots (`purchase_items`).
///
/// Methods taking `&mut self` write and need a `begin_transaction()` context; `&self` methods
/// are valid on a `begin_read()` context too.
pub trait PurchaseRepository {
    fn create_transaction(&mut self, tx: &PurchaseTransaction) -> Result<i64, AppError>;
    fn create_purchase_item(&mut self, line: &PurchaseItem) -> Result<i64, AppError>;
    fn find_transaction_by_id(&self, id: i64) -> Result<Option<PurchaseTransaction>, AppError>;
    /// One lot by id (any source). Replaces scanning the purchase list.
    fn find_purchase_item_by_id(&self, id: i64) -> Result<Option<PurchaseItem>, AppError>;
    /// Next code of the source's own sequence (`PUR-`/`ADJ-`/`OPN-`/`RTL-`). Call inside the
    /// write transaction that inserts the header; from a read context it is a display preview.
    fn get_next_transaction_code(&self, source: LotSource) -> Result<String, AppError>;

    /// Lots of any source with `remaining_quantity > 0`. Order is NOT part of the contract (it is
    /// `id` only for determinism); allocation strategies sort the lots themselves.
    fn get_active_lots_for_item(&self, item_id: i64) -> Result<Vec<PurchaseItem>, AppError>;
    /// `unit_cost` of the item's most recent lot (highest id, any source, any remaining).
    fn get_latest_lot_cost(&self, item_id: i64) -> Result<Option<f64>, AppError>;
    /// Lot inspector rows (header code, source, date, vendor), oldest lot first.
    fn list_lots_for_item(&self, item_id: i64) -> Result<Vec<ItemLot>, AppError>;
    /// Every lot (valuation reports; F5 replaces this with SQL aggregates).
    fn list_all_lots(&self) -> Result<Vec<PurchaseItem>, AppError>;

    /// Sets remaining quantity on a lot; rejects values < 0 or > the lot quantity (1e-9 tolerance).
    fn update_remaining_quantity(&mut self, lot_id: i64, new_remaining_qty: f64) -> Result<(), AppError>;
    /// Writes the cached money fields only; called exclusively by `PaymentService`.
    fn update_money_fields(
        &mut self,
        tx_id: i64,
        paid_amount: f64,
        balance_due: f64,
        status: PaymentStatus,
    ) -> Result<(), AppError>;
    /// Open bills for FIFO settlement / credit: `source = PURCHASE`, `balance_due > 0`,
    /// `ORDER BY transaction_date, id`.
    fn list_open_for_party(&self, party_id: i64) -> Result<Vec<PurchaseTransaction>, AppError>;
    /// History list: `source = PURCHASE` only.
    fn list_purchases(
        &self,
        start_date: Option<&str>,
        end_date: Option<&str>,
        status: Option<PaymentStatus>,
        party_id: Option<i64>,
        offset: usize,
        limit: usize,
    ) -> Result<Vec<PurchaseTransaction>, AppError>;
    fn list_transactions(&self, filter: &PurchaseFilter) -> Result<PaginatedList<PurchaseTransaction>, AppError>;
    fn list_items_by_transaction_id(&self, tx_id: i64) -> Result<Vec<PurchaseItem>, AppError>;
    /// Σ quantity of vendor `RETURNED` return lines referencing this lot.
    fn get_returned_quantity_for_lot(&self, purchase_item_id: i64) -> Result<f64, AppError>;
}
