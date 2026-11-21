use serde::{Deserialize, Serialize};
use crate::core::value_objects::{PaymentKind, TransactionType};

/// Records a balance-reducing ledger entry (`kind` is Upfront, Settlement or ReturnCredit).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecordPaymentPayload {
    pub party_id: Option<i64>,
    pub transaction_type: TransactionType,
    pub transaction_id: i64,
    pub kind: PaymentKind, // Upfront (checkout / inward), Settlement (dues) or ReturnCredit (return)
    pub return_id: Option<i64>, // Required iff kind == ReturnCredit
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

/// Cash movement over a date range, summed from the ledger by the DD-01 cash direction table.
/// The one source for the live cash drawer and the cash-flow report.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct LedgerCashSummary {
    pub sale_upfront: f64,         // SALE·UPFRONT       (in)
    pub sale_settlement: f64,      // SALE·SETTLEMENT    (in)
    pub vendor_refund_in: f64,     // PURCHASE·REFUND_IN (in)
    pub purchase_upfront: f64,     // PURCHASE·UPFRONT   (out)
    pub purchase_settlement: f64,  // PURCHASE·SETTLEMENT (out)
    pub customer_refund_out: f64,  // SALE·REFUND_OUT    (out)
    pub cash_in: f64,
    pub cash_out: f64,
    pub net: f64,                  // cash_in - cash_out
}
