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

fn integrity_ok(conn: &rusqlite::Connection) -> Result<String, AppError> {
    conn.query_row("PRAGMA integrity_check;", [], |row| row.get(0))
        .map_err(|e| AppError::DatabaseError(format!("Integrity check failed: {e}")))
}

impl BackupServiceImpl {
    /// Online backup of the live database into the (reserved, empty) `dest`, verifying both ends.
    fn copy_and_verify(&self, dest: &Path) -> Result<(), AppError> {
        let src_conn = rusqlite::Connection::open(&self.db_path)
            .map_err(|e| AppError::DatabaseError(format!("Failed to open source db: {e}")))?;
        let src_state = integrity_ok(&src_conn)?;
        if src_state != "ok" {
            return Err(AppError::DatabaseError(format!(
                "Source database failed integrity check: {src_state}"
            )));
        }
        let mut dst_conn = rusqlite::Connection::open(dest)
            .map_err(|e| AppError::DatabaseError(format!("Failed to open destination db: {e}")))?;
        {
            let backup = rusqlite::backup::Backup::new(&src_conn, &mut dst_conn)
                .map_err(|e| AppError::DatabaseError(format!("Backup initialization failed: {e}")))?;
            backup
                .run_to_completion(5, std::time::Duration::from_millis(100), None)
                .map_err(|e| AppError::DatabaseError(format!("Backup execution failed: {e}")))?;
        }
        let dst_state = integrity_ok(&dst_conn)?;
        if dst_state != "ok" {
            return Err(AppError::DatabaseError(format!(
                "Backup database failed integrity check: {dst_state}"
            )));
        }
        Ok(())
    }
}

impl BackupService for BackupServiceImpl {
    fn backup_database(&self, destination_path: &str) -> Result<String, AppError> {
        let dest_dir = Path::new(destination_path);
        let meta = std::fs::metadata(dest_dir).map_err(|e| {
            AppError::IoError(format!("Backup destination '{destination_path}' is not accessible: {e}"))
        })?;
        if !meta.is_dir() {
            return Err(AppError::IoError(format!(
                "Backup destination '{destination_path}' must be an existing directory"
            )));
        }
        if meta.permissions().readonly() {
            return Err(AppError::IoError(format!(
                "Backup destination '{destination_path}' is not writable"
            )));
        }
        let live = self.db_path.canonicalize().unwrap_or_else(|_| self.db_path.clone());
        if dest_dir.canonicalize().ok().as_deref() == Some(live.as_path()) {
            return Err(AppError::IoError("Backup destination cannot be the live database".into()));
        }

        // Local-time stamp; `_2`, `_3`, ... on collision. `create_new` reserves the name
        // atomically so an existing backup is never overwritten.
        let stamp = chrono::Local::now().format("%Y%m%d_%H%M");
        let mut n = 1u32;
        let final_dest_path: PathBuf = loop {
            let name = if n == 1 {
                format!("book_keeper_backup_{stamp}.db")
            } else {
                format!("book_keeper_backup_{stamp}_{n}.db")
            };
            let candidate = dest_dir.join(name);
            match std::fs::OpenOptions::new().write(true).create_new(true).open(&candidate) {
                Ok(_) => break candidate,
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => n += 1,
                Err(e) => {
                    return Err(AppError::IoError(format!("Cannot write to backup destination: {e}")))
                }
            }
        };
        if final_dest_path.canonicalize().ok().as_deref() == Some(live.as_path()) {
            let _ = std::fs::remove_file(&final_dest_path);
            return Err(AppError::IoError("Backup target cannot be the live database".into()));
        }

        let result = self.copy_and_verify(&final_dest_path);
        if let Err(e) = result {
            let _ = std::fs::remove_file(&final_dest_path);
            return Err(e);
        }

        Ok(final_dest_path
            .canonicalize()
            .unwrap_or(final_dest_path)
            .to_string_lossy()
            .to_string())
    }

    fn verify_database_integrity(&self) -> Result<bool, AppError> {
        if !self.db_path.exists() && self.db_path.to_str() != Some(":memory:") {
            return Ok(false);
        }
        // Read-write (without CREATE): the FTS5 integrity check needs a writable handle and
        // reports a false failure on a read-only one. `integrity_check` itself writes nothing.
        let conn = rusqlite::Connection::open_with_flags(
            &self.db_path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE,
        )
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        let integrity: String = conn
            .query_row("PRAGMA integrity_check;", [], |row| row.get(0))
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        Ok(integrity == "ok")
    }
}
