use crate::core::dtos::{
    CashFlowReport, DashboardMetrics, InventorySpeedFilter, InventorySpeedReport, ItemProfitFilter,
    ItemProfitReport, LowStockItem, PnLReport, RecentTransaction,
    StockValuationReport, TrendDataPoint,
};
use crate::core::error::AppError;

pub trait ReportService: Send + Sync {
    /// Coordinated single-payload query returning all dashboard KPIs in < 2ms.
    fn get_dashboard_metrics(&self) -> Result<DashboardMetrics, AppError>;
    fn get_low_stock_alerts(&self) -> Result<Vec<LowStockItem>, AppError>;
    fn get_sales_purchase_trend(&self, period: &str) -> Result<Vec<TrendDataPoint>, AppError>;
    fn get_pnl_report(&self, from_date: &str, to_date: &str) -> Result<PnLReport, AppError>;
    fn get_stock_valuation_report(&self) -> Result<StockValuationReport, AppError>;
    fn get_cash_flow_report(&self, from_date: &str, to_date: &str) -> Result<CashFlowReport, AppError>;
    /// Newest `limit` (1..=100) sales, PURCHASE-source purchases and returns, newest first.
    fn get_recent_transactions(&self, limit: usize) -> Result<Vec<RecentTransaction>, AppError>;
    /// Item-wise profit for the period (FR-38). Read-only; sorting, search and paging run in SQL.
    fn get_item_profit_report(&self, filter: ItemProfitFilter) -> Result<ItemProfitReport, AppError>;
    /// Fast / slow / dead stock for the period (FR-39, DD-68). Read-only.
    fn get_inventory_speed_report(&self, filter: InventorySpeedFilter) -> Result<InventorySpeedReport, AppError>;
}

impl<T: ReportService + ?Sized> ReportService for std::sync::Arc<T> {
    fn get_dashboard_metrics(&self) -> Result<DashboardMetrics, AppError> {
        (**self).get_dashboard_metrics()
    }
    fn get_low_stock_alerts(&self) -> Result<Vec<LowStockItem>, AppError> {
        (**self).get_low_stock_alerts()
    }
    fn get_sales_purchase_trend(&self, period: &str) -> Result<Vec<TrendDataPoint>, AppError> {
        (**self).get_sales_purchase_trend(period)
    }
    fn get_pnl_report(&self, from_date: &str, to_date: &str) -> Result<PnLReport, AppError> {
        (**self).get_pnl_report(from_date, to_date)
    }
    fn get_stock_valuation_report(&self) -> Result<StockValuationReport, AppError> {
        (**self).get_stock_valuation_report()
    }
    fn get_cash_flow_report(&self, from_date: &str, to_date: &str) -> Result<CashFlowReport, AppError> {
        (**self).get_cash_flow_report(from_date, to_date)
    }
    fn get_recent_transactions(&self, limit: usize) -> Result<Vec<RecentTransaction>, AppError> {
        (**self).get_recent_transactions(limit)
    }
    fn get_item_profit_report(&self, filter: ItemProfitFilter) -> Result<ItemProfitReport, AppError> {
        (**self).get_item_profit_report(filter)
    }
    fn get_inventory_speed_report(&self, filter: InventorySpeedFilter) -> Result<InventorySpeedReport, AppError> {
        (**self).get_inventory_speed_report(filter)
    }
}
