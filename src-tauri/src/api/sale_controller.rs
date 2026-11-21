use crate::app_state::AppState;
use tauri::State;
use crate::core::dtos::*;
use crate::core::entities::SaleTransaction;

#[tauri::command]
pub fn get_next_sale_code(state: State<'_, AppState>) -> Result<String, String> {
    state.sale_service.get_next_sale_code().map_err(|e| e.to_string())
}
#[tauri::command]
pub fn create_sale(state: State<'_, AppState>, payload: CreateSalePayload) -> Result<SaleSummaryResponse, String> {
    state.sale_service.create_sale(payload).map_err(|e| e.to_string())
}
#[tauri::command]
pub fn get_sales_history(state: State<'_, AppState>, filter: SalesHistoryFilter) -> Result<PaginatedList<SaleTransaction>, String> {
    state.sale_service.get_sales_history(filter).map_err(|e| e.to_string())
}
#[tauri::command]
pub fn get_sale_details(state: State<'_, AppState>, sale_id: i64) -> Result<SaleDetailView, String> {
    state.sale_service.get_sale_details(sale_id).map_err(|e| e.to_string())
}
