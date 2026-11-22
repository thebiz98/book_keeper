use crate::app_state::AppState;
use crate::core::dtos::{ConflictStrategy, ImportSummary, ImportValidationReport, TemplateFormat};
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

#[tauri::command]
pub async fn validate_vedbill_import(state: State<'_, AppState>, file_path: String) -> Result<ImportValidationReport, AppError> {
    let svc = state.vedbill_import_service.clone();
    super::run_blocking(move || svc.validate_vedbill_import(&file_path)).await
}

#[tauri::command]
pub async fn execute_vedbill_import(
    state: State<'_, AppState>,
    file_path: String,
    conflict_strategy: ConflictStrategy,
) -> Result<ImportSummary, AppError> {
    let svc = state.vedbill_import_service.clone();
    super::run_blocking(move || svc.execute_vedbill_import(&file_path, conflict_strategy)).await
}

/// Saves the items-import template; returns the final path (extension corrected to the format).
#[tauri::command]
pub async fn write_items_import_template(dest_path: String, format: TemplateFormat) -> Result<String, AppError> {
    super::run_blocking(move || crate::services::r#impl::import_template_writer::write_template(&dest_path, format)).await
}
