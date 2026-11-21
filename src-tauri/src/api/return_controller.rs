use crate::app_state::AppState;
use tauri::State;
use crate::core::error::AppError;
use crate::core::dtos::*;

#[tauri::command]
pub async fn get_next_return_code(state: State<'_, AppState>) -> Result<String, AppError> {
    let svc = state.return_service.clone();
    super::run_blocking(move || svc.get_next_return_code()).await
}
#[tauri::command]
pub async fn create_customer_return(state: State<'_, AppState>, payload: CustomerReturnPayload) -> Result<String, AppError> {
    let svc = state.return_service.clone();
    super::run_blocking(move || svc.create_customer_return(payload)).await
}
#[tauri::command]
pub async fn create_vendor_return(state: State<'_, AppState>, payload: VendorReturnPayload) -> Result<String, AppError> {
    let svc = state.return_service.clone();
    super::run_blocking(move || svc.create_vendor_return(payload)).await
}
#[tauri::command]
pub async fn get_return_details(state: State<'_, AppState>, return_id: i64) -> Result<ReturnDetailView, AppError> {
    let svc = state.return_service.clone();
    super::run_blocking(move || svc.get_return_details(return_id)).await
}
