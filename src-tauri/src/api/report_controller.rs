use crate::app_state::AppState;
use tauri::State;
use crate::core::error::AppError;
use crate::core::dtos::*;

#[tauri::command]
pub async fn get_dashboard_metrics(state: State<'_, AppState>) -> Result<DashboardMetrics, AppError> {
    let svc = state.report_service.clone();
    super::run_blocking(move || svc.get_dashboard_metrics()).await
}
#[tauri::command]
pub async fn get_low_stock_alerts(state: State<'_, AppState>) -> Result<Vec<LowStockItem>, AppError> {
    let svc = state.report_service.clone();
    super::run_blocking(move || svc.get_low_stock_alerts()).await
}
#[tauri::command]
pub async fn get_sales_purchase_trend(state: State<'_, AppState>, period: String) -> Result<Vec<TrendDataPoint>, AppError> {
    let svc = state.report_service.clone();
    super::run_blocking(move || svc.get_sales_purchase_trend(&period)).await
}
#[tauri::command]
pub async fn get_pnl_report(state: State<'_, AppState>, from_date: String, to_date: String) -> Result<PnLReport, AppError> {
    let svc = state.report_service.clone();
    super::run_blocking(move || svc.get_pnl_report(&from_date, &to_date)).await
}
#[tauri::command]
pub async fn get_stock_valuation_report(state: State<'_, AppState>) -> Result<StockValuationReport, AppError> {
    let svc = state.report_service.clone();
    super::run_blocking(move || svc.get_stock_valuation_report()).await
}
#[tauri::command]
pub async fn get_cash_flow_report(state: State<'_, AppState>, from_date: String, to_date: String) -> Result<CashFlowReport, AppError> {
    let svc = state.report_service.clone();
    super::run_blocking(move || svc.get_cash_flow_report(&from_date, &to_date)).await
}
