use crate::app_state::AppState;
use tauri::State;
use crate::core::error::AppError;
use crate::core::entities::ShopSetting;
use serde::Serialize;
use crate::services::contracts::backup_service::BackupKind;
use crate::services::r#impl::restore::{self, BackupFileInfo, RestorePreview};

/// Runtime config shown on the Settings page.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppConfigView {
    pub testing_mode: bool,
    pub backup_directory: String,
    pub is_default_directory: bool,
    pub last_auto_backup_at: Option<String>,
    /// IANA shop time zone from app config; the UI formats stamps in it.
    pub time_zone: String,
    /// Why the last restore failed (FR-60); shown once until dismissed. Filled by the commands.
    pub restore_failure: Option<String>,
}

/// The app data folder (where the restore marker and staged file live). Managed by `main.rs`.
pub struct AppDataDir(pub std::path::PathBuf);

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
            .and_then(|t| {
                use chrono::TimeZone;
                crate::core::clock::shop_tz()
                    .from_local_datetime(&t)
                    .earliest()
                    .map(|d| d.with_timezone(&chrono::Utc).to_rfc3339())
            }),
        time_zone: state.config.time_zone.clone(),
        restore_failure: None,
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
pub async fn get_app_config(state: State<'_, AppState>, data_dir: State<'_, AppDataDir>) -> Result<AppConfigView, AppError> {
    let mut view = build_config_view(&state);
    view.restore_failure = crate::services::r#impl::restore::restore_failure(&data_dir.0);
    Ok(view)
}
#[tauri::command]
pub async fn dismiss_restore_failure(data_dir: State<'_, AppDataDir>) -> Result<(), AppError> {
    crate::services::r#impl::restore::dismiss_restore_failure(&data_dir.0);
    Ok(())
}
#[tauri::command]
pub async fn list_backups(state: State<'_, AppState>) -> Result<Vec<BackupFileInfo>, AppError> {
    let dir = state.backup_dir.read().unwrap_or_else(|e| e.into_inner()).clone();
    super::run_blocking(move || restore::list_backups(&dir)).await
}
#[tauri::command]
pub async fn preview_restore(path: String) -> Result<RestorePreview, AppError> {
    super::run_blocking(move || restore::preview_restore(std::path::Path::new(&path))).await
}
/// Stages the restore and restarts the app shortly after, so the UI gets this answer first.
/// The swap happens at the next start-up (DD-99).
#[tauri::command]
pub async fn stage_restore(
    app: tauri::AppHandle,
    data_dir: State<'_, AppDataDir>,
    path: String,
) -> Result<(), AppError> {
    let dir = data_dir.0.clone();
    super::run_blocking(move || restore::stage_restore(&dir, std::path::Path::new(&path)).map(|_| ())).await?;
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(1500));
        app.restart();
    });
    Ok(())
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
