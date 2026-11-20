use serde::{Deserialize, Serialize};
use crate::core::value_objects::AdjustmentReason;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StockAdjustmentPayload {
    #[serde(default)]
    pub adjustment_date: String,
    pub item_id: i64,
    pub quantity: f64, // Positive (+) to add, negative (-) to write off
    pub reason: AdjustmentReason,
    pub notes: Option<String>,
}

pub type AdjustmentItemPayload = StockAdjustmentPayload;
