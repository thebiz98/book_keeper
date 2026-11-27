use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AdjustmentReason {
    Damaged,
    Expired,
    CountCorrection,
    TheftLost,
    /// Positive only; lot at user-entered cost (DD-40).
    UnrecordedStock,
}

impl AdjustmentReason {
    /// DD-92 (S3): only found stock may add. Damaged, Expired and Theft / lost only reduce.
    pub fn allows_increase(self) -> bool {
        matches!(self, AdjustmentReason::UnrecordedStock | AdjustmentReason::CountCorrection)
    }
}
