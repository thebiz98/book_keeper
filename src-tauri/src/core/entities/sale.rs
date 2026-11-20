use serde::{Deserialize, Serialize};
use crate::core::value_objects::PaymentStatus;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SaleTransaction {
    pub id: i64,
    pub transaction_code: String, // e.g. "SAL-1001"
    pub transaction_date: String, // "YYYY-MM-DD"
    pub party_id: Option<i64>,    // Customer
    pub payment_mode: Option<String>,
    pub payment_status: PaymentStatus,
    pub total_amount: f64,
    pub paid_amount: f64,
    pub balance_due: f64,
    pub notes: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SaleItem {
    pub id: i64,
    pub sale_transaction_id: i64,
    pub item_id: i64,
    pub bundle_id: Option<i64>,
    pub bundle_quantity: f64,
    pub quantity: f64,   // Normalized base units
    pub unit_price: f64, // Effective selling rate per base unit
    pub total_amount: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SaleStockAllocation {
    pub id: i64,
    pub sale_item_id: i64,
    pub purchase_item_id: i64,  // Inward batch depleted
    pub quantity_depleted: f64, // Base units consumed from lot
    pub unit_cost: f64,         // Historical inward cost per base unit (for COGS)
}
