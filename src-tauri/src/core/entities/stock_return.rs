use serde::{Deserialize, Serialize};
use crate::core::value_objects::{ReturnLineType, ReturnType, SettlementType, StockCondition};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReturnTransaction {
    pub id: i64,
    pub return_code: String,     // e.g. "RET-1001"
    pub return_date: String,     // "YYYY-MM-DD"
    pub return_type: ReturnType, // Customer or Vendor
    pub party_id: Option<i64>,
    pub party_name: Option<String>, // Snapshot at write time (DD-30)
    pub original_transaction_id: Option<i64>, // Sale id (Customer) / Purchase id (Vendor); polymorphic, no FK
    pub settlement_type: SettlementType,
    pub total_amount: f64,
    pub reason: Option<String>,
    pub notes: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReturnItem {
    pub id: i64,
    pub return_transaction_id: i64,
    pub line_type: ReturnLineType, // Returned or ReplacementIssued
    pub item_id: i64,
    pub item_name: String,         // Name snapshot at write time (DD-30)
    pub sale_item_id: Option<i64>,     // Linked sale line for FIFO lot restoration
    pub purchase_item_id: Option<i64>, // Linked purchase lot for vendor returns
    pub quantity: f64,                 // Base units
    pub unit_price: f64,
    pub total_amount: f64,
    pub cost_amount: f64,              // FIFO cost of reversed (Returned) or issued units
    pub condition: Option<StockCondition>, // Customer Returned lines only
}
