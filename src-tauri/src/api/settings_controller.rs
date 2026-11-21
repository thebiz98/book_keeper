use crate::app_state::AppState;
use tauri::State;
use crate::core::error::AppError;
use crate::core::entities::ShopSetting;

#[tauri::command]
pub async fn get_shop_settings(state: State<'_, AppState>) -> Result<ShopSetting, AppError> {
    let svc = state.shop_setting_service.clone();
    super::run_blocking(move || svc.get_settings()).await
}
#[tauri::command]
pub async fn update_shop_settings(state: State<'_, AppState>, settings: ShopSetting) -> Result<(), AppError> {
    let svc = state.shop_setting_service.clone();
    super::run_blocking(move || svc.update_settings(settings)).await
}
#[tauri::command]
pub async fn backup_database(state: State<'_, AppState>, destination_path: String) -> Result<String, AppError> {
    let svc = state.backup_service.clone();
    super::run_blocking(move || svc.backup_database(&destination_path)).await
}
#[tauri::command]
pub async fn verify_database_integrity(state: State<'_, AppState>) -> Result<bool, AppError> {
    let svc = state.backup_service.clone();
    super::run_blocking(move || svc.verify_database_integrity()).await
}
