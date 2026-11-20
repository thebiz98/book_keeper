use crate::core::dtos::{BackupFileInfo, MigrationPreview, MigrationResult};
use crate::core::error::AppError;

pub trait MigrationService: Send + Sync {
    /// Scans default directory (Documents/VEDBIL) for available .vb backup files.
    fn scan_backup_directory(&self) -> Result<Vec<BackupFileInfo>, AppError>;

    /// Inspects a .vb archive and returns company info and table counts without importing.
    fn inspect_backup_file(&self, archive_path: &str) -> Result<MigrationPreview, AppError>;

    /// Executes the full atomic migration of items, parties, and transactions into active DB.
    fn execute_migration(&self, archive_path: &str) -> Result<MigrationResult, AppError>;
}
