use crate::app_state::AppState;
use crate::core::dtos::{ConflictStrategy, ImportSummary, ImportValidationReport};
use crate::core::error::AppError;
use tauri::State;

#[tauri::command]
pub async fn validate_items_import(state: State<'_, AppState>, file_path: String) -> Result<ImportValidationReport, AppError> {
    let svc = state.item_import_service.clone();
    super::run_blocking(move || svc.validate_items_import(&file_path)).await
}

#[tauri::command]
pub async fn execute_items_import(
    state: State<'_, AppState>,
    file_path: String,
    conflict_strategy: ConflictStrategy,
) -> Result<ImportSummary, AppError> {
    let svc = state.item_import_service.clone();
    super::run_blocking(move || svc.execute_items_import(&file_path, conflict_strategy)).await
}
