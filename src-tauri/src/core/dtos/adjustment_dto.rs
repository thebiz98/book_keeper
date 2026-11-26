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

/// `update_opening_lot_quantity` input (FR-51, DD-84): the corrected opening quantity of one opening lot.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UpdateOpeningQuantityPayload {
    pub purchase_item_id: i64,
    pub quantity: f64,
}

/// Result of `update_opening_lot_quantity`. No adjustment is recorded, so P&L losses never change.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OpeningQuantityResult {
    pub purchase_item_id: i64,
    pub item_id: i64,
    pub old_quantity: f64,
    pub new_quantity: f64,
    /// Units of this lot already sold, adjusted or returned: the floor for `new_quantity`.
    pub consumed: f64,
    pub unit_cost: f64,
    pub old_stock: f64,
    pub new_stock: f64,
    /// `(new - old) x unit_cost`.
    pub value_change: f64,
    /// True when quantity 0 removed the (unused) opening lot.
    pub lot_removed: bool,
}

/// Why an adjustment cannot be undone (machine code plus the message to show).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AdjustmentUndoReason {
    pub code: String,
    pub message: String,
}

/// `get_adjustment_undo_guard` result (FR-52, DD-85): what undoing the adjustment would do, or why it cannot.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AdjustmentUndoGuard {
    pub can_undo: bool,
    pub reason: Option<AdjustmentUndoReason>,
    /// The adjustment's signed quantity (negative = reduction).
    pub quantity: f64,
    pub item_id: i64,
    pub item_name: String,
    pub base_unit: String,
    /// Reduction: the Inventory Losses amount that disappears; 0 for an increase.
    pub loss_removed: f64,
    /// Effect on stock value: positive for a reduction undone, negative for an increase undone.
    pub value_change: f64,
}
