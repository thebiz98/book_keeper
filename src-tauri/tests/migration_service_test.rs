// tests/migration_service_test.rs
//! Test Suite: VedBill Legacy Migration Engine
//! Test Cases: TC-MIG-001 through TC-MIG-003
//! Derived from book_keeper_LLD.md § 11.10 and Spec Miner Report

mod common;

use std::fs::File;
use std::io::{Read, Write};
use common::{MigrationServiceImpl, TestDb};
use book_keeper::core::error::AppError;
use book_keeper::data::contracts::unit_of_work::UnitOfWork;
use book_keeper::services::contracts::migration_service::MigrationService;
use rusqlite::Connection;

/// Helper to generate a genuine valid .vb zip archive containing real SQLite database and manifest
fn create_test_vb_archive(archive_path: &str, company_name: &str, item_count: usize, party_count: usize) {
    let temp_db_path = std::env::temp_dir().join(format!("test_src_{}.db", uuid::Uuid::new_v4()));
    {
        let conn = Connection::open(&temp_db_path).expect("open temp sqlite");
        conn.execute_batch(r#"
            CREATE TABLE items (id TEXT PRIMARY KEY, name TEXT, salePrice REAL, purchasePrice REAL, mrp REAL, primaryQuantity REAL);
            CREATE TABLE parties (id TEXT PRIMARY KEY, name TEXT, phone TEXT, type TEXT);
            CREATE TABLE documents (id TEXT PRIMARY KEY, documentType TEXT, documentNumber TEXT, documentDate TEXT, total REAL);
            CREATE TABLE document_items (id TEXT PRIMARY KEY, documentId TEXT, itemId TEXT, quantity REAL, rate REAL, amount REAL);
        "#).expect("create vdbill tables");

        for i in 1..=item_count {
            conn.execute(
                "INSERT INTO items (id, name, salePrice, purchasePrice, mrp, primaryQuantity) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                rusqlite::params![format!("itm-{}", i), format!("Notebook {}", i), 50.0, 30.0, 60.0, 10.0],
            ).expect("insert test item");
        }

        for i in 1..=party_count {
            conn.execute(
                "INSERT INTO parties (id, name, phone, type) VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params![format!("pty-{}", i), format!("Supplier {}", i), "9876543210", "supplier"],
            ).expect("insert test party");
        }
    }

    let file = File::create(archive_path).expect("create zip file");
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Stored);

    zip.start_file("company_manifest.json", options).expect("manifest entry");
    let manifest_json = format!(
        r#"{{"companyName":"{}","schemaVersion":8,"createdAt":"2026-11-20T10:00:00Z"}}"#,
        company_name
    );
    zip.write_all(manifest_json.as_bytes()).expect("write manifest");

    zip.start_file("company.db", options).expect("company.db entry");
    let mut db_bytes = Vec::new();
    File::open(&temp_db_path).expect("open db").read_to_end(&mut db_bytes).expect("read db bytes");
    zip.write_all(&db_bytes).expect("write real sqlite db");

    zip.finish().expect("finish zip");
    let _ = std::fs::remove_file(&temp_db_path);
}

#[test]
fn test_tc_mig_001_vedbill_zip_container_discovery_and_validation() {
    let uow = TestDb::new();
    let migration_service = MigrationServiceImpl::new(uow);

    let archive_path = std::env::temp_dir().join(format!("test_company_valid_{}.vb", uuid::Uuid::new_v4()));
    let archive_path_str = archive_path.to_str().unwrap();
    create_test_vb_archive(archive_path_str, "Gupta Stationery Mart", 166, 12);

    // Action: inspect backup file
    let preview = migration_service.inspect_backup_file(archive_path_str).expect("preview succeeds");

    // Assert: Correct company name, schema version, items_count (166), parties_count (12)
    assert_eq!(preview.company_name, "Gupta Stationery Mart");
    assert_eq!(preview.schema_version, 8);
    assert_eq!(preview.table_stats.items_count, 166);
    assert_eq!(preview.table_stats.parties_count, 12);

    let _ = std::fs::remove_file(archive_path);
}

#[test]
fn test_tc_mig_002_atomic_database_migration() {
    let uow = TestDb::new();
    let migration_service = MigrationServiceImpl::new(uow.clone());

    let archive_path = std::env::temp_dir().join(format!("test_migration_exec_{}.vb", uuid::Uuid::new_v4()));
    let archive_path_str = archive_path.to_str().unwrap();
    create_test_vb_archive(archive_path_str, "Gupta Stationery Mart", 166, 12);

    // Action: execute migration
    let result = migration_service.execute_migration(archive_path_str).expect("migration succeeds");

    // Assert: All 166 items and 12 parties imported genuinely from company.db
    assert!(result.success);
    assert_eq!(result.items_imported, 166);
    assert_eq!(result.parties_imported, 12);

    let mut tx = uow.begin_transaction().expect("tx");
    let catalog = tx.items().list_catalog(None, false, 0, 200).expect("catalog");
    assert_eq!(catalog.len(), 166, "All 166 items must be present in database");

    let _ = std::fs::remove_file(archive_path);
}

#[test]
fn test_tc_mig_003_corrupt_non_zip_file_rejection() {
    let uow = TestDb::new();
    let migration_service = MigrationServiceImpl::new(uow.clone());

    let corrupt_path = std::env::temp_dir().join(format!("corrupt_file_{}.vb", uuid::Uuid::new_v4()));
    let corrupt_path_str = corrupt_path.to_str().unwrap();
    {
        let mut file = File::create(corrupt_path_str).expect("create corrupt file");
        file.write_all(b"NOT A VALID PKZIP FILE DATA CONTENT").expect("write junk");
    }

    // Action: execute_migration on corrupted file
    let err = migration_service.execute_migration(corrupt_path_str).unwrap_err();

    // Assert: Fails with MigrationError("Invalid ZIP header")
    match err {
        AppError::MigrationError(msg) => {
            assert!(msg.contains("Invalid ZIP header"), "Expected ZIP header error, got: {}", msg);
        }
        other => panic!("Expected MigrationError, got {:?}", other),
    }

    // Active database remains untouched (0 items)
    let mut tx = uow.begin_transaction().expect("tx");
    let catalog = tx.items().list_catalog(None, false, 0, 10).expect("catalog");
    assert!(catalog.is_empty(), "Active database must remain completely untouched on migration rejection");

    let _ = std::fs::remove_file(corrupt_path);
}
