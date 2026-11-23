use serde::{Deserialize, Serialize};

/// Whether a bill-level line adds to (`Charge`) or subtracts from (`Discount`) the bill total.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ChargeEffect {
    Charge,
    Discount,
}
