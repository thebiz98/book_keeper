use crate::core::error::AppError;

pub trait BackupService: Send + Sync {
    /// Creates a clean online snapshot of the active database to destination path.
    fn backup_database(&self, destination_path: &str) -> Result<String, AppError>;

    /// Runs `PRAGMA integrity_check` and returns true if the database is healthy.
    fn verify_database_integrity(&self) -> Result<bool, AppError>;
}
