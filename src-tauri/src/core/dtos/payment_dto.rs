use serde::{Deserialize, Serialize};
use crate::core::value_objects::{PaymentKind, TransactionType};

/// Records a payment that reduces a bill's balance (`kind` is Upfront or Settlement).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecordPaymentPayload {
    pub party_id: Option<i64>,
    pub transaction_type: TransactionType,
    pub transaction_id: i64,
    pub kind: PaymentKind, // Upfront (checkout / inward) or Settlement (later dues payment)
    pub settlement_date: String, // "YYYY-MM-DD"
    pub amount_paid: f64,
    pub payment_mode: Option<String>,
    pub notes: Option<String>,
}

/// Records a cash refund ledger row (RefundOut for Sale, RefundIn for Purchase). Amount is positive.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecordRefundPayload {
    pub party_id: Option<i64>,
    pub transaction_type: TransactionType,
    pub transaction_id: Option<i64>,
    pub return_id: Option<i64>,
    pub refund_date: String,
    pub amount: f64,
    pub payment_mode: Option<String>,
    pub notes: Option<String>,
}

/// Σ `amount_paid` of one bill's ledger rows grouped by `kind` (zero when a kind has no rows).
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct LedgerTotals {
    pub upfront: f64,
    pub settlement: f64,
    pub return_credit: f64,
    pub refund_out: f64,
    pub refund_in: f64,
}
