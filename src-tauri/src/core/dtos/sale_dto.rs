use serde::{Deserialize, Serialize};
use crate::core::dtos::charge_dto::BillChargeInput;
use crate::core::entities::{BillCharge, Party, PaymentSettlement, SaleItem, SaleStockAllocation, SaleTransaction};
use crate::core::dtos::pagination::{validate_date_range, PageRequest};
use crate::core::error::AppError;
use crate::core::value_objects::PaymentStatus;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CreateSalePayload {
    pub transaction_date: String,      // "YYYY-MM-DD"
    pub party_id: Option<i64>,         // Customer ID (required if UNPAID or PARTIAL)
    pub payment_status: PaymentStatus, // Paid, Partial, Unpaid; cross-checked against paid_amount (DD-02)
    pub paid_amount: f64,              // Cash received (Paid: may exceed total; the excess is change)
    pub payment_mode: Option<String>,  // e.g. "CASH", "UPI"
    pub notes: Option<String>,
    pub items: Vec<CreateSaleItemPayload>,
    /// Bill-level charge/discount lines (FR-19).
    #[serde(default)]
    pub charges: Vec<BillChargeInput>,
    /// DD-88 edit only: the user confirmed moving the bill's later payments to the new party.
    /// Ignored on create.
    #[serde(default)]
    pub confirm_move_settlements: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CreateSaleItemPayload {
    pub item_id: i64,
    pub bundle_id: Option<i64>, // Write-time helper only (multiplier + label); not stored (DD-28)
    pub bundle_quantity: f64,   // Units entered in selected packaging
    pub unit_price: f64,        // Rate charged per selected bundle/unit (₹); required
    /// DD-89 edit only: the stored line this payload line keeps (`None` = a new line). Lines match
    /// by it, so only what changed is written. `create_sale` rejects `Some`.
    #[serde(default)]
    pub sale_item_id: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SaleSummaryResponse {
    pub sale_id: i64,
    pub transaction_code: String,
    pub total_amount: f64,
    pub paid_amount: f64,
    pub balance_due: f64,
    pub change_amount: f64, // MAX(0, tendered - total); not stored. paid_amount is the stored upfront.
    pub payment_status: PaymentStatus,
}

/// Status filter of the history lists: `ALL` (or absent) means no status predicate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PaymentStatusFilter {
    All,
    Paid,
    Partial,
    Unpaid,
}

impl PaymentStatusFilter {
    pub fn as_status(self) -> Option<PaymentStatus> {
        match self {
            Self::All => None,
            Self::Paid => Some(PaymentStatus::Paid),
            Self::Partial => Some(PaymentStatus::Partial),
            Self::Unpaid => Some(PaymentStatus::Unpaid),
        }
    }
}

/// Filter of the sales and purchases history lists (HLD 9.3). Every field is optional; dates are
/// inclusive `YYYY-MM-DD`; `search_query` matches the transaction code or the party name
/// snapshot (purchases: also `supplier_invoice_ref`), case-insensitively for ASCII.
/// Newest first (`transaction_date DESC, id DESC`). Page defaults: page 1, limit 50 (max 200).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SalesHistoryFilter {
    #[serde(default)]
    pub start_date: Option<String>,
    #[serde(default)]
    pub end_date: Option<String>,
    #[serde(default)]
    pub payment_status: Option<PaymentStatusFilter>,
    #[serde(default)]
    pub party_id: Option<i64>,
    #[serde(default)]
    pub search_query: Option<String>,
    #[serde(default)]
    pub page: Option<usize>,
    #[serde(default)]
    pub limit: Option<usize>,
    /// Sales only (FR-34): when true each row carries the bill profit; otherwise `profit` is `null`.
    #[serde(default)]
    pub include_profit: Option<bool>,
    /// Dues paths only (FR-35): when true the list also holds the party's opening-balance bill.
    /// Absent / false: opening bills are excluded (history is trade only).
    #[serde(default)]
    pub include_opening_balance: Option<bool>,
    /// Sales history only (FR-34): when true the page also carries `totals` over every matching bill.
    #[serde(default)]
    pub include_totals: Option<bool>,
}

impl SalesHistoryFilter {
    /// Validates dates and paging; returns the effective page request.
    pub fn validate(&self) -> Result<PageRequest, AppError> {
        validate_date_range(self.start_date.as_deref(), self.end_date.as_deref())?;
        PageRequest::new(self.page, self.limit)
    }
}

/// Totals over every bill matching the history filter (all pages), computed in SQL (DD-56).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SalesHistoryTotals {
    pub bill_count: usize,
    pub total_amount: f64,
    pub paid_amount: f64,
    pub balance_due: f64,
    /// Bill-based profit (FR-34); `None` unless the filter asked for profit.
    #[serde(default)]
    pub profit: Option<f64>,
}

/// A sales history page: `Paginated` fields plus optional `totals` (when `include_totals`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SalesHistoryPage {
    pub items: Vec<SaleHistoryRow>,
    pub total: usize,
    pub page: usize,
    pub limit: usize,
    #[serde(default)]
    pub totals: Option<SalesHistoryTotals>,
}

/// One row of the sales history (party name is the header snapshot, DD-30).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SaleHistoryRow {
    pub id: i64,
    pub transaction_code: String,
    pub transaction_date: String,
    pub created_at: String,
    pub party_id: Option<i64>,
    pub party_name: Option<String>,
    pub line_count: i64,
    pub total_amount: f64,
    pub paid_amount: f64,
    pub balance_due: f64,
    pub payment_status: PaymentStatus,
    pub items_subtotal: f64,
    pub charges_total: f64,
    pub discounts_total: f64,
    /// Bill profit (FR-34); `None` unless the filter asked for it.
    #[serde(default)]
    pub profit: Option<f64>,
    /// Party opening-balance bill (FR-35); only present when the filter asked for it.
    #[serde(default)]
    pub is_opening_balance: bool,
}

/// Raw components of one bill's profit (same sources as the P&L), computed in SQL.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BillProfitParts {
    /// Gross FIFO cost of the lines less the `cost_amount` of their linked returned lines (DD-91).
    pub cost_net: f64,
    /// Σ linked RETURNED line totals of customer REFUND returns (R6: lines, not the header).
    pub refunds: f64,
    /// Cost of scrapped returned units (stays spent).
    pub scrap_cost: f64,
    /// FIFO cost of replacement units issued against the bill.
    pub replacement_cost: f64,
    /// Miscellaneous service cost of the bill's service lines (FR-37), derived in SQL.
    #[serde(default)]
    pub misc_cost: f64,
    /// `round2(total_amount - cost_net - misc_cost - refunds - scrap_cost - replacement_cost)`.
    pub total: f64,
}

/// Reconciliation of a sale's profit (FR-34): `lines_profit + charges + return_adjustment == total`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProfitSummary {
    pub lines_profit: f64,
    pub charges: f64,
    /// Refunds, scrapped cost and replacement cost, as a number <= 0.
    pub return_adjustment: f64,
    pub total: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SaleItemDetail {
    pub sale_item: SaleItem,
    pub item_name: String,
    pub bundle_name: Option<String>,
    /// E4 / DD-89: the item's bundle this line was sold in, resolved at read time from the stored
    /// pack (label and units per pack; units alone for a renamed bundle). `None` for base units or
    /// a bundle that no longer exists. The edit form loads the line by it.
    #[serde(default)]
    pub bundle_id: Option<i64>,
    /// Base units in one pack of this line (`quantity / pack_quantity`); `None` for a base-unit line.
    /// Printed next to the bundle name on the bill.
    #[serde(default)]
    pub units_per_bundle: Option<f64>,
    /// The item's base unit (e.g. "pcs"); `None` when the item no longer exists.
    #[serde(default)]
    pub base_unit: Option<String>,
    /// Base units of this line already returned by customer returns.
    pub returned_quantity: f64,
    /// FIFO cost breakdown of the line.
    pub allocations: Vec<SaleStockAllocation>,
    /// Sold rate net of the bill-level discount share (FR-19): the refund cap of a linked return.
    pub net_unit_price: f64,
    /// Line cost: FIFO cost net of returned units plus `misc_cost` (FR-34, FR-37).
    pub cost_amount: f64,
    /// Miscellaneous service cost of a service line (0 for physical lines).
    #[serde(default)]
    pub misc_cost: f64,
    /// True for a service line (FR-37): not returnable, so return pickers skip it.
    #[serde(default)]
    pub is_service: bool,
    /// FR-45: the item was deleted since the sale; return pickers disable the line (the backend rejects it).
    #[serde(default)]
    pub item_deleted: bool,
    /// `line total - bill discount share - cost_amount` (FR-34).
    pub profit: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SaleDetailView {
    pub sale: SaleTransaction,
    pub party: Option<Party>,
    pub items: Vec<SaleItemDetail>,
    pub settlements: Vec<PaymentSettlement>,
    pub charges: Vec<BillCharge>,
    pub profit_summary: ProfitSummary,
}
