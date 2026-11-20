use serde::{Deserialize, Serialize};
use crate::core::entities::Party;
use crate::core::value_objects::PartyType;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PartyDueSummary {
    pub party_id: i64,
    pub party_name: String,
    pub phone: Option<String>,
    pub party_type: PartyType,
    pub total_due: f64,
    pub oldest_bill_date: Option<String>,
    pub unpaid_bills_count: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SettlePaymentPayload {
    pub party_id: i64,
    pub amount: f64,
    pub settlement_date: String,        // "YYYY-MM-DD"
    pub payment_mode: Option<String>,
    pub specific_bill_id: Option<i64>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PartyLedgerEntry {
    pub date: String,
    pub transaction_code: String,
    pub transaction_type: String, // "SALE", "PURCHASE", "SETTLEMENT", "RETURN"
    pub debit: f64,
    pub credit: f64,
    pub running_balance: f64,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PartyLedgerView {
    pub party: Party,
    pub current_balance: f64,
    pub entries: Vec<PartyLedgerEntry>,
}
