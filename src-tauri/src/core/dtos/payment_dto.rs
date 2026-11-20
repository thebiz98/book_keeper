use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecordPaymentPayload {
    pub party_id: Option<i64>,
    pub transaction_type: String, // "SALE" or "PURCHASE"
    pub transaction_id: i64,
    pub settlement_date: String, // "YYYY-MM-DD"
    pub amount_paid: f64,
    pub payment_mode: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecordRefundPayload {
    pub party_id: Option<i64>,
    pub transaction_type: String, // "SALE" or "PURCHASE"
    pub transaction_id: Option<i64>,
    pub refund_date: String,
    pub amount: f64,
    pub payment_mode: Option<String>,
    pub notes: Option<String>,
}
