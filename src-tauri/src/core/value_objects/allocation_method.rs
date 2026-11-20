use serde::{Deserialize, Serialize};

/// Enum selecting the inventory allocation strategy at DI wiring time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AllocationMethod {
    Fifo,
}
