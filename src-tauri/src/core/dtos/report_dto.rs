use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DashboardMetrics {
    #[serde(default)]
    pub today_sales: f64,
    #[serde(default)]
    pub today_purchases: f64,
    #[serde(default)]
    pub today_sales_amount: f64,
    #[serde(default)]
    pub today_sales_count: usize,
    #[serde(default)]
    pub today_purchase_amount: f64,
    #[serde(default)]
    pub today_purchase_count: usize,
    pub today_net_profit: f64,      // Today's Sales - Today's COGS
    pub total_stock_valuation: f64, // Sum of (remaining_qty * unit_cost)
    pub customer_receivables: f64,  // Total outstanding customer dues
    pub vendor_payables: f64,       // Total outstanding vendor dues
    pub live_cash_in_drawer: f64,   // Physical cash balancing formula
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LowStockItem {
    pub id: i64,
    #[serde(default)]
    pub item_id: i64,
    pub name: String,
    pub category: Option<String>,
    pub base_unit: String,
    pub current_stock: f64, // strictly < 5
    #[serde(default)]
    pub default_sale_price: f64,
    pub mrp: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrendDataPoint {
    pub date: String, // "YYYY-MM-DD"
    pub sales_amount: f64,
    pub purchase_amount: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PnLReport {
    #[serde(default)]
    pub from_date: String,
    #[serde(default)]
    pub to_date: String,
    pub gross_sales: f64,
    pub cogs: f64, // Total FIFO Cost of Goods Sold
    pub net_profit: f64,
    #[serde(default)]
    pub total_discounts: f64,
    #[serde(default)]
    pub returns_refunded: f64,
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
    #[serde(default)]
    pub active_lots_count: usize,
    #[serde(default)]
    pub total_items_count: usize,
    #[serde(default)]
    pub categories_breakdown: Vec<CategoryValuation>,
    #[serde(default)]
    pub categories: Vec<CategoryValuation>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CashFlowReport {
    #[serde(default)]
    pub from_date: String,
    #[serde(default)]
    pub to_date: String,
    #[serde(default)]
    pub total_inflow: f64,
    #[serde(default)]
    pub total_outflow: f64,
    #[serde(default)]
    pub net_cash_flow: f64,
    #[serde(default)]
    pub closing_drawer_balance: f64,
    #[serde(default)]
    pub sales_cash_inflow: f64,
    #[serde(default)]
    pub customer_settlements_inflow: f64,
    #[serde(default)]
    pub purchase_cash_outflow: f64,
    #[serde(default)]
    pub vendor_settlements_outflow: f64,
    #[serde(default)]
    pub refunds_cash_outflow: f64,
    #[serde(default)]
    pub net_cash_balance: f64,
}
