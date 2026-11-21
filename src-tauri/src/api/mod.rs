pub mod adjustment_controller;
pub mod dues_controller;
pub mod item_controller;
pub mod import_controller;
pub mod party_controller;
pub mod purchase_controller;
pub mod report_controller;
pub mod return_controller;
pub mod sale_controller;
pub mod settings_controller;

use crate::core::error::AppError;

/// Runs a blocking service call off the async runtime's worker threads.
/// Join failures (panic/cancel) surface as `AppError::DatabaseError("task join: ...")`.
pub(crate) async fn run_blocking<T, F>(f: F) -> Result<T, AppError>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, AppError> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| AppError::DatabaseError(format!("task join: {e}")))?
}
