use serde::{Deserialize, Serialize};

/// Lot allocation algorithm. `app_state` maps the chosen method to an
/// `Arc<dyn InventoryAllocationStrategy>` (DD-08); services never name a concrete strategy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AllocationMethod {
    #[default]
    Fifo,
}
