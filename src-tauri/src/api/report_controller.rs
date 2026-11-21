use crate::app_state::AppState;
use tauri::State;
use crate::core::dtos::*;

#[tauri::command]
pub fn get_dashboard_metrics(state: State<'_, AppState>) -> Result<DashboardMetrics, String> {
    state.report_service.get_dashboard_metrics().map_err(|e| e.to_string())
}
#[tauri::command]
pub fn get_low_stock_alerts(state: State<'_, AppState>) -> Result<Vec<LowStockItem>, String> {
    state.report_service.get_low_stock_alerts().map_err(|e| e.to_string())
}
#[tauri::command]
pub fn get_sales_purchase_trend(state: State<'_, AppState>, period: String) -> Result<Vec<TrendDataPoint>, String> {
    state.report_service.get_sales_purchase_trend(&period).map_err(|e| e.to_string())
}
#[tauri::command]
pub fn get_pnl_report(state: State<'_, AppState>, from_date: String, to_date: String) -> Result<PnLReport, String> {
    state.report_service.get_pnl_report(&from_date, &to_date).map_err(|e| e.to_string())
}
#[tauri::command]
pub fn get_stock_valuation_report(state: State<'_, AppState>) -> Result<StockValuationReport, String> {
    state.report_service.get_stock_valuation_report().map_err(|e| e.to_string())
}
#[tauri::command]
pub fn get_cash_flow_report(state: State<'_, AppState>, from_date: String, to_date: String) -> Result<CashFlowReport, String> {
    state.report_service.get_cash_flow_report(&from_date, &to_date).map_err(|e| e.to_string())
}
