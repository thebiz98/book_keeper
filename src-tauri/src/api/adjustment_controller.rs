use crate::app_state::AppState;
use tauri::State;
use crate::core::error::AppError;
use crate::core::dtos::*;
use crate::core::entities::StockAdjustment;

#[tauri::command]
pub async fn create_stock_adjustment(state: State<'_, AppState>, payload: StockAdjustmentPayload) -> Result<StockAdjustmentResult, AppError> {
    let svc = state.inventory_service.clone();
    super::run_blocking(move || svc.create_adjustment(payload)).await
}
#[tauri::command]
pub async fn set_item_stock(state: State<'_, AppState>, payload: SetItemStockPayload) -> Result<SetItemStockResult, AppError> {
    let svc = state.inventory_service.clone();
    super::run_blocking(move || svc.set_item_stock(payload)).await
}
#[tauri::command]
pub async fn get_item_lots(state: State<'_, AppState>, item_id: i64) -> Result<Vec<ItemLot>, AppError> {
    let svc = state.inventory_service.clone();
    super::run_blocking(move || svc.get_item_lots(item_id)).await
}
#[tauri::command]
pub async fn list_stock_adjustments(state: State<'_, AppState>, item_id: Option<i64>, page: Option<usize>, limit: Option<usize>) -> Result<Paginated<StockAdjustment>, AppError> {
    let svc = state.inventory_service.clone();
    super::run_blocking(move || svc.list_adjustments(item_id, page, limit)).await
}
#[tauri::command]
pub async fn get_item_sales(state: State<'_, AppState>, item_id: i64, page: Option<usize>, limit: Option<usize>) -> Result<Paginated<ItemBillRef>, AppError> {
    let svc = state.inventory_service.clone();
    super::run_blocking(move || svc.get_item_sales(item_id, page, limit)).await
}
#[tauri::command]
pub async fn get_item_purchases(state: State<'_, AppState>, item_id: i64, page: Option<usize>, limit: Option<usize>) -> Result<Paginated<ItemBillRef>, AppError> {
    let svc = state.inventory_service.clone();
    super::run_blocking(move || svc.get_item_purchases(item_id, page, limit)).await
}
#[tauri::command]
pub async fn preview_stock_adjustment(state: State<'_, AppState>, payload: StockAdjustmentPayload) -> Result<StockAdjustmentPreview, AppError> {
    let svc = state.inventory_service.clone();
    super::run_blocking(move || svc.preview_adjustment(payload)).await
}
#[tauri::command]
pub async fn preview_set_item_stock(state: State<'_, AppState>, payload: SetItemStockPayload) -> Result<SetItemStockResult, AppError> {
    let svc = state.inventory_service.clone();
    super::run_blocking(move || svc.preview_set_item_stock(payload)).await
}
#[tauri::command]
pub async fn preview_lot_draw(state: State<'_, AppState>, item_id: i64, quantity: f64, prior_lines: Vec<PriorLine>) -> Result<LotDraw, AppError> {
    let svc = state.inventory_service.clone();
    super::run_blocking(move || svc.preview_lot_draw(item_id, quantity, &prior_lines)).await
}
#[tauri::command]
pub async fn preview_service_cost(state: State<'_, AppState>, payload: PreviewServiceCostPayload) -> Result<ServiceCostPreview, AppError> {
    let svc = state.inventory_service.clone();
    super::run_blocking(move || svc.preview_service_cost(payload)).await
}
#[tauri::command]
pub async fn update_opening_lot_quantity(state: State<'_, AppState>, payload: UpdateOpeningQuantityPayload) -> Result<OpeningQuantityResult, AppError> {
    let svc = state.inventory_service.clone();
    super::run_blocking(move || svc.update_opening_lot_quantity(payload)).await
}
#[tauri::command]
pub async fn preview_opening_lot_quantity(state: State<'_, AppState>, payload: UpdateOpeningQuantityPayload) -> Result<OpeningQuantityResult, AppError> {
    let svc = state.inventory_service.clone();
    super::run_blocking(move || svc.preview_opening_lot_quantity(payload)).await
}
