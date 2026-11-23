//! Reporting DTOs. Shapes match HLD §9.3 exactly (F5/T7); all money is rounded to 2 decimals.
use serde::{Deserialize, Serialize};

use crate::core::value_objects::PaymentStatus;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DashboardMetrics {
    pub today_sales_amount: f64,
    pub today_sales_count: usize,
    pub today_purchase_amount: f64, // PURCHASE-source bills only
    pub today_purchase_count: usize,
    pub today_net_profit: f64,      // Same formula as PnLReport.net_profit for today
    pub total_stock_valuation: f64, // Sum of (remaining_qty * unit_cost), all lots
    pub customer_receivables: f64,  // Outstanding customer dues
    pub vendor_payables: f64,       // Outstanding vendor dues (PURCHASE source)
    pub live_cash_in_drawer: f64,   // Today's ledger cash in - cash out - today's expenses
    pub today_expenses_amount: f64, // Σ expenses dated today (FR-18)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RecentTransactionKind {
    Sale,
    Purchase,
    Return,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecentTransaction {
    pub kind: RecentTransactionKind,
    pub id: i64,
    pub code: String,
    pub date: String,
    pub created_at: String,
    pub party_name: Option<String>,
    pub amount: f64,
    pub payment_status: Option<PaymentStatus>, // None for returns
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LowStockItem {
    pub id: i64,
    pub name: String,
    pub category: Option<String>,
    pub base_unit: String,
    pub current_stock: f64,
    pub default_sale_price: f64,
    pub mrp: Option<f64>,
    #[serde(default = "crate::core::stock_rules::default_low_stock_threshold")]
    pub low_stock_threshold: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrendDataPoint {
    pub date: String, // "YYYY-MM-DD" (first of month for the 1y period)
    pub sales_amount: f64,
    pub purchase_amount: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PnLReport {
    pub gross_sales: f64,
    pub sales_returns: f64,
    pub net_sales: f64,
    pub cogs: f64,
    pub gross_profit: f64,
    pub inventory_losses: f64,
    pub net_profit: f64,
    pub total_purchases: f64,
    /// Σ sale `discounts_total` (FR-19).
    pub sales_discounts: f64,
    /// `sale_charges_income + purchase_discounts_income` (FR-19).
    pub other_income: f64,
    /// Σ expenses by `expense_date` in range + `purchase_charges_expense`.
    pub operating_expenses: f64,
    /// Σ sale `charges_total` in range.
    pub sale_charges_income: f64,
    /// Σ PURCHASE-source `discounts_total` in range.
    pub purchase_discounts_income: f64,
    /// Σ PURCHASE-source `charges_total` in range (already inside `operating_expenses`).
    pub purchase_charges_expense: f64,
    /// Per expense type, `amount DESC, name`; filled by the service from the expense repository.
    pub expenses_by_type: Vec<crate::core::dtos::ExpenseTypeTotal>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CategoryValuation {
    pub category: String,
    pub items_count: usize,
    pub valuation: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StockValuationReport {
    pub total_valuation: f64,
    pub active_lots_count: usize,
    pub categories_breakdown: Vec<CategoryValuation>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CashFlowReport {
    pub sales_cash_inflow: f64,            // SALE·UPFRONT
    pub customer_settlements_inflow: f64,  // SALE·SETTLEMENT
    pub vendor_refunds_inflow: f64,        // PURCHASE·REFUND_IN
    pub purchase_cash_outflow: f64,        // PURCHASE·UPFRONT
    pub vendor_settlements_outflow: f64,   // PURCHASE·SETTLEMENT
    pub refunds_cash_outflow: f64,         // SALE·REFUND_OUT
    pub expenses_cash_outflow: f64,        // Σ expenses by date (FR-18)
    pub net_cash_balance: f64,             // inflows - outflows - expenses
}
