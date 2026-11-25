//! Reporting DTOs. Shapes match HLD §9.3 exactly (F5/T7); all money is rounded to 2 decimals.
use serde::{Deserialize, Serialize};

use crate::core::value_objects::PaymentStatus;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DashboardMetrics {
    pub today_sales_amount: f64,
    pub today_sales_count: usize,
    pub today_purchase_amount: f64, // PURCHASE-source bills only
    pub today_purchase_count: usize,
    pub total_stock_valuation: f64, // Sum of (remaining_qty * unit_cost), all lots
    pub customer_receivables: f64,  // Outstanding customer dues
    pub vendor_payables: f64,       // Outstanding vendor dues (PURCHASE source)
    pub cash_in_hand: f64,          // opening cash + all-time ledger cash in - out - expenses, up to today (FR-33)
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
    /// Miscellaneous service costs (FR-37): service targets not covered by component FIFO cost.
    #[serde(default)]
    pub service_misc_costs: f64,
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
    /// Σ of the three inflow lines (DD-56: the UI no longer sums).
    pub total_inflow: f64,
    /// Σ of the four outflow lines, expenses included.
    pub total_outflow: f64,
    pub net_cash_balance: f64,             // inflows - outflows - expenses
}

// ---- Item profit and inventory speed (FR-38, FR-39, DD-68) ---------------------------------------

/// Sort request of a report list. `dir` is `ASC` or `DESC`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReportSort {
    pub field: String,
    pub dir: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ItemProfitFilter {
    pub from_date: String,
    pub to_date: String,
    #[serde(default)]
    pub search: Option<String>,
    #[serde(default)]
    pub sort: Option<ReportSort>,
    #[serde(default)]
    pub page: Option<usize>,
    #[serde(default)]
    pub page_size: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ItemProfitRow {
    pub item_id: i64,
    pub name: String,
    pub base_unit: String,
    /// Units sold net of linked customer returns.
    pub qty_sold: f64,
    /// Line totals less the bill discount share and linked refunds.
    pub revenue: f64,
    /// Net FIFO cost (scrapped units keep theirs) plus misc. cost.
    pub cost: f64,
    pub profit: f64,
    /// `profit / revenue * 100`; `None` when revenue is 0.
    pub margin_pct: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ItemProfitTotals {
    pub revenue: f64,
    pub cost: f64,
    pub profit: f64,
    pub margin_pct: Option<f64>,
}

/// `items_profit + charges - replacement_cost = bills_profit` over every non-opening bill in range.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ItemProfitReconciliation {
    pub items_profit: f64,
    pub charges: f64,
    pub replacement_cost: f64,
    pub bills_profit: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ItemProfitReport {
    pub rows: Vec<ItemProfitRow>,
    pub totals: ItemProfitTotals,
    pub reconciliation: ItemProfitReconciliation,
    pub total_count: usize,
    pub page: usize,
    pub page_size: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InventorySpeedFilter {
    pub from_date: String,
    pub to_date: String,
    /// `ALL` (or absent), `FAST`, `NORMAL`, `SLOW`, `DEAD`.
    #[serde(default)]
    pub class: Option<String>,
    #[serde(default)]
    pub search: Option<String>,
    #[serde(default)]
    pub sort: Option<ReportSort>,
    #[serde(default)]
    pub page: Option<usize>,
    #[serde(default)]
    pub page_size: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SpeedClass {
    Fast,
    Normal,
    Slow,
    Dead,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InventorySpeedRow {
    pub item_id: i64,
    pub name: String,
    pub base_unit: String,
    /// Units sold net of returns plus units consumed by services.
    pub units_out: f64,
    pub units_per_day: f64,
    pub stock: f64,
    /// `stock / units_per_day`; `None` when nothing moved.
    pub days_of_cover: Option<f64>,
    pub class: SpeedClass,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpeedCounts {
    pub fast: usize,
    pub normal: usize,
    pub slow: usize,
    pub dead: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InventorySpeedReport {
    pub rows: Vec<InventorySpeedRow>,
    /// Over the search-filtered items, ignoring the class tab.
    pub counts: SpeedCounts,
    /// Inclusive day count of the period (>= 1).
    pub days: u32,
    pub total_count: usize,
    pub page: usize,
    pub page_size: usize,
}
