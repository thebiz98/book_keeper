//! Restore from backup (FR-60, DD-99): list and preview backup files, stage one, and swap it in
//! at the next start-up before any connection to `shop.db` exists.

use std::path::{Path, PathBuf};

use chrono::TimeZone;
use serde::{Deserialize, Serialize};

use crate::core::error::AppError;
use crate::data::sqlite::migrations::{current_version, latest_version};
use crate::services::contracts::backup_service::BackupKind;
use crate::services::r#impl::backup_service_impl::parse_stamp;

pub const STAGED_FILE: &str = "restore_staged.db";
pub const PENDING_MARKER: &str = "restore_pending.json";
pub const FAILED_MARKER: &str = "restore_failed.json";

const KINDS: [(BackupKind, &str); 5] = [
    (BackupKind::Auto, "AUTO"),
    (BackupKind::Manual, "MANUAL"),
    (BackupKind::PreReset, "PRE_RESET"),
    (BackupKind::PreMigration, "PRE_MIGRATION"),
    (BackupKind::PreRestore, "PRE_RESTORE"),
];

const NOT_A_BACKUP: &str = "This is not a Book Keeper backup file.";
const LIVE_DB: &str = "This is the database in use, not a backup.";

/// One file in the backup folder.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupFileInfo {
    pub path: String,
    pub file_name: String,
    /// `AUTO`, `MANUAL`, `PRE_RESET`, `PRE_MIGRATION` or `PRE_RESTORE`.
    pub kind: String,
    /// When the backup was taken (UTC, RFC 3339); the UI formats it in the shop time zone.
    pub stamp: String,
    pub size_bytes: u64,
}

/// What the backend found in a candidate backup file. `refusal` is set when it can't be restored.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RestorePreview {
    pub path: String,
    pub schema_version: i64,
    pub integrity_ok: bool,
    pub newer_than_app: bool,
    pub item_count: i64,
    pub sale_bill_count: i64,
    pub last_bill_date: Option<String>,
    pub refusal: Option<String>,
}

#[derive(Debug, Clone)]
pub struct RestoreOutcome {
    /// The "Before restore" safety copy, when a live database existed.
    pub safety_copy: Option<PathBuf>,
}

#[derive(Debug, Serialize, Deserialize)]
struct PendingMarker {
    staged: String,
    source: String,
    requested_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct FailedMarker {
    reason: String,
    source: String,
    failed_at: String,
}

fn io_err(what: &str, e: impl std::fmt::Display) -> AppError {
    AppError::IoError(format!("{what}: {e}"))
}

/// Backup files in `dir`, newest first. An absent folder has none.
pub fn list_backups(dir: &Path) -> Result<Vec<BackupFileInfo>, AppError> {
    let rd = match std::fs::read_dir(dir) {
        Ok(rd) => rd,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(io_err(&format!("Cannot read backup folder '{}'", dir.display()), e)),
    };
    let mut rows: Vec<(chrono::NaiveDateTime, BackupFileInfo)> = Vec::new();
    for e in rd.filter_map(Result::ok) {
        let Ok(meta) = e.metadata() else { continue };
        if !meta.is_file() {
            continue;
        }
        let name = e.file_name().to_string_lossy().to_string();
        let Some((t, label)) = KINDS.iter().find_map(|(k, l)| parse_stamp(&name, *k).map(|t| (t, *l))) else {
            continue;
        };
        let stamp = crate::core::clock::shop_tz()
            .from_local_datetime(&t)
            .earliest()
            .map(|d| d.with_timezone(&chrono::Utc).to_rfc3339())
            .unwrap_or_default();
        rows.push((
            t,
            BackupFileInfo {
                path: e.path().to_string_lossy().into_owned(),
                file_name: name,
                kind: label.to_string(),
                stamp,
                size_bytes: meta.len(),
            },
        ));
    }
    rows.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| b.1.file_name.cmp(&a.1.file_name)));
    Ok(rows.into_iter().map(|(_, i)| i).collect())
}

fn has_column(conn: &rusqlite::Connection, table: &str, col: &str) -> bool {
    conn.query_row("SELECT COUNT(*) FROM pragma_table_info(?1) WHERE name = ?2", [table, col], |r| {
        r.get::<_, i64>(0)
    })
    .map(|n| n > 0)
    .unwrap_or(false)
}

fn refused(path: &Path, schema_version: i64, integrity_ok: bool, newer: bool, reason: &str) -> RestorePreview {
    RestorePreview {
        path: path.to_string_lossy().into_owned(),
        schema_version,
        integrity_ok,
        newer_than_app: newer,
        item_count: 0,
        sale_bill_count: 0,
        last_bill_date: None,
        refusal: Some(reason.to_string()),
    }
}

/// True when `path` is the live database, or a file in the app data folder that the app itself
/// owns (`restore_staged.db`, `shop.db` and its `-wal` / `-shm` / `.restoring` siblings).
pub fn is_live_file(path: &Path, live_db: &Path, app_data_dir: &Path) -> bool {
    let Ok(p) = path.canonicalize() else { return false };
    if live_db.canonicalize().is_ok_and(|l| l == p) {
        return true;
    }
    let in_app_dir = p.parent().zip(app_data_dir.canonicalize().ok()).is_some_and(|(a, b)| a == b);
    let name = p.file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default();
    in_app_dir && (name == STAGED_FILE || name.starts_with("shop.db"))
}

/// `preview_restore`, but a source that is the database in use is refused first.
pub fn preview_restore_for(path: &Path, live_db: &Path, app_data_dir: &Path) -> Result<RestorePreview, AppError> {
    if is_live_file(path, live_db, app_data_dir) {
        return Ok(refused(path, 0, false, false, LIVE_DB));
    }
    preview_restore(path)
}

/// Opens `path` without modifying it, checks it and counts what the owner will recognise. Never writes.
pub fn preview_restore(path: &Path) -> Result<RestorePreview, AppError> {
    if !path.is_file() {
        return Ok(refused(path, 0, false, false, "That file does not exist."));
    }
    // The FTS5 integrity check reports a false failure on a read-only handle (see
    // `verify_database_integrity`), so open read-write (no create; nothing is written) and
    // fall back to read-only with `quick_check` for a file on read-only media.
    let (conn, check) = match rusqlite::Connection::open_with_flags(
        path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    ) {
        Ok(c) => (c, "PRAGMA integrity_check;"),
        Err(_) => match rusqlite::Connection::open_with_flags(
            path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
        ) {
            Ok(c) => (c, "PRAGMA quick_check;"),
            Err(_) => return Ok(refused(path, 0, false, false, NOT_A_BACKUP)),
        },
    };
    let Ok(integrity) = conn.query_row(check, [], |r| r.get::<_, String>(0)) else {
        // Not a database at all (or unreadable).
        return Ok(refused(path, 0, false, false, NOT_A_BACKUP));
    };
    let integrity_ok = integrity == "ok";
    let schema_version = current_version(&conn).unwrap_or(0);
    if schema_version == 0 {
        return Ok(refused(path, 0, integrity_ok, false, NOT_A_BACKUP));
    }
    let newer = schema_version > latest_version();
    if !integrity_ok {
        return Ok(refused(path, schema_version, false, newer, "This backup is damaged (the integrity check failed)."));
    }
    if newer {
        return Ok(refused(
            path,
            schema_version,
            true,
            true,
            "This backup was made by a newer version of Book Keeper. Update the app first.",
        ));
    }
    let count = |sql: &str| -> i64 { conn.query_row(sql, [], |r| r.get(0)).unwrap_or(0) };
    let item_count = count("SELECT COUNT(*) FROM items WHERE is_active = 1");
    let sale_filter = if has_column(&conn, "sale_transactions", "is_opening_balance") {
        " WHERE is_opening_balance = 0"
    } else {
        ""
    };
    let sale_bill_count = count(&format!("SELECT COUNT(*) FROM sale_transactions{sale_filter}"));
    let last_bill_date: Option<String> = conn
        .query_row(
            "SELECT MAX(d) FROM (SELECT MAX(transaction_date) AS d FROM sale_transactions
                                 UNION ALL SELECT MAX(transaction_date) FROM purchase_transactions)",
            [],
            |r| r.get(0),
        )
        .unwrap_or(None);
    Ok(RestorePreview {
        path: path.to_string_lossy().into_owned(),
        schema_version,
        integrity_ok: true,
        newer_than_app: false,
        item_count,
        sale_bill_count,
        last_bill_date,
        refusal: None,
    })
}

fn now_rfc3339() -> String {
    chrono::Utc::now().to_rfc3339()
}

/// Checks `source`, copies it next to the database as `restore_staged.db` and writes the
/// pending marker. The swap itself happens at the next start-up.
pub fn stage_restore(app_data_dir: &Path, live_db: &Path, source: &Path) -> Result<RestorePreview, AppError> {
    let preview = preview_restore_for(source, live_db, app_data_dir)?;
    if let Some(reason) = &preview.refusal {
        return Err(AppError::ValidationError { field: "path".into(), message: reason.clone() });
    }
    std::fs::create_dir_all(app_data_dir).map_err(|e| io_err("Cannot prepare the restore", e))?;
    let staged = app_data_dir.join(STAGED_FILE);
    let tmp = app_data_dir.join(format!("{STAGED_FILE}.tmp"));
    let _ = std::fs::remove_file(&tmp);
    let copy = std::fs::copy(source, &tmp)
        .map_err(|e| io_err("Cannot copy the backup", e))
        .and_then(|_| {
            // Re-check the copy, not just the original.
            match preview_restore(&tmp)?.refusal {
                Some(r) => Err(AppError::IoError(format!("The copied backup failed its check: {r}"))),
                None => Ok(()),
            }
        })
        .and_then(|_| {
            let _ = std::fs::remove_file(&staged);
            std::fs::rename(&tmp, &staged).map_err(|e| io_err("Cannot stage the backup", e))
        });
    if let Err(e) = copy {
        let _ = std::fs::remove_file(&tmp);
        return Err(e);
    }
    let marker = PendingMarker {
        staged: staged.to_string_lossy().into_owned(),
        source: source.to_string_lossy().into_owned(),
        requested_at: now_rfc3339(),
    };
    let json = serde_json::to_string_pretty(&marker).map_err(|e| io_err("Cannot write the restore marker", e))?;
    if let Err(e) = std::fs::write(app_data_dir.join(PENDING_MARKER), json) {
        let _ = std::fs::remove_file(&staged);
        return Err(io_err("Cannot write the restore marker", e));
    }
    Ok(preview)
}

/// Runs `f` until it succeeds, up to `attempts` times with `delay` between tries. An antivirus
/// scan or indexer can hold a file for a moment; the last error is returned if it never frees up.
pub fn retry_io<T>(
    attempts: u32,
    delay: std::time::Duration,
    mut f: impl FnMut() -> std::io::Result<T>,
) -> std::io::Result<T> {
    let mut n = 1;
    loop {
        match f() {
            Ok(v) => return Ok(v),
            Err(e) if n >= attempts => return Err(e),
            Err(_) => {
                n += 1;
                std::thread::sleep(delay);
            }
        }
    }
}

/// About 10 s in total.
const LOCK_RETRIES: u32 = 40;
const LOCK_RETRY_DELAY: std::time::Duration = std::time::Duration::from_millis(250);

fn remove_sidecars(db: &Path) {
    for suffix in ["-wal", "-shm"] {
        let mut s = db.as_os_str().to_owned();
        s.push(suffix);
        let p = PathBuf::from(s);
        let _ = retry_io(LOCK_RETRIES, LOCK_RETRY_DELAY, || match std::fs::remove_file(&p) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            other => other,
        });
    }
}

/// Replaces `target` with a copy of `from` (copy to `<target>.restoring`, then rename over it).
fn replace_file(from: &Path, target: &Path) -> Result<(), AppError> {
    let mut tmp_name = target.as_os_str().to_owned();
    tmp_name.push(".restoring");
    let tmp = PathBuf::from(tmp_name);
    let _ = std::fs::remove_file(&tmp);
    let result = std::fs::copy(from, &tmp)
        .map_err(|e| io_err("Cannot copy the backup into place", e))
        .and_then(|_| {
            retry_io(LOCK_RETRIES, LOCK_RETRY_DELAY, || std::fs::rename(&tmp, target))
                .map_err(|e| io_err("Cannot replace the database file", e))
        });
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result?;
    remove_sidecars(target);
    Ok(())
}

/// The folder chosen in Settings, read from the live DB with a plain connection; `default_dir`
/// when it is unset, unreadable or the column does not exist yet.
fn backup_dir_from_live(db_path: &Path, default_dir: &Path) -> PathBuf {
    let from_db = (|| -> Option<PathBuf> {
        let conn = rusqlite::Connection::open_with_flags(db_path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).ok()?;
        if !has_column(&conn, "shop_settings", "backup_directory") {
            return None;
        }
        let v: Option<String> =
            conn.query_row("SELECT backup_directory FROM shop_settings WHERE id = 1", [], |r| r.get(0)).ok()?;
        let p = PathBuf::from(v?.trim());
        (!p.as_os_str().is_empty()).then_some(p)
    })();
    from_db.unwrap_or_else(|| default_dir.to_path_buf())
}

/// `VACUUM INTO` a new `book_keeper_prerestore_*` file in `dir` after a full checkpoint.
fn safety_copy(db_path: &Path, dir: &Path) -> Result<PathBuf, AppError> {
    std::fs::create_dir_all(dir).map_err(|e| io_err(&format!("Cannot create backup folder '{}'", dir.display()), e))?;
    let stamp = crate::core::clock::now_local().format("%Y%m%d_%H%M");
    let prefix = BackupKind::PreRestore.prefix();
    let mut n = 1u32;
    let dest = loop {
        let name = if n == 1 { format!("{prefix}{stamp}.db") } else { format!("{prefix}{stamp}_{n}.db") };
        let c = dir.join(name);
        if !c.exists() {
            break c;
        }
        n += 1;
    };
    let conn = rusqlite::Connection::open_with_flags(db_path, rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE)
        .map_err(|e| io_err("Cannot open the current database for the safety copy", e))?;
    let run = || -> Result<(), rusqlite::Error> {
        conn.query_row("PRAGMA wal_checkpoint(TRUNCATE);", [], |_| Ok(()))?;
        let escaped = dest.to_string_lossy().replace('\'', "''");
        conn.execute_batch(&format!("VACUUM INTO '{escaped}';"))
    };
    if let Err(e) = run() {
        let _ = std::fs::remove_file(&dest);
        return Err(io_err("Cannot write the 'Before restore' copy", e));
    }
    drop(conn);
    Ok(dest)
}

fn integrity_of(db: &Path) -> Result<(), AppError> {
    let conn = rusqlite::Connection::open_with_flags(db, rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE)
        .map_err(|e| io_err("Cannot open the restored database", e))?;
    let s: String = conn
        .query_row("PRAGMA integrity_check;", [], |r| r.get(0))
        .map_err(|e| io_err("Integrity check failed", e))?;
    if s == "ok" {
        Ok(())
    } else {
        Err(AppError::DatabaseError(format!("Restored database failed its integrity check: {s}")))
    }
}

fn do_restore(staged: &Path, db_path: &Path, default_backup_dir: &Path) -> Result<RestoreOutcome, AppError> {
    if !staged.is_file() {
        return Err(AppError::IoError("The staged backup file is missing.".into()));
    }
    if let Some(reason) = preview_restore(staged)?.refusal {
        return Err(AppError::ValidationError { field: "path".into(), message: reason });
    }
    let safety = if db_path.is_file() {
        let dir = backup_dir_from_live(db_path, default_backup_dir);
        Some(safety_copy(db_path, &dir)?)
    } else {
        None
    };
    replace_file(staged, db_path)?;
    if let Err(e) = integrity_of(db_path) {
        if let Some(s) = &safety {
            let _ = replace_file(s, db_path);
        }
        return Err(e);
    }
    Ok(RestoreOutcome { safety_copy: safety })
}

/// Start-up step, before the pool opens. No marker: `Ok(None)`. On success the marker and the
/// staged file are deleted. On failure the current database is kept, the marker becomes
/// `restore_failed.json` with the reason, and the error is returned for logging.
pub fn apply_pending_restore(
    app_data_dir: &Path,
    db_path: &Path,
    default_backup_dir: &Path,
) -> Result<Option<RestoreOutcome>, AppError> {
    let marker_path = app_data_dir.join(PENDING_MARKER);
    if !marker_path.is_file() {
        return Ok(None);
    }
    let marker: Result<PendingMarker, AppError> = std::fs::read_to_string(&marker_path)
        .map_err(|e| io_err("Cannot read the restore marker", e))
        .and_then(|t| serde_json::from_str(&t).map_err(|e| io_err("The restore marker is damaged", e)));
    let (staged, source) = match &marker {
        Ok(m) => (PathBuf::from(&m.staged), m.source.clone()),
        Err(_) => (app_data_dir.join(STAGED_FILE), String::new()),
    };
    let result = marker.and_then(|_| do_restore(&staged, db_path, default_backup_dir));
    let _ = std::fs::remove_file(&marker_path);
    let _ = std::fs::remove_file(&staged);
    match result {
        Ok(outcome) => Ok(Some(outcome)),
        Err(e) => {
            let failed = FailedMarker { reason: e.to_string(), source, failed_at: now_rfc3339() };
            let json = serde_json::to_string_pretty(&failed).unwrap_or_default();
            let _ = std::fs::write(app_data_dir.join(FAILED_MARKER), json);
            Err(e)
        }
    }
}

/// The reason of the last failed restore, if Settings has not dismissed it yet.
pub fn restore_failure(app_data_dir: &Path) -> Option<String> {
    let text = std::fs::read_to_string(app_data_dir.join(FAILED_MARKER)).ok()?;
    serde_json::from_str::<FailedMarker>(&text).ok().map(|f| f.reason)
}

pub fn dismiss_restore_failure(app_data_dir: &Path) {
    let _ = std::fs::remove_file(app_data_dir.join(FAILED_MARKER));
}
