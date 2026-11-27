// tests/restore_test.rs
//! Test Suite: restore from backup (FR-60, DD-99): list, preview, staging and the startup swap.

mod common;

use book_keeper::data::sqlite::connection::create_pool;
use book_keeper::data::sqlite::migrations::{latest_version, run_migrations};
use book_keeper::services::r#impl::restore::{
    apply_pending_restore, dismiss_restore_failure, list_backups, preview_restore, preview_restore_for, restore_failure, stage_restore,
    FAILED_MARKER, PENDING_MARKER, STAGED_FILE,
};
use rusqlite::Connection;
use std::path::{Path, PathBuf};

/// A migrated database at `path` holding `items` active items.
fn make_db(path: &Path, items: usize) {
    {
        let pool = create_pool(path).unwrap();
        run_migrations(&pool.get().unwrap()).unwrap();
    }
    let c = Connection::open(path).unwrap();
    for i in 0..items {
        c.execute("INSERT INTO items (name) VALUES (?1)", [format!("Item {i}")]).unwrap();
    }
}

fn item_count(db: &Path) -> i64 {
    Connection::open(db).unwrap().query_row("SELECT COUNT(*) FROM items", [], |r| r.get(0)).unwrap()
}

fn files(dir: &Path) -> Vec<String> {
    std::fs::read_dir(dir)
        .map(|rd| rd.filter_map(Result::ok).map(|e| e.file_name().to_string_lossy().to_string()).collect())
        .unwrap_or_default()
}

fn app_dir() -> (common::TempDir, PathBuf, PathBuf) {
    let t = common::TempDir::new("restore");
    let db = t.join("shop.db");
    let backups = t.join("backups");
    (t, db, backups)
}

#[test]
fn preview_of_a_good_file_reports_counts_and_last_bill_date() {
    let t = common::TempDir::new("restore_prev");
    let f = t.join("b.db");
    make_db(&f, 3);
    let c = Connection::open(&f).unwrap();
    c.execute("UPDATE items SET is_active = 0 WHERE name = 'Item 2'", []).unwrap();
    for (code, date, opening) in [("S-1", "2026-11-01", 0), ("S-2", "2026-11-20", 0), ("OB-1", "2026-01-01", 1)] {
        c.execute(
            "INSERT INTO sale_transactions (transaction_code, transaction_date, payment_status, total_amount, is_opening_balance)
             VALUES (?1, ?2, 'PAID', 10, ?3)",
            rusqlite::params![code, date, opening],
        )
        .unwrap();
    }
    c.execute(
        "INSERT INTO purchase_transactions (transaction_code, transaction_date, payment_status, total_amount)
         VALUES ('P-1', '2026-11-25', 'PAID', 5)",
        [],
    )
    .unwrap();
    drop(c);
    let p = preview_restore(&f).unwrap();
    assert!(p.refusal.is_none(), "{p:?}");
    assert!(p.integrity_ok && !p.newer_than_app);
    assert_eq!(p.schema_version, latest_version());
    assert_eq!(p.item_count, 2, "active items only");
    assert_eq!(p.sale_bill_count, 2, "opening bills excluded");
    assert_eq!(p.last_bill_date.as_deref(), Some("2026-11-25"));
}

#[test]
fn preview_refuses_corrupt_foreign_and_newer_files() {
    let t = common::TempDir::new("restore_bad");
    let junk = t.join("junk.db");
    std::fs::write(&junk, vec![0x42u8; 8192]).unwrap();
    assert!(preview_restore(&junk).unwrap().refusal.is_some());
    assert!(preview_restore(&t.join("missing.db")).unwrap().refusal.is_some());

    let foreign = t.join("foreign.db");
    Connection::open(&foreign).unwrap().execute_batch("CREATE TABLE x (a INTEGER);").unwrap();
    let p = preview_restore(&foreign).unwrap();
    assert!(p.refusal.unwrap().contains("not a Book Keeper"));

    let newer = t.join("newer.db");
    make_db(&newer, 1);
    Connection::open(&newer)
        .unwrap()
        .execute("INSERT INTO schema_version (version, name) VALUES (?1, 'future')", [latest_version() + 1])
        .unwrap();
    let p = preview_restore(&newer).unwrap();
    assert!(p.newer_than_app && p.refusal.is_some(), "{p:?}");
}

#[test]
fn list_backups_is_newest_first_with_kinds_and_skips_other_files() {
    let t = common::TempDir::new("restore_list");
    for n in [
        "book_keeper_auto_20260101_0900.db",
        "book_keeper_backup_20260301_1000.db",
        "book_keeper_prerestore_20260201_0800.db",
        "book_keeper_auto_20260401_0900.db.tmp",
        "notes.txt",
    ] {
        std::fs::write(t.join(n), b"x").unwrap();
    }
    let rows = list_backups(t.path()).unwrap();
    let got: Vec<(&str, &str)> = rows.iter().map(|r| (r.kind.as_str(), r.file_name.as_str())).collect();
    assert_eq!(
        got,
        vec![
            ("MANUAL", "book_keeper_backup_20260301_1000.db"),
            ("PRE_RESTORE", "book_keeper_prerestore_20260201_0800.db"),
            ("AUTO", "book_keeper_auto_20260101_0900.db"),
        ]
    );
    assert!(list_backups(&t.join("nope")).unwrap().is_empty());
}

#[test]
fn staged_restore_is_swapped_in_with_a_safety_copy() {
    let (t, db, backups) = app_dir();
    make_db(&db, 5);
    let backup = t.join("old.db");
    make_db(&backup, 2);
    stage_restore(t.path(), &db, &backup).unwrap();
    assert!(t.join(STAGED_FILE).is_file() && t.join(PENDING_MARKER).is_file());

    let out = apply_pending_restore(t.path(), &db, &backups).unwrap().expect("applied");
    assert_eq!(item_count(&db), 2, "data replaced");
    let safety = out.safety_copy.expect("safety copy");
    assert!(safety.file_name().unwrap().to_string_lossy().starts_with("book_keeper_prerestore_"));
    assert_eq!(item_count(&safety), 5, "safety copy holds the data from before");
    assert!(!t.join(STAGED_FILE).exists() && !t.join(PENDING_MARKER).exists());
    assert!(!t.join("shop.db-wal").exists() && !t.join("shop.db.restoring").exists());
    assert!(restore_failure(t.path()).is_none());
    // Nothing pending: a second start does nothing.
    assert!(apply_pending_restore(t.path(), &db, &backups).unwrap().is_none());
}

#[test]
fn safety_copy_follows_the_backup_folder_saved_in_the_live_db() {
    let (t, db, backups) = app_dir();
    make_db(&db, 4);
    let chosen = t.join("usb");
    Connection::open(&db)
        .unwrap()
        .execute("UPDATE shop_settings SET backup_directory = ?1 WHERE id = 1", [chosen.to_str().unwrap()])
        .unwrap();
    let backup = t.join("old.db");
    make_db(&backup, 1);
    stage_restore(t.path(), &db, &backup).unwrap();
    apply_pending_restore(t.path(), &db, &backups).unwrap().unwrap();
    assert_eq!(files(&chosen).len(), 1, "{:?}", files(&chosen));
    assert!(!backups.exists());
}

#[test]
fn failed_swap_keeps_current_data_and_reports_once() {
    let (t, db, backups) = app_dir();
    make_db(&db, 5);
    let backup = t.join("old.db");
    make_db(&backup, 2);
    stage_restore(t.path(), &db, &backup).unwrap();
    // The staged copy gets damaged before the next start.
    std::fs::write(t.join(STAGED_FILE), vec![0x42u8; 8192]).unwrap();

    assert!(apply_pending_restore(t.path(), &db, &backups).is_err());
    assert_eq!(item_count(&db), 5, "current data untouched");
    assert!(!t.join(PENDING_MARKER).exists() && t.join(FAILED_MARKER).is_file());
    assert!(restore_failure(t.path()).is_some());
    dismiss_restore_failure(t.path());
    assert!(restore_failure(t.path()).is_none());
    assert!(!t.join(FAILED_MARKER).exists());
}

#[test]
fn staging_a_refused_file_writes_nothing() {
    let (t, db, _backups) = app_dir();
    let junk = t.join("junk.db");
    std::fs::write(&junk, vec![0x42u8; 8192]).unwrap();
    assert!(stage_restore(t.path(), &db, &junk).is_err());
    assert!(!t.join(STAGED_FILE).exists() && !t.join(PENDING_MARKER).exists());
}

#[test]
fn the_live_database_and_app_owned_files_are_refused() {
    let (t, db, _backups) = app_dir();
    make_db(&db, 2);
    let p = preview_restore_for(&db, &db, t.path()).unwrap();
    assert_eq!(p.refusal.as_deref(), Some("This is the database in use, not a backup."));
    // A differently spelled path to the same file is caught too.
    let dotted = t.path().join(".").join("shop.db");
    assert!(preview_restore_for(&dotted, &db, t.path()).unwrap().refusal.unwrap().contains("in use"));
    // The staged file inside the app data folder.
    let staged = t.join(STAGED_FILE);
    make_db(&staged, 1);
    assert!(preview_restore_for(&staged, &db, t.path()).unwrap().refusal.unwrap().contains("in use"));
    assert!(stage_restore(t.path(), &db, &db).is_err());
    assert!(!t.join(PENDING_MARKER).exists());
    // An ordinary backup elsewhere in the folder is fine.
    let ok = t.join("backups").join("book_keeper_backup_20260101_0900.db");
    std::fs::create_dir_all(ok.parent().unwrap()).unwrap();
    make_db(&ok, 1);
    assert!(preview_restore_for(&ok, &db, t.path()).unwrap().refusal.is_none());
}

#[test]
fn retry_io_retries_then_gives_up() {
    use book_keeper::services::r#impl::restore::retry_io;
    let mut calls = 0;
    let r = retry_io(5, std::time::Duration::from_millis(1), || {
        calls += 1;
        if calls < 3 { Err(std::io::Error::other("busy")) } else { Ok(calls) }
    });
    assert_eq!(r.unwrap(), 3);
    let mut calls = 0;
    let r: std::io::Result<()> = retry_io(4, std::time::Duration::from_millis(1), || {
        calls += 1;
        Err(std::io::Error::other("busy"))
    });
    assert!(r.is_err());
    assert_eq!(calls, 4);
}
