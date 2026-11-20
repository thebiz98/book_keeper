use crate::core::dtos::{
    CashFlowReport, DashboardMetrics, LowStockItem, PnLReport, StockValuationReport,
    TrendDataPoint,
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
}
