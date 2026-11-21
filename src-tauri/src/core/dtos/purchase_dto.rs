use serde::{Deserialize, Serialize};
use crate::core::entities::{Party, PaymentSettlement, PurchaseItem, PurchaseTransaction};
use crate::core::value_objects::PaymentStatus;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CreatePurchasePayload {
    pub transaction_date: String,             // "YYYY-MM-DD"
    pub party_id: Option<i64>,                // Vendor/Supplier (required if UNPAID or PARTIAL)
    pub supplier_invoice_ref: Option<String>, // Own column on the header (DD-25)
    pub payment_status: PaymentStatus,        // Paid, Partial, Unpaid; cross-checked (DD-02)
    pub paid_amount: f64,                     // Upfront cash paid (never > total)
    pub payment_mode: Option<String>,
    pub notes: Option<String>,
    pub items: Vec<CreatePurchaseItemPayload>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CreatePurchaseItemPayload {
    pub item_id: i64,
    pub bundle_id: Option<i64>, // Write-time helper only; not stored (DD-28)
    pub bundle_quantity: f64,   // Inward quantity in bundle or base units
    pub unit_cost: f64,         // Inward cost per selected unit/bundle (₹)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PurchaseSummaryResponse {
    pub purchase_id: i64,
    pub transaction_code: String,
}

/// Same shape as the sales filter; only `source = PURCHASE` headers are listed and
/// `search_query` also matches `supplier_invoice_ref`.
pub type PurchaseHistoryFilter = crate::core::dtos::sale_dto::SalesHistoryFilter;

/// One row of the purchases history (party name is the header snapshot, DD-30).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PurchaseHistoryRow {
    pub id: i64,
    pub transaction_code: String,
    pub supplier_invoice_ref: Option<String>,
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
pub struct PurchaseItemDetail {
    pub purchase_item: PurchaseItem,
    pub item_name: String,
    pub bundle_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PurchaseDetailView {
    pub purchase: PurchaseTransaction,
    pub party: Option<Party>,
    pub items: Vec<PurchaseItemDetail>,
    pub settlements: Vec<PaymentSettlement>,
}
