use crate::app_state::AppState;
use tauri::State;
use crate::core::dtos::*;

#[tauri::command]
pub fn get_next_return_code(state: State<'_, AppState>) -> Result<String, String> {
    state.return_service.get_next_return_code().map_err(|e| e.to_string())
}
#[tauri::command]
pub fn create_customer_return(state: State<'_, AppState>, payload: CustomerReturnPayload) -> Result<String, String> {
    state.return_service.create_customer_return(payload).map_err(|e| e.to_string())
}
#[tauri::command]
pub fn create_vendor_return(state: State<'_, AppState>, payload: VendorReturnPayload) -> Result<String, String> {
    state.return_service.create_vendor_return(payload).map_err(|e| e.to_string())
}
#[tauri::command]
pub fn get_return_details(state: State<'_, AppState>, return_id: i64) -> Result<ReturnDetailView, String> {
    state.return_service.get_return_details(return_id).map_err(|e| e.to_string())
}
