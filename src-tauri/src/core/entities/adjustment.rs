use serde::{Deserialize, Serialize};
use crate::core::value_objects::AdjustmentReason;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StockAdjustment {
    pub id: i64,
    pub adjustment_date: String, // "YYYY-MM-DD"
    pub item_id: i64,
    pub quantity: f64, // Positive (+) to add, negative (-) to write off
    pub reason: AdjustmentReason,
    pub notes: Option<String>,
    pub created_at: String,
}

/// Audit trail for negative stock adjustment FIFO lot depletions.
/// Mirrors SALE_STOCK_ALLOCATION but links to STOCK_ADJUSTMENT instead of SALE_ITEM.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AdjustmentStockAllocation {
    pub id: i64,
    pub stock_adjustment_id: i64,
    pub purchase_item_id: i64,  // Inward batch depleted
    pub quantity_depleted: f64, // Base units consumed from lot
    pub unit_cost: f64,         // Historical inward cost per base unit (for loss valuation)
}
