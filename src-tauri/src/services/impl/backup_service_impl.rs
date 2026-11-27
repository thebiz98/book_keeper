use std::path::{Path, PathBuf};
use crate::core::error::AppError;
use crate::services::contracts::backup_service::{BackupKind, BackupService};

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

impl BackupServiceImpl {
    /// Validated, never-overwriting backup of the live DB into the existing `destination_path`.
    fn backup_into(&self, destination_path: &str, kind: BackupKind) -> Result<String, AppError> {
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
        let stamp = crate::core::clock::now_local().format("%Y%m%d_%H%M");
        let mut n = 1u32;
        let final_dest_path: PathBuf = loop {
            let prefix = kind.prefix();
            let name = if n == 1 {
                format!("{prefix}{stamp}.db")
            } else {
                format!("{prefix}{stamp}_{n}.db")
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
}

impl BackupService for BackupServiceImpl {
    fn backup_database(&self, destination_path: &str) -> Result<String, AppError> {
        self.backup_into(destination_path, BackupKind::Manual)
    }

    fn backup_to_dir(&self, dir: &str, kind: BackupKind) -> Result<String, AppError> {
        std::fs::create_dir_all(dir)
            .map_err(|e| AppError::IoError(format!("Cannot create backup folder '{dir}': {e}")))?;
        self.backup_into(dir, kind)
    }

    fn rotate(&self, dir: &str, retention_days: u32) -> Result<Vec<String>, AppError> {
        let names = list_file_names(dir)?;
        let now = crate::core::clock::now_local().naive_local();
        let doomed = select_for_deletion(&names, now, retention_days);
        let mut deleted = Vec::new();
        for name in doomed {
            match std::fs::remove_file(Path::new(dir).join(&name)) {
                Ok(()) => deleted.push(name),
                Err(e) => eprintln!("[backup] could not delete old backup {name}: {e}"),
            }
        }
        Ok(deleted)
    }

    fn last_auto_backup(&self, dir: &str) -> Result<Option<chrono::NaiveDateTime>, AppError> {
        Ok(list_file_names(dir)?
            .iter()
            .filter_map(|n| parse_stamp(n, BackupKind::Auto))
            .max())
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

/// File names directly inside `dir`; an absent folder has no files.
fn list_file_names(dir: &str) -> Result<Vec<String>, AppError> {
    let rd = match std::fs::read_dir(dir) {
        Ok(rd) => rd,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(AppError::IoError(format!("Cannot read backup folder '{dir}': {e}"))),
    };
    Ok(rd
        .filter_map(Result::ok)
        .filter(|e| e.file_type().map(|t| t.is_file()).unwrap_or(false))
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect())
}

/// Stamp of a backup file name of `kind`: `<prefix>YYYYMMDD_HHMM[_n].db`. `None` for other files.
pub fn parse_stamp(name: &str, kind: BackupKind) -> Option<chrono::NaiveDateTime> {
    let mut rest = name.strip_prefix(kind.prefix())?.strip_suffix(".db")?;
    if kind == BackupKind::PreMigration {
        // `v{from}_to_v{to}_` precedes the stamp in pre-migration file names (optional here).
        if let Some(r) = rest.strip_prefix('v') {
            let (from, r) = r.split_once("_to_v")?;
            let (to, r) = r.split_once('_')?;
            if from.is_empty() || to.is_empty() || !from.chars().chain(to.chars()).all(|c| c.is_ascii_digit()) {
                return None;
            }
            rest = r;
        }
    }
    let stamp = rest.get(..13)?;
    let tail = &rest[13..];
    if !(tail.is_empty() || (tail.starts_with('_') && tail.len() > 1 && tail[1..].chars().all(|c| c.is_ascii_digit()))) {
        return None;
    }
    chrono::NaiveDateTime::parse_from_str(stamp, "%Y%m%d_%H%M").ok()
}

/// Pure rotation rule: auto, pre-reset and pre-migration backups older than `retention_days` before `now`,
/// except the newest auto backup. Manual and unrecognised files are never selected.
pub fn select_for_deletion(names: &[String], now: chrono::NaiveDateTime, retention_days: u32) -> Vec<String> {
    let cutoff = now - chrono::Duration::days(i64::from(retention_days));
    let newest_auto = names
        .iter()
        .filter_map(|n| parse_stamp(n, BackupKind::Auto).map(|t| (t, n)))
        .max()
        .map(|(_, n)| n.clone());
    let mut doomed: Vec<String> = names
        .iter()
        .filter(|n| Some(*n) != newest_auto.as_ref())
        .filter(|n| {
            [BackupKind::Auto, BackupKind::PreReset, BackupKind::PreMigration]
                .iter()
                .any(|k| parse_stamp(n, *k).is_some_and(|t| t < cutoff))
        })
        .cloned()
        .collect();
    doomed.sort();
    doomed
}
