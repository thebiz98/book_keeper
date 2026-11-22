use tauri::State;

use crate::app_state::AppState;
use crate::core::error::AppError;
use crate::services::contracts::data_reset_service::{ResetScope, ResetSummary};

/// Testing-only data reset. The service refuses unless `testingMode` is on, so hiding the UI
/// is not the only guard.
#[tauri::command]
pub async fn reset_data(state: State<'_, AppState>, scope: ResetScope) -> Result<ResetSummary, AppError> {
    let svc = state.data_reset_service.clone();
    let dir = state.backup_dir.read().unwrap_or_else(|e| e.into_inner()).to_string_lossy().into_owned();
    super::run_blocking(move || svc.reset(scope, &dir)).await
}
