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

/// How Set stock applies a change (FR-57 scope B, DD-92).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SetStockMode {
    /// The opening stock was entered wrong: the Opening lot(s) are edited, no adjustment, no loss.
    OpeningCorrection,
    /// A stock adjustment: COUNT_CORRECTION loss going down, UNRECORDED_STOCK (found, no gain) going up.
    Adjustment,
}

/// `set_item_stock` input (FR-08 rev., DD-40): the counted stock on hand.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
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
    /// The path the user picked from `options`; `None` = the only enabled one (refused when two are).
    #[serde(default)]
    pub mode: Option<SetStockMode>,
}

/// One path Set stock offers for this change, with why it is unavailable (DD-56: the UI only shows it).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SetStockOption {
    pub mode: SetStockMode,
    pub label: String,
    /// One line on what it does to stock, value and P&L.
    pub detail: String,
    pub enabled: bool,
    pub disabled_reason: Option<String>,
    /// True when the typed unit cost is used (found stock); an opening correction keeps the lot cost.
    pub takes_unit_cost: bool,
}

/// An earlier write-off of the item that G7 can undo (DD-92 "Undo it instead").
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UndoableWriteoff {
    pub adjustment_id: i64,
    pub adjustment_date: String,
    /// Units written off (positive).
    pub quantity: f64,
    pub reason: AdjustmentReason,
    /// Inventory Losses amount that undoing removes.
    pub loss: f64,
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
    /// `+delta x unit_cost` for unrecorded stock, `-FIFO loss` for a count correction,
    /// `delta x opening lot cost` for an opening correction.
    pub value_change: f64,
    /// The path applied (or previewed); `None` when nothing changes or a choice is still needed.
    #[serde(default)]
    pub mode: Option<SetStockMode>,
    /// The paths for this change. Empty when nothing changes; one entry when the item has no opening lot.
    #[serde(default)]
    pub options: Vec<SetStockOption>,
    /// Two paths are enabled and the payload named none: nothing was applied, the user must pick.
    #[serde(default)]
    pub choice_required: bool,
    /// Increases only: earlier write-offs that can be undone instead (DD-92).
    #[serde(default)]
    pub undoable_writeoffs: Vec<UndoableWriteoff>,
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

/// One row of the Adjustments list (`list_stock_adjustments`): the adjustment with its item and value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AdjustmentListRow {
    pub id: i64,
    pub adjustment_date: String,
    pub created_at: String,
    pub item_id: i64,
    pub item_name: String,
    pub base_unit: String,
    pub item_is_active: bool,
    /// Signed: negative = reduction.
    pub quantity: f64,
    pub reason: AdjustmentReason,
    pub unit_cost: Option<f64>,
    pub notes: Option<String>,
    /// Positive. Reduction: the FIFO loss (matches Inventory Losses); increase: `quantity x unit_cost`.
    pub value: f64,
}
