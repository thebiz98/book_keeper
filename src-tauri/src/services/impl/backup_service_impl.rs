use std::path::{Path, PathBuf};
use crate::core::error::AppError;
use crate::services::contracts::backup_service::BackupService;

#[derive(Clone, Debug)]
pub struct BackupServiceImpl {
    pub db_path: PathBuf,
}

impl BackupServiceImpl {
    pub fn new<P: Into<PathBuf>>(db_path: P) -> Self {
        Self {
            db_path: db_path.into(),
        }
    }
}

impl BackupService for BackupServiceImpl {
    fn backup_database(&self, destination_path: &str) -> Result<String, AppError> {
        let dest = Path::new(destination_path);
        let final_dest_path: PathBuf = if dest.is_dir() || !destination_path.ends_with(".db") {
            let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
            let file_name = format!("book_keeper_backup_{}.db", timestamp);
            dest.join(file_name)
        } else {
            dest.to_path_buf()
        };

        if let Some(parent) = final_dest_path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent).map_err(|e| AppError::IoError(e.to_string()))?;
            }
        }

        let src_conn = rusqlite::Connection::open(&self.db_path)
            .map_err(|e| AppError::DatabaseError(format!("Failed to open source db: {}", e)))?;
        let mut dst_conn = rusqlite::Connection::open(&final_dest_path)
            .map_err(|e| AppError::DatabaseError(format!("Failed to open destination db: {}", e)))?;

        {
            let backup = rusqlite::backup::Backup::new(&src_conn, &mut dst_conn)
                .map_err(|e| AppError::DatabaseError(format!("Backup initialization failed: {}", e)))?;

            backup
                .run_to_completion(5, std::time::Duration::from_millis(100), None)
                .map_err(|e| AppError::DatabaseError(format!("Backup execution failed: {}", e)))?;
        }

        // Verify destination integrity after backup handle is dropped
        let integrity: String = dst_conn
            .query_row("PRAGMA integrity_check;", [], |row| row.get(0))
            .map_err(|e| AppError::DatabaseError(format!("Backup integrity check failed: {}", e)))?;

        if integrity != "ok" {
            return Err(AppError::DatabaseError(format!(
                "Backup database failed integrity check: {}",
                integrity
            )));
        }

        let absolute_path = final_dest_path
            .canonicalize()
            .unwrap_or(final_dest_path)
            .to_string_lossy()
            .to_string();

        Ok(absolute_path)
    }

    fn verify_database_integrity(&self) -> Result<bool, AppError> {
        if !self.db_path.exists() && self.db_path.to_str() != Some(":memory:") {
            return Ok(false);
        }
        let conn = rusqlite::Connection::open_with_flags(
            &self.db_path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        let integrity: String = conn
            .query_row("PRAGMA integrity_check;", [], |row| row.get(0))
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        Ok(integrity == "ok")
    }
}
