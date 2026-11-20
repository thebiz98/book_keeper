use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AdjustmentReason {
    AuditCorrection,
    DamageWriteOff,
    Expiry,
    Theft,
    FoundStock,
    InitialSetup,
    Damaged,
    Expired,
    CountCorrection,
    TheftLost,
}
