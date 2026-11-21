use crate::app_state::AppState;
use tauri::State;
use crate::core::dtos::*;
use crate::core::entities::PurchaseTransaction;

#[tauri::command]
pub fn get_next_purchase_code(state: State<'_, AppState>) -> Result<String, String> {
    state.purchase_service.get_next_purchase_code().map_err(|e| e.to_string())
}
#[tauri::command]
pub fn create_purchase(state: State<'_, AppState>, payload: CreatePurchasePayload) -> Result<i64, String> {
    state.purchase_service.create_purchase(payload).map_err(|e| e.to_string())
}
#[tauri::command]
pub fn get_purchases_history(state: State<'_, AppState>, filter: PurchaseHistoryFilter) -> Result<PaginatedList<PurchaseTransaction>, String> {
    state.purchase_service.get_purchases_history(filter).map_err(|e| e.to_string())
}
#[tauri::command]
pub fn get_purchase_details(state: State<'_, AppState>, purchase_id: i64) -> Result<PurchaseDetailView, String> {
    state.purchase_service.get_purchase_details(purchase_id).map_err(|e| e.to_string())
}
