use serde::{Deserialize, Serialize};
use crate::core::entities::{Party, PaymentSettlement, SaleItem, SaleStockAllocation, SaleTransaction};
use crate::core::value_objects::PaymentStatus;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CreateSalePayload {
    pub transaction_date: String,      // "YYYY-MM-DD"
    pub party_id: Option<i64>,         // Customer ID (required if UNPAID or PARTIAL)
    pub payment_status: PaymentStatus, // Paid, Partial, Unpaid; cross-checked against paid_amount (DD-02)
    pub paid_amount: f64,              // Cash received (Paid: may exceed total; the excess is change)
    pub payment_mode: Option<String>,  // e.g. "CASH", "UPI"
    pub notes: Option<String>,
    pub items: Vec<CreateSaleItemPayload>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CreateSaleItemPayload {
    pub item_id: i64,
    pub bundle_id: Option<i64>, // Write-time helper only (multiplier + label); not stored (DD-28)
    pub bundle_quantity: f64,   // Units entered in selected packaging
    pub unit_price: f64,        // Rate charged per selected bundle/unit (₹); required
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SaleSummaryResponse {
    pub sale_id: i64,
    pub transaction_code: String,
    pub total_amount: f64,
    pub paid_amount: f64,
    pub balance_due: f64,
    pub change_amount: f64, // MAX(0, tendered - total); not stored. paid_amount is the stored upfront.
    pub payment_status: PaymentStatus,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SalesHistoryFilter {
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    #[serde(default)]
    pub from_date: Option<String>,
    #[serde(default)]
    pub to_date: Option<String>,
    pub payment_status: Option<PaymentStatus>,
    pub party_id: Option<i64>,
    pub page: Option<usize>,
    pub limit: Option<usize>,
}

pub type SalesFilter = SalesHistoryFilter;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SaleItemDetail {
    pub sale_item: SaleItem,
    pub item_name: String,
    pub bundle_name: Option<String>,
    pub allocations: Vec<SaleStockAllocation>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SaleDetailView {
    pub sale: SaleTransaction,
    pub party: Option<Party>,
    pub items: Vec<SaleItemDetail>,
    pub settlements: Vec<PaymentSettlement>,
}
