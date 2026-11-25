use serde::{Deserialize, Serialize};
use crate::core::value_objects::PaymentStatus;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SaleTransaction {
    pub id: i64,
    pub transaction_code: String, // e.g. "SAL-1001"
    pub transaction_date: String, // "YYYY-MM-DD"
    pub party_id: Option<i64>,    // Customer
    pub party_name: Option<String>, // Snapshot at write time (DD-30)
    pub payment_mode: Option<String>,
    pub payment_status: PaymentStatus,
    pub total_amount: f64,
    pub paid_amount: f64,
    pub balance_due: f64,
    pub notes: Option<String>,
    pub created_at: String,
    /// Σ line totals; `total_amount = items_subtotal + charges_total - discounts_total` (FR-19).
    pub items_subtotal: f64,
    pub charges_total: f64,
    pub discounts_total: f64,
    /// Last edit time (FR-20, v8); `None` if never edited.
    pub updated_at: Option<String>,
    /// Party opening balance bill (FR-35, v11): no lines, never a sale for any report.
    #[serde(default)]
    pub is_opening_balance: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SaleItem {
    pub id: i64,
    pub sale_transaction_id: i64,
    pub item_id: i64,
    pub item_name: String,          // Name snapshot at write time (DD-30)
    pub pack_label: Option<String>, // Display snapshot ("Box of 10"); None = base units (DD-28)
    pub pack_quantity: f64,         // Display snapshot: qty in that pack
    pub quantity: f64,   // Base units
    pub unit_price: f64, // Effective selling rate per base unit
    pub total_amount: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SaleStockAllocation {
    pub id: i64,
    pub sale_item_id: i64,
    pub purchase_item_id: i64,  // Inward batch depleted
    pub quantity_depleted: f64, // Base units consumed from lot
    pub returned_quantity: f64, // Base units later reversed by customer returns (0..=quantity_depleted)
    pub unit_cost: f64,         // Historical inward cost per base unit (for COGS)
}
