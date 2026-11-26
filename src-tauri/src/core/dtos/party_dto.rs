use serde::{Deserialize, Serialize};
use crate::core::dtos::DeleteMode;
use crate::core::value_objects::PartyType;

/// FR-46: what deleting a party would do, computed by the backend (DD-56). Writes nothing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PartyDeletePreview {
    pub party_id: i64,
    pub name: String,
    pub party_type: PartyType,
    /// HARD = nothing references the party, removed permanently (DD-80); SOFT = `is_active = 0`.
    pub mode: DeleteMode,
    /// Bills of the party's side with `balance_due > 0` (opening bills included).
    pub open_bills_count: i64,
    pub balance_due_total: f64,
    pub can_delete: bool,
    /// Why `can_delete` is false (backend sentence).
    pub reason: Option<String>,
    /// Why the party is SOFT ("3 bills", "1 payment"); empty for HARD.
    pub reasons_used: Vec<String>,
}

/// `delete_party` result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeletePartyResult {
    pub mode: DeleteMode,
}

/// One row of the Deleted parties list (customers and vendors together).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeletedPartyRow {
    pub party_id: i64,
    pub name: String,
    pub party_type: PartyType,
    pub phone: Option<String>,
    /// `updated_at` (the delete time: every other write to a deleted party is rejected).
    pub deleted_at: String,
}
