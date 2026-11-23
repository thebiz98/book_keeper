use serde::{Deserialize, Serialize};
use crate::core::value_objects::{LotSource, PaymentStatus};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PurchaseTransaction {
    pub id: i64,
    pub transaction_code: String, // e.g. "PUR-1001"
    pub source: LotSource,        // Non-Purchase: total 0, party None, status Paid (DD-07)
    pub supplier_invoice_ref: Option<String>,
    pub transaction_date: String, // "YYYY-MM-DD"
    pub party_id: Option<i64>,    // Supplier
    pub party_name: Option<String>, // Snapshot at write time (DD-30)
    pub payment_mode: Option<String>,
    pub payment_status: PaymentStatus,
    pub total_amount: f64,
    pub paid_amount: f64,
    pub balance_due: f64,
    pub notes: Option<String>,
    pub created_at: String,
    /// Σ line totals for PURCHASE-source bills (0 on source-lot headers); FR-19.
    pub items_subtotal: f64,
    pub charges_total: f64,
    pub discounts_total: f64,
    /// Last edit time (FR-20, v8); `None` if never edited.
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PurchaseItem {
    pub id: i64,
    pub purchase_transaction_id: i64,
    pub item_id: i64,
    pub item_name: String,          // Name snapshot at write time (DD-30)
    pub pack_label: Option<String>, // Display snapshot ("Box of 10"); None = base units (DD-28)
    pub pack_quantity: f64,         // Display snapshot: qty in that pack
    pub quantity: f64,           // Base units (pack_quantity * multiplier at entry)
    pub unit_cost: f64,          // Cost per base unit in ₹
    pub total_amount: f64,
    pub remaining_quantity: f64, // Active lot base units available for FIFO
}
