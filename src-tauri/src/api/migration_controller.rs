use crate::app_state::AppState;
use tauri::State;
use crate::core::dtos::*;

#[tauri::command]
pub fn scan_backup_directory(state: State<'_, AppState>, directory_path: Option<String>) -> Result<Vec<BackupFileInfo>, String> {
    let _ = directory_path; // service scans its configured directory
    state.migration_service.scan_backup_directory().map_err(|e| e.to_string())
}
#[tauri::command]
pub fn inspect_backup_file(state: State<'_, AppState>, file_path: String) -> Result<MigrationPreview, String> {
    state.migration_service.inspect_backup_file(&file_path).map_err(|e| e.to_string())
}
#[tauri::command]
pub fn execute_migration(state: State<'_, AppState>, file_path: String) -> Result<MigrationResult, String> {
    state.migration_service.execute_migration(&file_path).map_err(|e| e.to_string())
}
#[tauri::command]
pub fn export_items_to_excel(state: State<'_, AppState>, destination_path: String) -> Result<String, String> {
    state.import_export_service.export_items_to_excel(&destination_path).map_err(|e| e.to_string())?;
    Ok(destination_path)
}
#[tauri::command]
pub fn import_items_from_excel(state: State<'_, AppState>, file_path: String) -> Result<ImportSummary, String> {
    state.import_export_service.import_items_from_excel(&file_path).map_err(|e| e.to_string())
}
#[tauri::command]
pub fn export_transactions_to_excel(state: State<'_, AppState>, destination_path: String, filter: Option<serde_json::Value>) -> Result<String, String> {
    let get = |keys: &[&str], default: &str| -> String {
        filter
            .as_ref()
            .and_then(|f| keys.iter().find_map(|k| f.get(*k)).and_then(|v| v.as_str()))
            .unwrap_or(default)
            .to_string()
    };
    let from = get(&["from_date", "fromDate", "start_date", "startDate"], "0000-01-01");
    let to = get(&["to_date", "toDate", "end_date", "endDate"], "9999-12-31");
    state.import_export_service
        .export_transactions_to_excel(&from, &to, &destination_path)
        .map_err(|e| e.to_string())?;
    Ok(destination_path)
}
