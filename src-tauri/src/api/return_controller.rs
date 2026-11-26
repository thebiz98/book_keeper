use crate::app_state::AppState;
use tauri::State;
use crate::core::error::AppError;
use crate::core::dtos::*;
use crate::core::entities::ReturnTransaction;

#[tauri::command]
pub async fn get_next_return_code(state: State<'_, AppState>) -> Result<String, AppError> {
    let svc = state.return_service.clone();
    super::run_blocking(move || svc.get_next_return_code()).await
}
#[tauri::command]
pub async fn create_customer_return(state: State<'_, AppState>, payload: CustomerReturnPayload) -> Result<ReturnResult, AppError> {
    let svc = state.return_service.clone();
    super::run_blocking(move || svc.create_customer_return(payload)).await
}
#[tauri::command]
pub async fn create_vendor_return(state: State<'_, AppState>, payload: VendorReturnPayload) -> Result<ReturnResult, AppError> {
    let svc = state.return_service.clone();
    super::run_blocking(move || svc.create_vendor_return(payload)).await
}
#[tauri::command]
pub async fn get_return_details(state: State<'_, AppState>, return_id: i64) -> Result<ReturnDetailView, AppError> {
    let svc = state.return_service.clone();
    super::run_blocking(move || svc.get_return_details(return_id)).await
}
#[tauri::command]
pub async fn list_returns(state: State<'_, AppState>, filter: ReturnsFilter) -> Result<Paginated<ReturnTransaction>, AppError> {
    let svc = state.return_service.clone();
    super::run_blocking(move || svc.list_returns(filter)).await
}
#[tauri::command]
pub async fn delete_return(state: State<'_, AppState>, return_id: i64) -> Result<DeleteReturnResult, AppError> {
    let svc = state.return_service.clone();
    super::run_blocking(move || svc.delete_return(return_id)).await
}
#[tauri::command]
pub async fn preview_customer_return(state: State<'_, AppState>, payload: CustomerReturnPayload) -> Result<ReturnResult, AppError> {
    let svc = state.return_service.clone();
    super::run_blocking(move || svc.preview_customer_return(payload)).await
}
#[tauri::command]
pub async fn preview_vendor_return(state: State<'_, AppState>, payload: VendorReturnPayload) -> Result<ReturnResult, AppError> {
    let svc = state.return_service.clone();
    super::run_blocking(move || svc.preview_vendor_return(payload)).await
}
/// DD-86: the lots a vendor return may take (picker rows and per-lot maximum).
#[tauri::command]
pub async fn list_vendor_return_lots(
    state: State<'_, AppState>,
    item_id: i64,
    party_id: i64,
    purchase_id: Option<i64>,
) -> Result<Vec<VendorReturnLot>, AppError> {
    let svc = state.return_service.clone();
    super::run_blocking(move || svc.list_vendor_return_lots(item_id, party_id, purchase_id)).await
}
