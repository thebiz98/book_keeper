use crate::app_state::AppState;
use tauri::State;
use crate::core::error::AppError;
use crate::core::dtos::*;

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
pub async fn get_sales_history(state: State<'_, AppState>, filter: SalesHistoryFilter) -> Result<Paginated<SaleHistoryRow>, AppError> {
    let svc = state.sale_service.clone();
    super::run_blocking(move || svc.get_sales_history(filter)).await
}
#[tauri::command]
pub async fn get_sale_details(state: State<'_, AppState>, sale_id: i64) -> Result<SaleDetailView, AppError> {
    let svc = state.sale_service.clone();
    super::run_blocking(move || svc.get_sale_details(sale_id)).await
}
#[tauri::command]
pub async fn update_sale(state: State<'_, AppState>, sale_id: i64, payload: CreateSalePayload) -> Result<SaleSummaryResponse, AppError> {
    let svc = state.sale_service.clone();
    super::run_blocking(move || svc.update_sale(sale_id, payload)).await
}
#[tauri::command]
pub async fn delete_sale(state: State<'_, AppState>, sale_id: i64) -> Result<DeleteBillResult, AppError> {
    let svc = state.sale_service.clone();
    super::run_blocking(move || svc.delete_sale(sale_id)).await
}
/// DD-56: dry run of create/update sale (transaction dropped); errors come back as data.
#[tauri::command]
pub async fn preview_sale(state: State<'_, AppState>, payload: PreviewSalePayload, sale_id: Option<i64>) -> Result<BillPreview, AppError> {
    let svc = state.sale_service.clone();
    super::run_blocking(move || svc.preview_sale(sale_id, payload)).await
}
/// FR-47: entry-row line total and the adjustment of a typed total (no database).
#[tauri::command]
pub async fn preview_sale_line(state: State<'_, AppState>, payload: PreviewSaleLinePayload) -> Result<SaleLinePreview, AppError> {
    let svc = state.sale_service.clone();
    super::run_blocking(move || svc.preview_sale_line(payload)).await
}
