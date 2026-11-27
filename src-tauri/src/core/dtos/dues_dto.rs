use serde::{Deserialize, Serialize};
use crate::core::entities::{Party, PaymentSettlement, ReturnTransaction};
use crate::core::value_objects::{PaymentStatus, TransactionType};

/// Outstanding dues of one party, aggregated over its open bills (`balance_due > 0`).
/// Customers: sales; vendors: `source = PURCHASE` purchases (DD-07).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PartyDueSummary {
    pub party_id: i64,
    pub party_name: String,
    pub phone: Option<String>,
    pub party_type: crate::core::value_objects::PartyType,
    pub unsettled_bills_count: usize,
    /// Minimum `transaction_date` among the open bills.
    pub earliest_bill_date: String,
    /// Σ `total_amount` of the open bills.
    pub total_invoiced: f64,
    /// Σ `paid_amount` of the open bills (upfront + settlements; return credits are not in it).
    pub total_paid: f64,
    /// L3: Σ (UPFRONT + SETTLEMENT + RETURN_CREDIT) of the open bills = `total_invoiced - balance_due`.
    /// The Dues "Paid & credited" column.
    #[serde(default)]
    pub paid_and_credited: f64,
    /// Σ `balance_due` of the open bills.
    pub balance_due: f64,
    /// FR-46: false for a deleted party that still has dues (UI shows a "Deleted" badge).
    #[serde(default = "default_true")]
    pub is_active: bool,
}

fn default_true() -> bool {
    true
}

/// Cash settlement of dues (DD-04). `transaction_id = Some(bill)` settles that bill
/// (amount <= its balance); `None` is a lump sum applied to the party's oldest open bills
/// (amount <= the party's total outstanding). `transaction_type` must match the party type
/// (SALE => CUSTOMER, PURCHASE => VENDOR).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SettlePaymentPayload {
    pub party_id: i64,
    pub transaction_type: TransactionType,
    pub transaction_id: Option<i64>,
    pub settlement_date: String, // "YYYY-MM-DD"
    pub amount_paid: f64,
    pub payment_mode: Option<String>,
    pub notes: Option<String>,
}

/// One bill's share of a batch settlement.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SettleAllocation {
    pub transaction_id: i64,
    pub amount_paid: f64,
}

/// D9: settle several bills of one party in a single all-or-nothing transaction (1..=200, unique).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SettleBatchPayload {
    pub party_id: i64,
    pub transaction_type: TransactionType,
    pub settlement_date: String, // "YYYY-MM-DD"
    pub payment_mode: Option<String>,
    pub notes: Option<String>,
    pub allocations: Vec<SettleAllocation>,
}

/// D8: edit of one SETTLEMENT row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UpdateSettlementPayload {
    pub amount_paid: f64,
    pub settlement_date: String, // "YYYY-MM-DD"
    pub payment_mode: Option<String>,
    pub notes: Option<String>,
}

/// D8: what a settlement delete removed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeleteSettlementResult {
    pub removed_amount: f64,
    pub transaction_id: i64,
}

/// Result of a settlement: one SETTLEMENT ledger row per bill touched.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SettlementResult {
    pub settlement_ids: Vec<i64>,
    pub bills_touched: Vec<i64>,
}

/// One chronological line of a party ledger. `transaction_type` is the entry kind: `SALE` /
/// `PURCHASE` (a bill, debit), `UPFRONT` / `SETTLEMENT` / `RETURN_CREDIT` (credit),
/// `REFUND_OUT` / `REFUND_IN` / `RETURN` (informational, no effect on the balance).
/// `running_balance` is what the party owes the shop (customer) or the shop owes the party (vendor).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PartyLedgerEntry {
    pub date: String,
    pub transaction_code: String,
    pub transaction_type: String,
    pub debit: f64,
    pub credit: f64,
    pub running_balance: f64,
    pub notes: Option<String>,
}

/// A bill (sale or PURCHASE-source purchase) in a party ledger, open or closed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PartyLedgerBill {
    pub id: i64,
    pub transaction_type: TransactionType,
    pub transaction_code: String,
    pub transaction_date: String,
    pub total_amount: f64,
    pub paid_amount: f64,
    pub balance_due: f64,
    pub payment_status: PaymentStatus,
    /// Party opening-balance bill (FR-35).
    #[serde(default)]
    pub is_opening_balance: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PartyLedgerView {
    pub party: Party,
    pub current_balance: f64,
    pub bills: Vec<PartyLedgerBill>,
    pub payments: Vec<PaymentSettlement>,
    pub returns: Vec<ReturnTransaction>,
    pub entries: Vec<PartyLedgerEntry>,
}

/// FR-42: filter and page request for the settlement history tab. `side` limits to one
/// transaction type (SALE = received from customers, PURCHASE = paid to vendors).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SettlementHistoryFilter {
    pub from_date: String, // "YYYY-MM-DD"
    pub to_date: String,   // "YYYY-MM-DD"
    #[serde(default)]
    pub side: Option<TransactionType>,
    #[serde(default)]
    pub party_search: Option<String>,
    pub page: u32,
    pub page_size: u32,
}

/// FR-42: one `SETTLEMENT` ledger row with its bill and party.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SettlementHistoryRow {
    pub settlement_id: i64,
    pub settlement_date: String,
    pub party_id: Option<i64>,
    pub party_name: String,
    pub transaction_type: TransactionType,
    pub transaction_id: i64,
    pub transaction_code: String,
    pub is_opening_balance: bool,
    pub amount: f64,
    /// Prefills the edit form (an edit without it would reset the mode).
    pub payment_mode: Option<String>,
    pub notes: Option<String>,
}

/// FR-42: one page plus totals over the whole filter (ignoring paging).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SettlementHistoryPage {
    pub rows: Vec<SettlementHistoryRow>,
    pub total_received: f64,
    pub total_paid: f64,
    pub total_count: i64,
}
