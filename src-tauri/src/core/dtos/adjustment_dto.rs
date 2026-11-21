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
