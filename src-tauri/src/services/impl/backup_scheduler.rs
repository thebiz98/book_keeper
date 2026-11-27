//! Automatic backups: one check at startup, then hourly; plus an optional backup on close.
//! Errors are logged and never stop the app.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use crate::config::BackupConfig;
use crate::core::error::AppError;
use crate::services::contracts::backup_service::{BackupKind, BackupService};
use crate::services::r#impl::backup_service_impl::parse_stamp;

/// How often the background thread wakes to check whether a backup is due.
pub const CHECK_EVERY: Duration = Duration::from_secs(60 * 60);

/// Pure: an auto backup is due when there is none yet, or the newest is `interval_hours` old.
pub fn is_due(last: Option<chrono::NaiveDateTime>, now: chrono::NaiveDateTime, interval_hours: u32) -> bool {
    match last {
        None => true,
        Some(t) => now - t >= chrono::Duration::hours(i64::from(interval_hours)),
    }
}

fn mtime(p: &Path) -> Option<SystemTime> {
    std::fs::metadata(p).and_then(|m| m.modified()).ok()
}

/// Modification time of the newest auto backup file in `dir`.
fn newest_auto_mtime(dir: &Path) -> Option<SystemTime> {
    std::fs::read_dir(dir)
        .ok()?
        .filter_map(Result::ok)
        .filter(|e| parse_stamp(&e.file_name().to_string_lossy(), BackupKind::Auto).is_some())
        .filter_map(|e| mtime(&e.path()))
        .max()
}

/// True when the database (`shop.db` or its `-wal`) was modified after the newest auto backup
/// in `dir` was written, or when there is no auto backup yet.
pub fn db_changed_since_last_auto(db_path: &Path, dir: &Path) -> bool {
    let Some(backup_t) = newest_auto_mtime(dir) else { return true };
    let mut wal = db_path.as_os_str().to_owned();
    wal.push("-wal");
    let db_t = [mtime(db_path), mtime(Path::new(&wal))].into_iter().flatten().max();
    db_t.is_none_or(|t| t > backup_t)
}

/// Writes an auto backup into `dir` and rotates. Returns the new file path.
pub fn run_auto_backup(svc: &dyn BackupService, dir: &Path, retention_days: u32) -> Result<String, AppError> {
    let dir_s = dir.to_string_lossy();
    let path = svc.backup_to_dir(&dir_s, BackupKind::Auto)?;
    let deleted = svc.rotate(&dir_s, retention_days)?;
    if !deleted.is_empty() {
        eprintln!("[backup] rotation removed {} old backup(s): {}", deleted.len(), deleted.join(", "));
    }
    Ok(path)
}

/// Scheduled check: back up when due. Logs instead of failing.
pub fn check_and_backup(svc: &dyn BackupService, dir: &Path, cfg: &BackupConfig) {
    let last = match svc.last_auto_backup(&dir.to_string_lossy()) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("[backup] cannot read backup folder {}: {e}", dir.display());
            return;
        }
    };
    if !is_due(last, crate::core::clock::now_local().naive_local(), cfg.interval_hours) {
        return;
    }
    match run_auto_backup(svc, dir, cfg.retention_days) {
        Ok(p) => eprintln!("[backup] auto backup written: {p}"),
        Err(e) => eprintln!("[backup] auto backup failed: {e}"),
    }
}

/// On-close backup: skipped when disabled or when nothing changed since the last auto backup.
pub fn backup_on_exit(svc: &dyn BackupService, db_path: &Path, dir: &Path, cfg: &BackupConfig) {
    if !cfg.on_exit || !db_changed_since_last_auto(db_path, dir) {
        return;
    }
    match run_auto_backup(svc, dir, cfg.retention_days) {
        Ok(p) => eprintln!("[backup] on-close backup written: {p}"),
        Err(e) => eprintln!("[backup] on-close backup failed: {e}"),
    }
}

/// Startup check, then an hourly background check. `current_dir` is read each time so a folder
/// change in Settings takes effect without a restart.
pub fn spawn(
    svc: Arc<dyn BackupService>,
    cfg: BackupConfig,
    current_dir: impl Fn() -> PathBuf + Send + 'static,
) -> std::thread::JoinHandle<()> {
    std::thread::Builder::new()
        .name("auto-backup".into())
        .spawn(move || loop {
            check_and_backup(&*svc, &current_dir(), &cfg);
            std::thread::sleep(CHECK_EVERY);
        })
        .expect("failed to spawn auto-backup thread")
}
