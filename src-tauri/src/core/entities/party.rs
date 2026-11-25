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
    /// The party's opening bill (FR-35), read-only: filled by the repository, written through
    /// `opening_balance` on the create / update payloads.
    #[serde(default)]
    pub opening_balance: Option<PartyOpeningBalance>,
}

/// A party's opening bill as shown by `get_party` / the directory (FR-35).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PartyOpeningBalance {
    pub bill_id: i64,
    pub transaction_code: String,
    pub amount: f64,
    pub as_of_date: String,
    /// `amount - balance_due`: Σ SETTLEMENT + Σ RETURN_CREDIT applied to the bill.
    pub settled_amount: f64,
}

/// Opening balance input on party create / update (FR-35).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OpeningBalanceInput {
    pub amount: f64,
    pub as_of_date: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CreatePartyPayload {
    pub name: String,
    pub phone: Option<String>,
    pub party_type: PartyType,
    pub notes: Option<String>,
    #[serde(default)]
    pub opening_balance: Option<OpeningBalanceInput>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UpdatePartyPayload {
    pub name: String,
    pub phone: Option<String>,
    pub party_type: PartyType,
    pub notes: Option<String>,
    #[serde(default)]
    pub opening_balance: Option<OpeningBalanceInput>,
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
