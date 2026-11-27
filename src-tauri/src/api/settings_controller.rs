use crate::app_state::AppState;
use tauri::State;
use crate::core::error::AppError;
use crate::core::entities::ShopSetting;
use serde::Serialize;
use crate::services::contracts::backup_service::BackupKind;

/// Runtime config shown on the Settings page.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppConfigView {
    pub testing_mode: bool,
    pub backup_directory: String,
    pub is_default_directory: bool,
    pub last_auto_backup_at: Option<String>,
}

pub fn build_config_view(state: &AppState) -> AppConfigView {
    let dir = state.backup_dir.read().unwrap_or_else(|e| e.into_inner()).clone();
    AppConfigView {
        testing_mode: state.config.testing_mode,
        backup_directory: dir.to_string_lossy().into_owned(),
        is_default_directory: dir == state.default_backup_dir,
        last_auto_backup_at: state
            .backup_service
            .last_auto_backup(&dir.to_string_lossy())
            .ok()
            .flatten()
            .map(|t| t.format("%Y-%m-%dT%H:%M:00+05:30").to_string()),
    }
}

/// Persists and applies a new backup folder; `None`/blank resets to the default.
pub fn apply_backup_directory(state: &AppState, path: Option<String>) -> Result<AppConfigView, AppError> {
    let chosen = path.map(|p| p.trim().to_string()).filter(|p| !p.is_empty());
    let new_dir = match &chosen {
        None => state.default_backup_dir.clone(),
        Some(p) => {
            let dir = std::path::PathBuf::from(p);
            std::fs::create_dir_all(&dir)
                .map_err(|e| AppError::IoError(format!("Cannot create backup folder '{p}': {e}")))?;
            let probe = dir.join(format!(".bk_write_test_{}", std::process::id()));
            std::fs::write(&probe, b"ok")
                .map_err(|e| AppError::IoError(format!("Backup folder '{p}' is not writable: {e}")))?;
            let _ = std::fs::remove_file(&probe);
            dir
        }
    };
    let mut tx = state.unit_of_work.begin_transaction()?;
    tx.settings().set_backup_directory(chosen.as_deref())?;
    tx.commit()?;
    *state.backup_dir.write().unwrap_or_else(|e| e.into_inner()) = new_dir;
    Ok(build_config_view(state))
}

#[tauri::command]
pub async fn get_app_config(state: State<'_, AppState>) -> Result<AppConfigView, AppError> {
    Ok(build_config_view(&state))
}
#[tauri::command]
pub async fn set_backup_directory(state: State<'_, AppState>, path: Option<String>) -> Result<AppConfigView, AppError> {
    let state = state.inner();
    // Filesystem + DB work: keep it off the async workers.
    tokio::task::block_in_place(|| apply_backup_directory(state, path))
}

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
    // Creates the folder if missing (the default `<app data>/backups` may not exist yet).
    super::run_blocking(move || svc.backup_to_dir(&destination_path, BackupKind::Manual)).await
}
#[tauri::command]
pub async fn verify_database_integrity(state: State<'_, AppState>) -> Result<bool, AppError> {
    let svc = state.backup_service.clone();
    super::run_blocking(move || svc.verify_database_integrity()).await
}
