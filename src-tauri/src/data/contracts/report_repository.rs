use crate::core::dtos::{CashFlowReport, DashboardMetrics, PnLReport, StockValuationReport, TrendDataPoint};
use crate::core::entities::Item;
use crate::core::error::AppError;

pub trait ReportRepository: Send + Sync {
    fn get_dashboard_metrics(&self, today_date: &str) -> Result<DashboardMetrics, AppError>;
    fn get_low_stock_items(&self) -> Result<Vec<Item>, AppError>;
    fn get_sales_purchase_trend(&self, days: usize) -> Result<Vec<TrendDataPoint>, AppError>;
    fn get_pnl_report(&self, from_date: &str, to_date: &str) -> Result<PnLReport, AppError>;
    fn get_stock_valuation_report(&self) -> Result<StockValuationReport, AppError>;
    fn get_cash_flow_report(&self, from_date: &str, to_date: &str) -> Result<CashFlowReport, AppError>;
}
