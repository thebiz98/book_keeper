use crate::app_state::AppState;
use tauri::State;
use crate::core::error::AppError;
use crate::core::dtos::*;
use crate::core::entities::SaleTransaction;

#[tauri::command]
pub async fn get_next_sale_code(state: State<'_, AppState>) -> Result<String, AppError> {
    let svc = state.sale_service.clone();
    super::run_blocking(move || svc.get_next_sale_code()).await
}
#[tauri::command]
pub async fn create_sale(state: State<'_, AppState>, payload: CreateSalePayload) -> Result<SaleSummaryResponse, AppError> {
    let svc = state.sale_service.clone();
    super::run_blocking(move || svc.create_sale(payload)).await
}
#[tauri::command]
pub async fn get_sales_history(state: State<'_, AppState>, filter: SalesHistoryFilter) -> Result<PaginatedList<SaleTransaction>, AppError> {
    let svc = state.sale_service.clone();
    super::run_blocking(move || svc.get_sales_history(filter)).await
}
#[tauri::command]
pub async fn get_sale_details(state: State<'_, AppState>, sale_id: i64) -> Result<SaleDetailView, AppError> {
    let svc = state.sale_service.clone();
    super::run_blocking(move || svc.get_sale_details(sale_id)).await
}
