use serde::{Deserialize, Serialize};
use crate::core::value_objects::PaymentStatus;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PurchaseTransaction {
    pub id: i64,
    pub transaction_code: String, // e.g. "PUR-1001"
    pub transaction_date: String, // "YYYY-MM-DD"
    pub party_id: Option<i64>,    // Supplier
    pub payment_mode: Option<String>,
    pub payment_status: PaymentStatus,
    pub total_amount: f64,
    pub paid_amount: f64,
    pub balance_due: f64,
    pub notes: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PurchaseItem {
    pub id: i64,
    pub purchase_transaction_id: i64,
    pub item_id: i64,
    pub bundle_id: Option<i64>,
    pub bundle_quantity: f64,
    pub quantity: f64,           // Normalized base units (bundle_qty * multiplier)
    pub unit_cost: f64,          // Cost per base unit in ₹
    pub total_amount: f64,
    pub remaining_quantity: f64, // Active lot base units available for FIFO
}
