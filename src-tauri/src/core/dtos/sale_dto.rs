use serde::{Deserialize, Serialize};
use crate::core::entities::{Party, PaymentSettlement, SaleItem, SaleStockAllocation, SaleTransaction};
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
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CreateSaleItemPayload {
    pub item_id: i64,
    pub bundle_id: Option<i64>, // Write-time helper only (multiplier + label); not stored (DD-28)
    pub bundle_quantity: f64,   // Units entered in selected packaging
    pub unit_price: f64,        // Rate charged per selected bundle/unit (₹); required
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
}

impl SalesHistoryFilter {
    /// Validates dates and paging; returns the effective page request.
    pub fn validate(&self) -> Result<PageRequest, AppError> {
        validate_date_range(self.start_date.as_deref(), self.end_date.as_deref())?;
        PageRequest::new(self.page, self.limit)
    }
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
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SaleItemDetail {
    pub sale_item: SaleItem,
    pub item_name: String,
    pub bundle_name: Option<String>,
    /// Base units of this line already returned by customer returns.
    pub returned_quantity: f64,
    /// FIFO cost breakdown of the line.
    pub allocations: Vec<SaleStockAllocation>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SaleDetailView {
    pub sale: SaleTransaction,
    pub party: Option<Party>,
    pub items: Vec<SaleItemDetail>,
    pub settlements: Vec<PaymentSettlement>,
}
