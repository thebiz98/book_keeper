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
#[tauri::command]
pub async fn get_service_components(state: State<'_, AppState>, item_id: i64) -> Result<Vec<ServiceComponentView>, AppError> {
    let svc = state.item_service.clone();
    super::run_blocking(move || svc.get_service_components(item_id)).await
}
#[tauri::command]
pub async fn preview_item_delete(state: State<'_, AppState>, item_id: i64) -> Result<ItemDeletePreview, AppError> {
    let svc = state.item_service.clone();
    super::run_blocking(move || svc.preview_item_delete(item_id)).await
}
#[tauri::command]
pub async fn delete_item(state: State<'_, AppState>, item_id: i64) -> Result<DeleteItemResult, AppError> {
    let svc = state.item_service.clone();
    super::run_blocking(move || svc.delete_item(item_id)).await
}
#[tauri::command]
pub async fn restore_item(state: State<'_, AppState>, item_id: i64) -> Result<(), AppError> {
    let svc = state.item_service.clone();
    super::run_blocking(move || svc.restore_item(item_id)).await
}
#[tauri::command]
pub async fn list_deleted_items(
    state: State<'_, AppState>,
    search: Option<String>,
    page: Option<usize>,
    page_size: Option<usize>,
) -> Result<Paginated<DeletedItemRow>, AppError> {
    let svc = state.item_service.clone();
    super::run_blocking(move || svc.list_deleted_items(search, page, page_size)).await
}
#[tauri::command]
pub async fn get_item(state: State<'_, AppState>, item_id: i64) -> Result<ItemSearchResult, AppError> {
    let svc = state.item_service.clone();
    super::run_blocking(move || svc.get_item(item_id)).await
}
