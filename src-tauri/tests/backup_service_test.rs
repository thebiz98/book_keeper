// tests/backup_service_test.rs
//! Test Suite: BackupService (LLD 6.13): naming, never overwrite, destination validation, integrity.

mod common;

use book_keeper::data::sqlite::connection::create_pool;
use book_keeper::data::sqlite::migrations::run_migrations;
use book_keeper::services::contracts::backup_service::BackupService;
use common::BackupServiceImpl;
use std::path::PathBuf;

fn live_db() -> (PathBuf, PathBuf, BackupServiceImpl) {
    let dir = std::env::temp_dir().join(format!("bk_backup_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let live = dir.join("shop.db");
    let pool = create_pool(&live).unwrap();
    run_migrations(&pool.get().unwrap()).unwrap();
    drop(pool);
    let dest = dir.join("out");
    std::fs::create_dir_all(&dest).unwrap();
    (live.clone(), dest, BackupServiceImpl::new(live))
}

#[test]
fn name_format_collision_suffix_and_integrity() {
    let (_live, dest, svc) = live_db();
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
    let (live, dest, svc) = live_db();
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
