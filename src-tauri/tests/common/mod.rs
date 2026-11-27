// tests/common/mod.rs
//! Shared test harness. Every test runs against the production SQLite stack
//! (`SqliteUnitOfWork` + schema migrations + FTS5 indexer) on its own fresh
//! temporary database file, so suites are isolated and exercise real SQL.

#![allow(dead_code, unused_imports, unused_variables)]

pub mod fixtures;
pub mod invariants;
pub mod ledger;
pub mod stock;
pub mod vedbill_fixture;

use std::collections::HashMap;
use std::fs::File;
use std::sync::{Arc, Mutex};
use rusqlite::Connection;

use book_keeper::core::dtos::*;
use book_keeper::core::entities::*;
use book_keeper::core::error::AppError;
use book_keeper::core::value_objects::*;
use book_keeper::data::contracts::search_indexer::ItemSearchIndexer;
use book_keeper::data::contracts::unit_of_work::{TransactionContext, UnitOfWork};
use book_keeper::data::contracts::*;
use book_keeper::services::contracts::accounting_service::AccountingService;
use book_keeper::services::contracts::inventory_allocation_strategy::{
    DepletionResult, InventoryAllocationStrategy, OutflowDepletionResult, ReversalResult,
};
use book_keeper::services::contracts::inventory_service::InventoryService;
use book_keeper::services::contracts::payment_service::PaymentService;
use book_keeper::services::contracts::purchase_service::PurchaseService;
use book_keeper::services::contracts::report_service::ReportService;
use book_keeper::services::contracts::return_service::ReturnService;
use book_keeper::services::contracts::sale_service::SaleService;
use std::path::PathBuf;

use book_keeper::data::sqlite::connection::{create_pool, SqlitePool};
use book_keeper::data::sqlite::fts_indexer::SqliteFts5SearchIndexer;
use book_keeper::data::sqlite::migrations::run_migrations;
use book_keeper::data::sqlite::sqlite_uow::SqliteUnitOfWork;

/// Isolated SQLite database for one test. Cheap to clone (shares the pool);
/// the file is deleted when the last clone is dropped.
#[derive(Clone)]
pub struct TestDb {
    uow: Arc<SqliteUnitOfWork>,
    pub pool: SqlitePool,
    _file: Arc<TempDbFile>,
}

struct TempDbFile(PathBuf);

static SWEEP_ONCE: std::sync::Once = std::sync::Once::new();

/// Once per test binary: deletes `%TEMP%/bk_*` entries older than 2 hours. Catches files an
/// earlier run could not delete (Windows keeps open files locked) and single temp files.
pub fn sweep_stale_temp() {
    SWEEP_ONCE.call_once(|| {
        let tmp = std::env::temp_dir();
        let Ok(rd) = std::fs::read_dir(&tmp) else { return };
        let limit = std::time::Duration::from_secs(2 * 60 * 60);
        for e in rd.filter_map(Result::ok) {
            if !e.file_name().to_string_lossy().starts_with("bk_") {
                continue;
            }
            let old = e
                .metadata()
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| t.elapsed().ok())
                .is_some_and(|age| age > limit);
            if old {
                let p = e.path();
                let _ = if p.is_dir() { std::fs::remove_dir_all(&p) } else { std::fs::remove_file(&p) };
            }
        }
    });
}

/// Unique `%TEMP%/bk_<tag>_<uuid>` folder, removed recursively on drop (best effort).
pub struct TempDir(PathBuf);

impl TempDir {
    pub fn new(tag: &str) -> Self {
        sweep_stale_temp();
        let dir = std::env::temp_dir().join(format!("bk_{tag}_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        TempDir(dir)
    }
    pub fn path(&self) -> &std::path::Path {
        &self.0
    }
    pub fn join(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Unique single-file path `%TEMP%/bk_<tag>_<uuid>.<ext>`; the sweep removes it in a later run.
pub fn temp_file(tag: &str, ext: &str) -> PathBuf {
    sweep_stale_temp();
    std::env::temp_dir().join(format!("bk_{tag}_{}.{ext}", uuid::Uuid::new_v4()))
}

impl Drop for TempDbFile {
    fn drop(&mut self) {
        for ext in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{}", self.0.display(), ext));
        }
    }
}

impl TestDb {
    pub fn new() -> Self {
        sweep_stale_temp();
        let path = std::env::temp_dir().join(format!("bk_test_{}.db", uuid::Uuid::new_v4()));
        let pool = create_pool(&path).expect("create test pool");
        run_migrations(&pool.get().expect("checkout")).expect("run migrations");
        Self {
            uow: Arc::new(SqliteUnitOfWork::new(pool.clone())),
            pool,
            _file: Arc::new(TempDbFile(path)),
        }
    }

    /// Party FTS index bound to this test database.
    pub fn party_index(&self) -> book_keeper::data::sqlite::fts::SqliteFtsIndex {
        book_keeper::data::sqlite::fts::SqliteFtsIndex::new(
            self.pool.clone(),
            &book_keeper::data::sqlite::fts_specs::PARTY_FTS,
        )
    }

    /// Expense type FTS index bound to this test database.
    pub fn expense_type_index(&self) -> book_keeper::data::sqlite::fts::SqliteFtsIndex {
        book_keeper::data::sqlite::fts::SqliteFtsIndex::new(
            self.pool.clone(),
            &book_keeper::data::sqlite::fts_specs::EXPENSE_TYPE_FTS,
        )
    }

    /// Charge type FTS index bound to this test database.
    pub fn charge_type_index(&self) -> book_keeper::data::sqlite::fts::SqliteFtsIndex {
        book_keeper::data::sqlite::fts::SqliteFtsIndex::new(
            self.pool.clone(),
            &book_keeper::data::sqlite::fts_specs::CHARGE_TYPE_FTS,
        )
    }

    /// Production FTS5 indexer bound to this test database.
    pub fn indexer(&self) -> SqliteFts5SearchIndexer {
        SqliteFts5SearchIndexer::new(self.pool.clone())
    }
}

impl Default for TestDb {
    fn default() -> Self {
        Self::new()
    }
}

impl UnitOfWork for TestDb {
    fn begin_transaction(&self) -> Result<Box<dyn TransactionContext>, AppError> {
        self.uow.begin_transaction()
    }

    fn begin_read(&self) -> Result<Box<dyn TransactionContext>, AppError> {
        self.uow.begin_read()
    }
}

// =========================================================================
// Production domain services bound to the SQLite test database
// =========================================================================

pub use book_keeper::services::{
    FifoStrategy,
    PaymentServiceImpl,
    BackupServiceImpl,
};

pub type InventoryServiceImpl<S> = book_keeper::services::InventoryServiceImpl<S, TestDb>;
pub type SaleServiceImpl<I, P> = book_keeper::services::SaleServiceImpl<I, P, TestDb>;
pub type PurchaseServiceImpl<I, P> = book_keeper::services::PurchaseServiceImpl<I, P, TestDb>;
pub type ReturnServiceImpl<I, P, A> = book_keeper::services::ReturnServiceImpl<I, P, A, TestDb>;
pub type AccountingServiceImpl<P> = book_keeper::services::AccountingServiceImpl<P, TestDb>;
pub type ReportServiceImpl = book_keeper::services::ReportServiceImpl<TestDb>;
pub type ItemImportServiceImpl<I> = book_keeper::services::ItemImportServiceImpl<TestDb, I>;
pub type VedbillImportServiceImpl<I> =
    book_keeper::services::VedbillImportServiceImpl<TestDb, I, book_keeper::data::vedbill::ZipVedbillReader>;
pub type ItemServiceImpl<I> = book_keeper::services::ItemServiceImpl<TestDb, I>;
pub type PartyServiceImpl = book_keeper::services::PartyServiceImpl<TestDb>;
pub type ExpenseServiceImpl = book_keeper::services::ExpenseServiceImpl<TestDb>;
pub type ChargeTypeServiceImpl = book_keeper::services::ChargeTypeServiceImpl<TestDb>;
pub type ShopSettingServiceImpl = book_keeper::services::ShopSettingServiceImpl<TestDb>;
