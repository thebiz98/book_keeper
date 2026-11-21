use crate::app_state::AppState;
use tauri::State;
use crate::core::error::AppError;
use crate::core::dtos::*;
use crate::core::entities::StockAdjustment;

#[tauri::command]
pub async fn create_stock_adjustment(state: State<'_, AppState>, payload: StockAdjustmentPayload) -> Result<i64, AppError> {
    let svc = state.inventory_service.clone();
    super::run_blocking(move || svc.create_adjustment(payload)).await
}
#[tauri::command]
pub async fn list_stock_adjustments(state: State<'_, AppState>, item_id: Option<i64>, page: Option<usize>, limit: Option<usize>) -> Result<PaginatedList<StockAdjustment>, AppError> {
    let svc = state.inventory_service.clone();
    super::run_blocking(move || svc .list_adjustments(item_id, page.unwrap_or(1), limit.unwrap_or(50)) ).await
}
