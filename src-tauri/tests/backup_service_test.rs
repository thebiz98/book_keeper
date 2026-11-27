// tests/backup_service_test.rs
//! Test Suite: BackupService (LLD 6.13): naming, never overwrite, destination validation, integrity.

mod common;

use book_keeper::data::sqlite::connection::create_pool;
use book_keeper::data::sqlite::migrations::run_migrations;
use book_keeper::services::contracts::backup_service::BackupService;
use common::BackupServiceImpl;
use std::path::PathBuf;

fn live_db() -> (PathBuf, PathBuf, BackupServiceImpl, common::TempDir) {
    let guard = common::TempDir::new("backup");
    let dir = guard.path().to_path_buf();
    let live = dir.join("shop.db");
    let pool = create_pool(&live).unwrap();
    run_migrations(&pool.get().unwrap()).unwrap();
    drop(pool);
    let dest = dir.join("out");
    std::fs::create_dir_all(&dest).unwrap();
    (live.clone(), dest, BackupServiceImpl::new(live), guard)
}

#[test]
fn name_format_collision_suffix_and_integrity() {
    let (_live, dest, svc, _guard) = live_db();
    let d = dest.to_str().unwrap();
    let p1 = svc.backup_database(d).unwrap();
    let p2 = svc.backup_database(d).unwrap();
    let p3 = svc.backup_database(d).unwrap();
    let n = |p: &str| std::path::Path::new(p).file_name().unwrap().to_string_lossy().to_string();
    let n1 = n(&p1);
    assert!(n1.starts_with("book_keeper_backup_") && n1.ends_with(".db"), "{n1}");
    let stamp = n1.trim_start_matches("book_keeper_backup_").trim_end_matches(".db");
    assert_eq!(stamp.len(), 13, "YYYYMMDD_HHMM: {stamp}");
    assert!(stamp.chars().enumerate().all(|(i, c)| if i == 8 { c == '_' } else { c.is_ascii_digit() }));
    // Normally the minute does not roll between calls; accept either way but never overwrite.
    let all = [p1.clone(), p2.clone(), p3.clone()];
    let uniq: std::collections::HashSet<_> = all.iter().collect();
    assert_eq!(uniq.len(), 3);
    assert!(std::path::Path::new(&p1).is_absolute());
    if n(&p2).starts_with(&format!("book_keeper_backup_{stamp}")) {
        assert!(n(&p2).ends_with("_2.db"), "{}", n(&p2));
        assert!(n(&p3).ends_with("_3.db") || !n(&p3).contains(stamp), "{}", n(&p3));
    }
    // copy is a healthy DB with the schema
    let conn = rusqlite::Connection::open(&p1).unwrap();
    let ok: String = conn.query_row("PRAGMA integrity_check", [], |r| r.get(0)).unwrap();
    assert_eq!(ok, "ok");
    let tables: i64 = conn.query_row("SELECT COUNT(*) FROM sqlite_master WHERE name='items'", [], |r| r.get(0)).unwrap();
    assert_eq!(tables, 1);
    assert!(svc.verify_database_integrity().unwrap());
}

#[test]
fn rejects_missing_dir_file_and_live_db() {
    let (live, dest, svc, _guard) = live_db();
    let missing = dest.join("nope");
    assert!(svc.backup_database(missing.to_str().unwrap()).is_err());
    assert!(!missing.exists(), "must not create directories");
    assert!(svc.backup_database(live.to_str().unwrap()).is_err(), "live DB path is not a directory");
    // an existing file is not a directory either
    let f = dest.join("x.db");
    std::fs::write(&f, b"x").unwrap();
    assert!(svc.backup_database(f.to_str().unwrap()).is_err());
    assert_eq!(std::fs::read(&f).unwrap(), b"x");
}

// ---- auto backups, rotation, scheduler (feat/backup-reset) ----

use book_keeper::services::contracts::backup_service::BackupKind;
use book_keeper::services::r#impl::backup_scheduler::{db_changed_since_last_auto, is_due, run_auto_backup};
use book_keeper::services::r#impl::backup_service_impl::{parse_stamp, select_for_deletion};

fn ts(s: &str) -> chrono::NaiveDateTime {
    chrono::NaiveDateTime::parse_from_str(s, "%Y%m%d_%H%M").unwrap()
}

#[test]
fn backup_to_dir_creates_missing_folder_and_uses_kind_prefix() {
    let (_live, dest, svc, _guard) = live_db();
    let nested = dest.join("a").join("b");
    let p = svc.backup_to_dir(nested.to_str().unwrap(), BackupKind::Auto).unwrap();
    assert!(nested.is_dir());
    let name = std::path::Path::new(&p).file_name().unwrap().to_string_lossy().to_string();
    assert!(name.starts_with("book_keeper_auto_"), "{name}");
    assert!(parse_stamp(&name, BackupKind::Auto).is_some());
    let p2 = svc.backup_to_dir(nested.to_str().unwrap(), BackupKind::PreReset).unwrap();
    assert!(p2.contains("book_keeper_prereset_"));
    assert!(svc.last_auto_backup(nested.to_str().unwrap()).unwrap().is_some());
}

#[test]
fn parse_stamp_accepts_suffix_and_rejects_other_files() {
    assert_eq!(parse_stamp("book_keeper_auto_20260101_0930.db", BackupKind::Auto), Some(ts("20260101_0930")));
    assert_eq!(parse_stamp("book_keeper_auto_20260101_0930_3.db", BackupKind::Auto), Some(ts("20260101_0930")));
    assert_eq!(parse_stamp("book_keeper_auto_20260101_0930.db", BackupKind::Manual), None);
    assert_eq!(parse_stamp("book_keeper_auto_20260101_0930_x.db", BackupKind::Auto), None);
    assert_eq!(parse_stamp("book_keeper_auto_garbage.db", BackupKind::Auto), None);
    assert_eq!(parse_stamp("notes.txt", BackupKind::Auto), None);
}

#[test]
fn rotation_rule_keeps_manual_unknown_and_newest_auto() {
    let now = ts("20260120_1200");
    let names: Vec<String> = [
        "book_keeper_auto_20260101_1000.db",     // old -> delete
        "book_keeper_auto_20260105_1000_2.db",   // old -> delete
        "book_keeper_auto_20260119_1000.db",     // recent -> keep
        "book_keeper_prereset_20260102_1000.db", // old -> delete
        "book_keeper_prereset_20260118_1000.db", // recent -> keep
        "book_keeper_backup_20250101_1000.db",   // manual -> never
        "random.db",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    assert_eq!(
        select_for_deletion(&names, now, 7),
        vec![
            "book_keeper_auto_20260101_1000.db".to_string(),
            "book_keeper_auto_20260105_1000_2.db".to_string(),
            "book_keeper_prereset_20260102_1000.db".to_string(),
        ]
    );
    // Only old auto backups: the newest one survives anyway.
    let only_old: Vec<String> = ["book_keeper_auto_20260101_1000.db", "book_keeper_auto_20260102_1000.db"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    assert_eq!(select_for_deletion(&only_old, now, 7), vec!["book_keeper_auto_20260101_1000.db".to_string()]);
}

#[test]
fn rotate_deletes_old_auto_files_on_disk() {
    let (_live, dest, svc, _guard) = live_db();
    let d = dest.to_str().unwrap();
    for n in ["book_keeper_auto_20200101_1000.db", "book_keeper_prereset_20200101_1000.db", "book_keeper_backup_20200101_1000.db"] {
        std::fs::write(dest.join(n), b"x").unwrap();
    }
    let fresh = run_auto_backup(&svc, &dest, 7).unwrap();
    assert!(std::path::Path::new(&fresh).exists());
    assert!(!dest.join("book_keeper_auto_20200101_1000.db").exists());
    assert!(!dest.join("book_keeper_prereset_20200101_1000.db").exists());
    assert!(dest.join("book_keeper_backup_20200101_1000.db").exists(), "manual backups are never rotated");
    assert!(svc.rotate(d, 7).unwrap().is_empty());
    // A missing folder rotates to nothing rather than erroring.
    assert!(svc.rotate(dest.join("missing").to_str().unwrap(), 7).unwrap().is_empty());
}

#[test]
fn is_due_rules() {
    let now = ts("20260120_1200");
    assert!(is_due(None, now, 24));
    assert!(!is_due(Some(ts("20260120_0100")), now, 24));
    assert!(is_due(Some(ts("20260119_1200")), now, 24));
}

#[test]
fn change_detection_uses_file_times() {
    let (live, dest, svc, _guard) = live_db();
    assert!(db_changed_since_last_auto(&live, &dest), "no auto backup yet");
    std::thread::sleep(std::time::Duration::from_millis(50));
    svc.backup_to_dir(dest.to_str().unwrap(), BackupKind::Auto).unwrap();
    assert!(!db_changed_since_last_auto(&live, &dest), "nothing written since the backup");
    std::thread::sleep(std::time::Duration::from_millis(50));
    let conn = rusqlite::Connection::open(&live).unwrap();
    conn.execute("UPDATE shop_settings SET shop_name = shop_name || 'x'", []).unwrap();
    drop(conn);
    assert!(db_changed_since_last_auto(&live, &dest), "a write after the backup counts as a change");
}

#[test]
fn premigration_files_are_parsed_and_rotated_like_prereset() {
    assert_eq!(
        parse_stamp("book_keeper_premigration_v2_to_v3_20260101_0930.db", BackupKind::PreMigration),
        Some(ts("20260101_0930"))
    );
    assert_eq!(
        parse_stamp("book_keeper_premigration_v2_to_v3_20260101_0930_2.db", BackupKind::PreMigration),
        Some(ts("20260101_0930"))
    );
    assert_eq!(parse_stamp("book_keeper_premigration_vx_to_v3_20260101_0930.db", BackupKind::PreMigration), None);
    let now = ts("20260120_1200");
    let names: Vec<String> = [
        "book_keeper_premigration_v2_to_v3_20260102_1000.db", // old -> delete
        "book_keeper_premigration_v3_to_v4_20260119_1000.db", // recent -> keep
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    assert_eq!(
        select_for_deletion(&names, now, 7),
        vec!["book_keeper_premigration_v2_to_v3_20260102_1000.db".to_string()]
    );
}

// ---- tmp-then-rename copy ----

fn files_in(dir: &std::path::Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .filter_map(Result::ok)
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();
    v.sort();
    v
}

#[test]
fn backup_leaves_no_tmp_file_and_result_is_healthy() {
    let (_live, dest, svc, _guard) = live_db();
    let p = svc.backup_database(dest.to_str().unwrap()).unwrap();
    let names = files_in(&dest);
    assert_eq!(names.len(), 1, "{names:?}");
    assert!(!names.iter().any(|n| n.contains(".tmp")), "{names:?}");
    assert!(std::fs::metadata(&p).unwrap().len() > 0);
    let conn = rusqlite::Connection::open(&p).unwrap();
    let ok: String = conn.query_row("PRAGMA integrity_check", [], |r| r.get(0)).unwrap();
    assert_eq!(ok, "ok");
}

#[test]
fn failed_backup_leaves_neither_tmp_nor_final_file() {
    let (live, dest, svc, _guard) = live_db();
    // Not a database: the source open/integrity check fails after the name was reserved.
    std::fs::write(&live, vec![0x42u8; 4096]).unwrap();
    assert!(svc.backup_database(dest.to_str().unwrap()).is_err());
    assert!(files_in(&dest).is_empty(), "{:?}", files_in(&dest));
}

#[test]
fn rotate_removes_only_stale_tmp_files() {
    let (_live, dest, svc, _guard) = live_db();
    let old = dest.join("book_keeper_auto_20260101_0900.db.tmp");
    let fresh = dest.join("book_keeper_auto_20260102_0900.db.tmp");
    std::fs::write(&old, b"x").unwrap();
    std::fs::write(&fresh, b"x").unwrap();
    let two_hours_ago = std::time::SystemTime::now() - std::time::Duration::from_secs(2 * 3600);
    std::fs::File::options().write(true).open(&old).unwrap().set_modified(two_hours_ago).unwrap();
    svc.rotate(dest.to_str().unwrap(), 30).unwrap();
    assert!(!old.exists());
    assert!(fresh.exists());
}
