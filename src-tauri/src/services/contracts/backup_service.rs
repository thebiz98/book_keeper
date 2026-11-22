use crate::core::error::AppError;

/// What produced a backup file; decides its file-name prefix and whether rotation may delete it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackupKind {
    /// User-triggered (Backup Now / Ctrl+B). Never deleted by rotation.
    Manual,
    /// Scheduled or on-close backup. Rotated after `retentionDays`; the newest is always kept.
    Auto,
    /// Written right before a testing data reset. Rotated after `retentionDays`.
    PreReset,
}

impl BackupKind {
    pub fn prefix(self) -> &'static str {
        match self {
            BackupKind::Manual => "book_keeper_backup_",
            BackupKind::Auto => "book_keeper_auto_",
            BackupKind::PreReset => "book_keeper_prereset_",
        }
    }
}

pub trait BackupService: Send + Sync {
    /// Creates a clean online snapshot of the active database in the existing directory
    /// `destination_path` (a manual backup). Never creates directories.
    fn backup_database(&self, destination_path: &str) -> Result<String, AppError>;

    /// Like `backup_database`, but creates `dir` if missing and names the file by `kind`.
    fn backup_to_dir(&self, dir: &str, kind: BackupKind) -> Result<String, AppError>;

    /// Deletes auto and pre-reset backups in `dir` older than `retention_days` (by the stamp in
    /// their name). Always keeps the newest auto backup; never touches manual or unknown files.
    /// Returns the deleted file names.
    fn rotate(&self, dir: &str, retention_days: u32) -> Result<Vec<String>, AppError>;

    /// Stamp (local time) of the newest auto backup in `dir`, if any.
    fn last_auto_backup(&self, dir: &str) -> Result<Option<chrono::NaiveDateTime>, AppError>;

    /// Runs `PRAGMA integrity_check` and returns true if the database is healthy.
    fn verify_database_integrity(&self) -> Result<bool, AppError>;
}

impl<T: BackupService + ?Sized> BackupService for std::sync::Arc<T> {
    fn backup_database(&self, destination_path: &str) -> Result<String, AppError> {
        (**self).backup_database(destination_path)
    }

    fn backup_to_dir(&self, dir: &str, kind: BackupKind) -> Result<String, AppError> {
        (**self).backup_to_dir(dir, kind)
    }

    fn rotate(&self, dir: &str, retention_days: u32) -> Result<Vec<String>, AppError> {
        (**self).rotate(dir, retention_days)
    }

    fn last_auto_backup(&self, dir: &str) -> Result<Option<chrono::NaiveDateTime>, AppError> {
        (**self).last_auto_backup(dir)
    }

    fn verify_database_integrity(&self) -> Result<bool, AppError> {
        (**self).verify_database_integrity()
    }
}
