use serde::{Deserialize, Serialize};
use crate::core::value_objects::PartyType;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Party {
    pub id: i64,
    pub name: String,
    pub phone: Option<String>,
    pub party_type: PartyType,
    pub notes: Option<String>,
    pub is_active: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CreatePartyPayload {
    pub name: String,
    pub phone: Option<String>,
    pub party_type: PartyType,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UpdatePartyPayload {
    pub name: String,
    pub phone: Option<String>,
    pub party_type: PartyType,
    pub notes: Option<String>,
}

/// Directory row: the party plus its outstanding balance (sale side for customers,
/// `source = PURCHASE` side for vendors) and the number of its bills (`transaction_count`).
/// Serialised flat: all `Party` fields plus the two extras.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PartyListRow {
    #[serde(flatten)]
    pub party: Party,
    pub outstanding_balance: f64,
    pub transaction_count: i64,
}
