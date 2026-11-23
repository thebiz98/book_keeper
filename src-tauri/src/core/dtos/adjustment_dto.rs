use serde::{Deserialize, Serialize};
use crate::core::value_objects::AdjustmentReason;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StockAdjustmentPayload {
    /// `YYYY-MM-DD`; required (validated by `InventoryService::create_adjustment`).
    #[serde(default)]
    pub adjustment_date: String,
    pub item_id: i64,
    pub quantity: f64, // Positive (+) to add, negative (-) to write off
    pub reason: AdjustmentReason,
    /// Positive adjustments only: explicit lot cost. Falls back to the item's latest lot cost
    /// (DD-12); with neither, the adjustment is rejected. Ignored for write-offs.
    #[serde(default)]
    pub unit_cost: Option<f64>,
    pub notes: Option<String>,
}

pub type AdjustmentItemPayload = StockAdjustmentPayload;

/// Result of `create_adjustment`: the new row id and, for write-offs, the FIFO loss value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StockAdjustmentResult {
    pub adjustment_id: i64,
    /// Σ qty × lot cost for a write-off; `0.0` for a positive adjustment.
    pub loss_value: f64,
}

/// `set_item_stock` input (FR-08 rev., DD-40): the counted stock on hand.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SetItemStockPayload {
    pub item_id: i64,
    pub counted_quantity: f64,
    /// Used only when the count is above the system stock (unrecorded stock lot cost).
    #[serde(default)]
    pub unit_cost: Option<f64>,
    #[serde(default)]
    pub adjustment_date: String,
    #[serde(default)]
    pub notes: Option<String>,
}

/// `set_item_stock` result. A zero delta writes nothing (`adjustment_id`/`reason` are `None`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SetItemStockResult {
    pub adjustment_id: Option<i64>,
    pub delta: f64,
    pub reason: Option<AdjustmentReason>,
    pub previous_stock: f64,
    pub new_stock: f64,
    pub unit_cost: Option<f64>,
    /// `+delta x unit_cost` for unrecorded stock, `-FIFO loss` for a count correction.
    pub value_change: f64,
}
