use crate::app_state::AppState;
use crate::core::dtos::ImportSummary;
use tauri::State;
use crate::core::error::AppError;

#[tauri::command]
pub async fn import_items_from_excel(state: State<'_, AppState>, file_path: String) -> Result<ImportSummary, AppError> {
    let svc = state.item_import_service.clone();
    super::run_blocking(move || svc.import_items_from_excel(&file_path)).await
}
