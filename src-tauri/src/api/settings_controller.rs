use crate::app_state::AppState;
use tauri::State;
use crate::core::entities::ShopSetting;

#[tauri::command]
pub fn get_shop_settings(state: State<'_, AppState>) -> Result<ShopSetting, String> {
    state.shop_setting_service.get_settings().map_err(|e| e.to_string())
}
#[tauri::command]
pub fn update_shop_settings(state: State<'_, AppState>, settings: ShopSetting) -> Result<(), String> {
    state.shop_setting_service.update_settings(settings).map_err(|e| e.to_string())
}
#[tauri::command]
pub fn backup_database(state: State<'_, AppState>, destination_path: String) -> Result<String, String> {
    state.backup_service.backup_database(&destination_path).map_err(|e| e.to_string())
}
#[tauri::command]
pub fn verify_database_integrity(state: State<'_, AppState>) -> Result<bool, String> {
    state.backup_service.verify_database_integrity().map_err(|e| e.to_string())
}
