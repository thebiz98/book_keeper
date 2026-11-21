use crate::app_state::AppState;
use tauri::State;
use crate::core::dtos::*;
use crate::core::entities::Item;

#[tauri::command]
pub fn search_items_fts(state: State<'_, AppState>, query: String, limit: Option<usize>) -> Result<Vec<ItemSearchResult>, String> {
    let r = match limit {
        Some(l) => state.search_indexer.search_with_limit(&query, l),
        None => state.search_indexer.search(&query),
    };
    r.map_err(|e| e.to_string())
}
#[tauri::command]
pub fn get_items_catalog(state: State<'_, AppState>, category: Option<String>, low_stock_only: Option<bool>, page: Option<usize>, limit: Option<usize>) -> Result<PaginatedList<Item>, String> {
    state.item_service
        .get_catalog(category, low_stock_only.unwrap_or(false), page.unwrap_or(1), limit.unwrap_or(50))
        .map_err(|e| e.to_string())
}
#[tauri::command]
pub fn create_item(state: State<'_, AppState>, payload: CreateItemPayload) -> Result<i64, String> {
    state.item_service.create_item(payload).map_err(|e| e.to_string())
}
#[tauri::command]
pub fn update_item(state: State<'_, AppState>, item_id: i64, payload: UpdateItemPayload) -> Result<(), String> {
    state.item_service.update_item(item_id, payload).map_err(|e| e.to_string())
}
#[tauri::command]
pub fn archive_item(state: State<'_, AppState>, item_id: i64) -> Result<(), String> {
    state.item_service.archive_item(item_id).map_err(|e| e.to_string())
}
#[tauri::command]
pub fn create_bundle(state: State<'_, AppState>, payload: CreateBundlePayload) -> Result<i64, String> {
    state.item_service.create_bundle(payload).map_err(|e| e.to_string())
}
#[tauri::command]
pub fn get_bundle_suggestions(state: State<'_, AppState>) -> Result<Vec<BundleSuggestion>, String> {
    state.item_service.get_bundle_suggestions().map_err(|e| e.to_string())
}
