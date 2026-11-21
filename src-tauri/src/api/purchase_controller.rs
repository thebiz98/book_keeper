use crate::app_state::AppState;
use tauri::State;
use crate::core::error::AppError;
use crate::core::dtos::*;
use crate::core::entities::PurchaseTransaction;

#[tauri::command]
pub async fn get_next_purchase_code(state: State<'_, AppState>) -> Result<String, AppError> {
    let svc = state.purchase_service.clone();
    super::run_blocking(move || svc.get_next_purchase_code()).await
}
#[tauri::command]
pub async fn create_purchase(state: State<'_, AppState>, payload: CreatePurchasePayload) -> Result<i64, AppError> {
    let svc = state.purchase_service.clone();
    super::run_blocking(move || svc.create_purchase(payload)).await
}
#[tauri::command]
pub async fn get_purchases_history(state: State<'_, AppState>, filter: PurchaseHistoryFilter) -> Result<PaginatedList<PurchaseTransaction>, AppError> {
    let svc = state.purchase_service.clone();
    super::run_blocking(move || svc.get_purchases_history(filter)).await
}
#[tauri::command]
pub async fn get_purchase_details(state: State<'_, AppState>, purchase_id: i64) -> Result<PurchaseDetailView, AppError> {
    let svc = state.purchase_service.clone();
    super::run_blocking(move || svc.get_purchase_details(purchase_id)).await
}
