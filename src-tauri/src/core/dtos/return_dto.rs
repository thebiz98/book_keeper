use serde::{Deserialize, Serialize};
use crate::core::entities::{PaymentSettlement, Party, ReturnItem, ReturnTransaction, StockOutflowAllocation};
use crate::core::value_objects::{ReturnType, SettlementType, StockCondition};

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
    /// REPLACEMENT only (any active item, base units); must be empty for REFUND.
    #[serde(default)]
    pub replacement_items: Vec<ReplacementItemPayload>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReplacementItemPayload {
    pub item_id: i64,
    pub quantity: f64, // Issued base units
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CustomerReturnItemPayload {
    pub item_id: i64,
    pub sale_item_id: Option<i64>, // Linked sale item for lot restoration
    pub quantity: f64,             // Returned base units
    pub unit_price: f64,
    pub condition: StockCondition,
    /// Unlinked RESTOCKABLE only: lot cost when the item has no prior lot.
    #[serde(default)]
    pub unit_cost_override: Option<f64>,
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
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReturnResult {
    pub return_id: i64,
    pub return_code: String,
    pub credited_amount: f64, // applied to dues as RETURN_CREDIT
    pub cash_refunded: f64,   // REFUND_OUT (customer) / REFUND_IN (vendor)
    /// Value of the returned lines (customer: qty x refund rate; vendor: the lot cost removed).
    pub total_amount: f64,
    /// Replacement only: Σ qty x the issued item's default sale price (display of the value gap); else 0.
    pub issued_value: f64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ReturnsFilter {
    #[serde(default)]
    pub return_type: Option<ReturnType>,
    #[serde(default)]
    pub start_date: Option<String>,
    #[serde(default)]
    pub end_date: Option<String>,
    #[serde(default)]
    pub page: Option<usize>,
    #[serde(default)]
    pub limit: Option<usize>,
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
    /// FIFO outflow rows logged for this return (REPLACEMENT issues / VENDOR_RETURN removals).
    pub allocations: Vec<StockOutflowAllocation>,
    /// Ledger rows (RETURN_CREDIT / REFUND_OUT / REFUND_IN) booked by this return.
    pub ledger: Vec<PaymentSettlement>,
}
