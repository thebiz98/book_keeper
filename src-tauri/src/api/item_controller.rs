use crate::app_state::AppState;
use tauri::State;
use crate::core::error::AppError;
use crate::core::dtos::*;

#[tauri::command]
pub async fn search_items_fts(state: State<'_, AppState>, query: String, limit: Option<usize>) -> Result<Vec<ItemSearchResult>, AppError> {
    let svc = state.item_service.clone();
    super::run_blocking(move || svc.search_items_fts(&query, limit)).await
}
#[tauri::command]
pub async fn get_items_catalog(state: State<'_, AppState>, category: Option<String>, low_stock_only: Option<bool>, search: Option<String>, page: Option<usize>, limit: Option<usize>) -> Result<Paginated<ItemSearchResult>, AppError> {
    let svc = state.item_service.clone();
    super::run_blocking(move || svc.get_catalog(CatalogFilter { category, low_stock_only, search, page, limit })).await
}
#[tauri::command]
pub async fn list_categories(state: State<'_, AppState>) -> Result<Vec<String>, AppError> {
    let svc = state.item_service.clone();
    super::run_blocking(move || svc.list_categories()).await
}
#[tauri::command]
pub async fn create_item(state: State<'_, AppState>, payload: CreateItemPayload) -> Result<i64, AppError> {
    let svc = state.item_service.clone();
    super::run_blocking(move || svc.create_item(payload)).await
}
#[tauri::command]
pub async fn update_item(state: State<'_, AppState>, item_id: i64, payload: UpdateItemPayload) -> Result<(), AppError> {
    let svc = state.item_service.clone();
    super::run_blocking(move || svc.update_item(item_id, payload)).await
}
#[tauri::command]
pub async fn update_item_low_stock_threshold(
    state: State<'_, AppState>,
    item_id: i64,
    threshold: f64,
) -> Result<ItemSearchResult, AppError> {
    let svc = state.item_service.clone();
    super::run_blocking(move || svc.update_low_stock_threshold(item_id, threshold)).await
}
#[tauri::command]
pub async fn create_bundle(state: State<'_, AppState>, payload: CreateBundlePayload) -> Result<SavedBundle, AppError> {
    let svc = state.item_service.clone();
    super::run_blocking(move || svc.create_bundle(payload)).await
}
#[tauri::command]
pub async fn update_bundle(state: State<'_, AppState>, bundle_id: i64, payload: UpdateBundlePayload) -> Result<SavedBundle, AppError> {
    let svc = state.item_service.clone();
    super::run_blocking(move || svc.update_bundle(bundle_id, payload)).await
}
#[tauri::command]
pub async fn delete_bundle(state: State<'_, AppState>, bundle_id: i64) -> Result<(), AppError> {
    let svc = state.item_service.clone();
    super::run_blocking(move || svc.delete_bundle(bundle_id)).await
}
#[tauri::command]
pub async fn get_bundle_suggestions(state: State<'_, AppState>) -> Result<Vec<BundleSuggestion>, AppError> {
    let svc = state.item_service.clone();
    super::run_blocking(move || svc.get_bundle_suggestions()).await
}
