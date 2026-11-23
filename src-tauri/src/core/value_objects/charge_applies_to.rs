use serde::{Deserialize, Serialize};

use super::TransactionType;

/// Which bill side a charge type may be used on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ChargeAppliesTo {
    Sale,
    Purchase,
    Both,
}

impl ChargeAppliesTo {
    pub fn covers(self, side: TransactionType) -> bool {
        matches!(
            (self, side),
            (Self::Both, _) | (Self::Sale, TransactionType::Sale) | (Self::Purchase, TransactionType::Purchase)
        )
    }
}
