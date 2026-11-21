use serde::{Deserialize, Serialize};
use crate::core::entities::{Party, ReturnItem, ReturnTransaction};
use crate::core::value_objects::{SettlementType, StockCondition};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CustomerReturnPayload {
    pub original_sale_id: Option<i64>,
    pub party_id: Option<i64>,
    pub return_date: String,
    pub settlement_type: SettlementType,
    #[serde(default)]
    pub reason: Option<String>,
    pub notes: Option<String>,
    pub items: Vec<CustomerReturnItemPayload>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CustomerReturnItemPayload {
    pub item_id: i64,
    pub sale_item_id: Option<i64>, // Linked sale item for lot restoration
    pub quantity: f64,             // Returned base units
    pub unit_price: f64,
    pub condition: StockCondition,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VendorReturnPayload {
    pub original_purchase_id: Option<i64>,
    pub party_id: Option<i64>,
    pub return_date: String,
    pub settlement_type: SettlementType,
    pub reason: Option<String>,
    pub notes: Option<String>,
    pub items: Vec<VendorReturnItemPayload>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VendorReturnItemPayload {
    pub item_id: i64,
    pub purchase_item_id: Option<i64>, // Specific lot being returned (or null for FIFO)
    pub quantity: f64,                 // Returned base units
    pub unit_cost: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReturnItemPayload {
    pub item_id: i64,
    pub sale_item_id: Option<i64>,
    pub purchase_item_id: Option<i64>,
    pub quantity: f64,
    pub unit_price: Option<f64>,
    pub unit_cost: Option<f64>,
    pub condition: Option<StockCondition>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReturnItemDetail {
    pub return_item: ReturnItem,
    pub item_name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReturnDetailView {
    pub return_tx: ReturnTransaction,
    pub party: Option<Party>,
    pub items: Vec<ReturnItemDetail>,
}
