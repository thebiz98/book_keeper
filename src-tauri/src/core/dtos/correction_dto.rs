//! FR-20 bill correction DTOs: the pre-check guard and the delete results.
use serde::{Deserialize, Serialize};

/// Which kind of record a guard describes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CorrectionTarget {
    Sale,
    Purchase,
    Return,
}

/// One reason an edit or delete is blocked (`code` is a stable `BillLocked` reason code).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GuardReason {
    pub code: String,
    pub message: String,
    pub blocks_edit: bool,
    pub blocks_delete: bool,
}

/// Per-lot limits of a purchase edit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LotEditLimit {
    pub purchase_item_id: i64,
    /// `quantity - remaining_quantity` in base units: the floor for the lot's new quantity.
    pub consumed_quantity: f64,
    /// Allocations or return lines point at the lot: item change / removal is blocked.
    pub referenced: bool,
    /// A `VENDOR_RETURN` outflow exists: the cost is locked.
    pub vendor_returned: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BillEditGuard {
    pub target: CorrectionTarget,
    pub id: i64,
    pub code: String,
    pub can_edit: bool,
    pub can_delete: bool,
    pub can_change_party: bool,
    pub reasons: Vec<GuardReason>,
    /// Σ UPFRONT of the bill.
    pub upfront_amount: f64,
    /// Σ SETTLEMENT + Σ RETURN_CREDIT: money applied later that stays on an edit.
    pub applied_amount: f64,
    /// Cash that leaves the records on delete (bills: UPFRONT + SETTLEMENT; returns: REFUND rows).
    pub removed_cash_amount: f64,
    /// Return credit that is taken back on a return delete (0 for bills).
    pub removed_credit_amount: f64,
    pub lot_limits: Vec<LotEditLimit>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeleteBillResult {
    pub removed_cash_amount: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeleteReturnResult {
    pub removed_cash_amount: f64,
    pub removed_credit_amount: f64,
}
