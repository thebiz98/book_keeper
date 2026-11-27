use crate::core::dtos::{ItemBillRef, ItemLot, PurchaseHistoryFilter, PurchaseHistoryRow, VendorReturnLot};
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
    /// `unit_cost` of the item's most recent PURCHASE or OPENING lot (highest id, any remaining),
    /// else of its most recent lot of any source (DD-92, S8).
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
    /// Next `OB-` code (FR-35); call inside the write transaction that inserts the bill.
    fn next_opening_code(&self) -> Result<String, AppError>;
    /// The vendor's opening-balance bill (FR-35), if any.
    fn find_opening_for_party(&self, party_id: i64) -> Result<Option<PurchaseTransaction>, AppError>;
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
    /// History page: `source = PURCHASE` only, filtered, newest first (`transaction_date DESC,
    /// id DESC`), paged in SQL, plus the total matching count (same `WHERE`).
    fn list_history(
        &self,
        filter: &PurchaseHistoryFilter,
        offset: usize,
        limit: usize,
    ) -> Result<(Vec<PurchaseHistoryRow>, usize), AppError>;
    fn list_items_by_transaction_id(&self, tx_id: i64) -> Result<Vec<PurchaseItem>, AppError>;
    /// Σ quantity of vendor `RETURNED` return lines referencing this lot.
    fn get_returned_quantity_for_lot(&self, purchase_item_id: i64) -> Result<f64, AppError>;

    /// FR-23: distinct `source = PURCHASE` bills containing the item, `transaction_date DESC, id DESC`,
    /// paged in SQL, plus the distinct bill count. Served by `idx_purchase_items_item_date`.
    fn list_bills_for_item(&self, item_id: i64, offset: usize, limit: usize) -> Result<(Vec<ItemBillRef>, usize), AppError>;

    /// FR-20: rewrites date, party (+ name snapshot), invoice ref, mode, notes, subtotal/charges/
    /// discounts, total and `updated_at`. Never touches the code, source, `created_at` or the
    /// cached money fields.
    fn update_header_for_edit(&mut self, h: &PurchaseTransaction) -> Result<(), AppError>;
    /// FR-20: rewrites every mutable lot column (the table CHECK enforces `0 <= remaining <= quantity`).
    fn update_purchase_item(&mut self, lot: &PurchaseItem) -> Result<(), AppError>;
    fn delete_purchase_item(&mut self, id: i64) -> Result<(), AppError>;
    /// FR-20: removes the header (lots, charges and ledger must be gone first).
    fn delete_transaction(&mut self, id: i64) -> Result<(), AppError>;
    /// A sale allocation, an outflow allocation or a return line points at the lot.
    fn is_lot_referenced(&self, lot_id: i64) -> Result<bool, AppError>;
    /// Like [`Self::is_lot_referenced`] but ignoring return lines of one return (its own link).
    fn is_lot_referenced_except_return(&self, lot_id: i64, return_id: i64) -> Result<bool, AppError>;
    /// FR-52: the `ADJUSTMENT` headers whose notes start `Stock adjustment #{id}:` (the lot link of a positive adjustment).
    fn find_adjustment_headers(&self, adjustment_id: i64) -> Result<Vec<PurchaseTransaction>, AppError>;
    /// `RETURN`-source lots of item `item_id` whose header notes equal `note`, lowest id first.
    fn find_return_lots_by_note(&self, note: &str, item_id: i64) -> Result<Vec<PurchaseItem>, AppError>;
    /// DD-86: the lots of `item_id` a vendor return to `party_id` may take (PURCHASE source, not an
    /// opening bill, bought from the party, of `purchase_id` when given, stock left), `ORDER BY id`.
    /// `party_id = None` matches walk-in bills (no vendor); the caller pairs it with `purchase_id`.
    /// Same rule as [`vendor_return_ineligibility`]; `net_unit_cost` from the header's discount share.
    fn list_vendor_return_lots(
        &self,
        item_id: i64,
        party_id: Option<i64>,
        purchase_id: Option<i64>,
    ) -> Result<Vec<VendorReturnLot>, AppError>;
}

/// DD-86 eligibility of a lot for a vendor return, given the lot's header (the Rust twin of the
/// `list_vendor_return_lots` filter; stock left is checked by the caller's simulated draw).
/// `None` = eligible, else the refusal message. `party_id = None` is a walk-in bill's return: only
/// allowed linked to that bill (its header has no vendor either).
pub fn vendor_return_ineligibility(
    header: &PurchaseTransaction,
    party_id: Option<i64>,
    party_name: Option<&str>,
    linked_bill: Option<&PurchaseTransaction>,
) -> Option<String> {
    if header.source != LotSource::Purchase || header.is_opening_balance {
        return Some("Only purchase lots can be returned to a vendor".to_string());
    }
    if party_id.is_none() && linked_bill.is_none() {
        return Some("Choose the vendor the goods go back to".to_string());
    }
    if header.party_id != party_id {
        return Some(match party_name {
            Some(name) => format!("This lot was not bought from '{name}'"),
            None => "This lot was bought from a vendor; choose that vendor".to_string(),
        });
    }
    match linked_bill {
        Some(bill) if bill.id != header.id => Some(format!("This lot is not on bill {}", bill.transaction_code)),
        _ => None,
    }
}
