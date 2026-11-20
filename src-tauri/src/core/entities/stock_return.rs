use serde::{Deserialize, Serialize};
use crate::core::value_objects::{ReturnType, SettlementType, StockCondition};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReturnTransaction {
    pub id: i64,
    pub return_code: String,     // e.g. "RET-1001"
    pub return_date: String,     // "YYYY-MM-DD"
    pub return_type: ReturnType, // Customer or Vendor
    pub party_id: Option<i64>,
    pub original_transaction_id: Option<i64>,
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
    pub item_id: i64,
    pub sale_item_id: Option<i64>,     // Linked sale line for FIFO lot restoration
    pub purchase_item_id: Option<i64>, // Linked purchase lot for vendor returns
    pub quantity: f64,                 // Returned base units
    pub unit_price: f64,
    pub total_amount: f64,
    pub condition: StockCondition, // Restockable or DamagedScrap
}
