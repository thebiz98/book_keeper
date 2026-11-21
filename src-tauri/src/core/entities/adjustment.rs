use serde::{Deserialize, Serialize};
use crate::core::value_objects::{AdjustmentReason, OutflowSourceType};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StockAdjustment {
    pub id: i64,
    pub adjustment_date: String, // "YYYY-MM-DD"
    pub item_id: i64,
    pub quantity: f64, // Non-zero. Positive (+) to add, negative (-) to write off
    pub unit_cost: Option<f64>, // Positive adjustments only: lot cost actually used
    pub reason: AdjustmentReason,
    pub notes: Option<String>,
    pub created_at: String,
}

/// FIFO audit trail for every non-sale stock outflow (negative adjustments, replacement
/// issues, vendor returns). Mirrors SALE_STOCK_ALLOCATION. `source_id` is polymorphic
/// (stock_adjustment.id or return_transaction.id) and carries no FK.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StockOutflowAllocation {
    pub id: i64,
    pub source_type: OutflowSourceType,
    pub source_id: i64,
    pub purchase_item_id: i64,  // Inward batch depleted
    pub quantity_depleted: f64, // Base units consumed from lot
    pub unit_cost: f64,         // Lot cost per base unit (for loss valuation)
}
