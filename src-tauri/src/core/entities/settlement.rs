use serde::{Deserialize, Serialize};
use crate::core::value_objects::{PaymentKind, TransactionType};

/// The payment ledger: every money event. `amount_paid` is always > 0.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PaymentSettlement {
    pub id: i64,
    pub settlement_date: String, // "YYYY-MM-DD"
    pub party_id: Option<i64>,
    pub kind: PaymentKind,
    pub transaction_type: TransactionType,
    pub transaction_id: Option<i64>, // Required unless kind is RefundOut/RefundIn
    pub return_id: Option<i64>,      // Set for ReturnCredit/RefundOut/RefundIn
    pub amount_paid: f64,            // > 0
    pub payment_mode: Option<String>,
    pub notes: Option<String>,
    pub created_at: String,
}
