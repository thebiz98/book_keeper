use crate::app_state::AppState;
use tauri::State;
use crate::core::error::AppError;
use crate::core::dtos::*;

#[tauri::command]
pub async fn get_next_purchase_code(state: State<'_, AppState>) -> Result<String, AppError> {
    let svc = state.purchase_service.clone();
    super::run_blocking(move || svc.get_next_purchase_code()).await
}
#[tauri::command]
pub async fn create_purchase(state: State<'_, AppState>, payload: CreatePurchasePayload) -> Result<PurchaseSummaryResponse, AppError> {
    let svc = state.purchase_service.clone();
    super::run_blocking(move || svc.create_purchase(payload)).await
}
#[tauri::command]
pub async fn get_purchases_history(state: State<'_, AppState>, filter: PurchaseHistoryFilter) -> Result<Paginated<PurchaseHistoryRow>, AppError> {
    let svc = state.purchase_service.clone();
    super::run_blocking(move || svc.get_purchases_history(filter)).await
}
#[tauri::command]
pub async fn get_purchase_details(state: State<'_, AppState>, purchase_id: i64) -> Result<PurchaseDetailView, AppError> {
    let svc = state.purchase_service.clone();
    super::run_blocking(move || svc.get_purchase_details(purchase_id)).await
}
#[tauri::command]
pub async fn update_purchase(state: State<'_, AppState>, purchase_id: i64, payload: CreatePurchasePayload) -> Result<PurchaseSummaryResponse, AppError> {
    let svc = state.purchase_service.clone();
    super::run_blocking(move || svc.update_purchase(purchase_id, payload)).await
}
#[tauri::command]
pub async fn update_lot_purchase_price(state: State<'_, AppState>, payload: UpdateLotPricePayload) -> Result<LotPriceUpdateResult, AppError> {
    let svc = state.purchase_service.clone();
    super::run_blocking(move || svc.update_lot_purchase_price(payload)).await
}
#[tauri::command]
pub async fn delete_purchase(state: State<'_, AppState>, purchase_id: i64) -> Result<DeleteBillResult, AppError> {
    let svc = state.purchase_service.clone();
    super::run_blocking(move || svc.delete_purchase(purchase_id)).await
}
/// DD-56: dry run of create/update purchase (transaction dropped); errors come back as data.
#[tauri::command]
pub async fn preview_purchase(state: State<'_, AppState>, payload: PreviewPurchasePayload, purchase_id: Option<i64>) -> Result<BillPreview, AppError> {
    let svc = state.purchase_service.clone();
    super::run_blocking(move || svc.preview_purchase(purchase_id, payload)).await
}
#[tauri::command]
pub async fn preview_lot_purchase_price(state: State<'_, AppState>, payload: UpdateLotPricePayload) -> Result<LotPriceUpdateResult, AppError> {
    let svc = state.purchase_service.clone();
    super::run_blocking(move || svc.preview_lot_purchase_price(payload)).await
}
