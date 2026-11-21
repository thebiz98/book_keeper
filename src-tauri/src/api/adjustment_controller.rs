use crate::app_state::AppState;
use tauri::State;
use crate::core::dtos::*;
use crate::core::entities::StockAdjustment;

#[tauri::command]
pub fn create_stock_adjustment(state: State<'_, AppState>, payload: StockAdjustmentPayload) -> Result<i64, String> {
    state.inventory_service.create_adjustment(payload).map_err(|e| e.to_string())
}
#[tauri::command]
pub fn list_stock_adjustments(state: State<'_, AppState>, item_id: Option<i64>, page: Option<usize>, limit: Option<usize>) -> Result<PaginatedList<StockAdjustment>, String> {
    state.inventory_service
        .list_adjustments(item_id, page.unwrap_or(1), limit.unwrap_or(50))
        .map_err(|e| e.to_string())
}
