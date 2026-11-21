use crate::app_state::AppState;
use crate::core::dtos::ImportSummary;
use tauri::State;

#[tauri::command]
pub fn import_items_from_excel(state: State<'_, AppState>, file_path: String) -> Result<ImportSummary, String> {
    state.item_import_service.import_items_from_excel(&file_path).map_err(|e| e.to_string())
}
