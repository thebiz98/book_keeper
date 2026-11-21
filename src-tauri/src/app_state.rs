use std::sync::Arc;
use crate::data::contracts::search_indexer::ItemSearchIndexer;
use crate::data::contracts::unit_of_work::UnitOfWork;
use crate::services::contracts::accounting_service::AccountingService;
use crate::services::contracts::backup_service::BackupService;
use crate::services::contracts::item_import_service::ItemImportService;
use crate::services::contracts::inventory_allocation_strategy::InventoryAllocationStrategy;
use crate::services::contracts::inventory_service::InventoryService;
use crate::services::contracts::item_service::ItemService;
use crate::services::contracts::party_service::PartyService;
use crate::services::contracts::payment_service::PaymentService;
use crate::services::contracts::purchase_service::PurchaseService;
use crate::services::contracts::report_service::ReportService;
use crate::services::contracts::return_service::ReturnService;
use crate::services::contracts::sale_service::SaleService;
use crate::services::contracts::shop_setting_service::ShopSettingService;

pub struct AppState {
    pub unit_of_work: Arc<dyn UnitOfWork>,
    pub search_indexer: Arc<dyn ItemSearchIndexer>,
    pub inventory_allocation_strategy: Arc<dyn InventoryAllocationStrategy>,
    pub inventory_service: Arc<dyn InventoryService>,
    pub item_service: Arc<dyn ItemService>,
    pub sale_service: Arc<dyn SaleService>,
    pub purchase_service: Arc<dyn PurchaseService>,
    pub payment_service: Arc<dyn PaymentService>,
    pub accounting_service: Arc<dyn AccountingService>,
    pub return_service: Arc<dyn ReturnService>,
    pub report_service: Arc<dyn ReportService>,
    pub party_service: Arc<dyn PartyService>,
    pub item_import_service: Arc<dyn ItemImportService>,
    pub backup_service: Arc<dyn BackupService>,
    pub shop_setting_service: Arc<dyn ShopSettingService>,
}

/// Env var holding a full database file path that overrides the default location (DD-20).
pub const DB_PATH_ENV: &str = "BOOK_KEEPER_DB_PATH";

/// Default database file name inside the app data directory (DD-20).
pub const DEFAULT_DB_FILE: &str = "shop.db";

/// Resolves the database file path: a non-blank `override_path` (the value of
/// `BOOK_KEEPER_DB_PATH`, a full file path) wins, otherwise `<app_data_dir>/shop.db`.
/// Pure so it can be tested without Tauri; the caller creates the parent directory.
pub fn resolve_db_path(app_data_dir: &std::path::Path, override_path: Option<&str>) -> std::path::PathBuf {
    match override_path.map(str::trim) {
        Some(p) if !p.is_empty() => std::path::PathBuf::from(p),
        _ => app_data_dir.join(DEFAULT_DB_FILE),
    }
}

/// Maps the configured allocation method to its strategy (DD-08). The only place a concrete
/// strategy is named; services receive it as `Arc<dyn InventoryAllocationStrategy>`.
pub fn strategy_for(method: crate::core::value_objects::AllocationMethod) -> Arc<dyn InventoryAllocationStrategy> {
    use crate::core::value_objects::AllocationMethod;
    match method {
        AllocationMethod::Fifo => Arc::new(crate::services::r#impl::FifoStrategy),
    }
}

pub fn build_sqlite_app_state(db_path: &std::path::Path) -> Result<AppState, crate::core::error::AppError> {
    build_sqlite_app_state_with_strategy(db_path, strategy_for(crate::core::value_objects::AllocationMethod::default()))
}

/// Same wiring with an explicit allocation strategy (tests inject alternative strategies here).
pub fn build_sqlite_app_state_with_strategy(
    db_path: &std::path::Path,
    allocation_strategy: Arc<dyn InventoryAllocationStrategy>,
) -> Result<AppState, crate::core::error::AppError> {
    use crate::core::error::AppError;
    use crate::data::sqlite::connection::create_pool;
    use crate::data::sqlite::fts_indexer::SqliteFts5SearchIndexer;
    use crate::data::sqlite::migrations::run_migrations;
    use crate::data::sqlite::sqlite_uow::SqliteUnitOfWork;
    use crate::services::r#impl::*;

    let pool = create_pool(db_path)?;
    {
        let conn = pool
            .get()
            .map_err(|e| AppError::DatabaseError(format!("pool checkout failed: {e}")))?;
        run_migrations(&conn)?;
    }

    let uow: Arc<dyn UnitOfWork> = Arc::new(SqliteUnitOfWork::new(pool.clone()));
    let search_indexer: Arc<dyn ItemSearchIndexer> = Arc::new(SqliteFts5SearchIndexer::new(pool));

    // Inventory first: ItemService and ItemImportService delegate opening stock to it, and it
    // reads items through the unit of work, so the graph is acyclic.
    let inventory_service: Arc<dyn InventoryService> =
        Arc::new(InventoryServiceImpl::new(allocation_strategy.clone(), uow.clone()));
    let item_service: Arc<dyn ItemService> = Arc::new(ItemServiceImpl::new(
        uow.clone(), search_indexer.clone(), inventory_service.clone(),
    ));
    let payment_service: Arc<dyn PaymentService> = Arc::new(PaymentServiceImpl::new());
    let accounting_service: Arc<dyn AccountingService> =
        Arc::new(AccountingServiceImpl::new(payment_service.clone(), uow.clone()));
    let sale_service: Arc<dyn SaleService> = Arc::new(SaleServiceImpl::new(
        inventory_service.clone(), payment_service.clone(), uow.clone(),
    ));
    let purchase_service: Arc<dyn PurchaseService> = Arc::new(PurchaseServiceImpl::new(
        inventory_service.clone(), payment_service.clone(), uow.clone(),
    ));
    let return_service: Arc<dyn ReturnService> = Arc::new(ReturnServiceImpl::new(
        inventory_service.clone(), payment_service.clone(), uow.clone(),
    ));
    let report_service: Arc<dyn ReportService> = Arc::new(ReportServiceImpl::new(uow.clone()));
    let party_service: Arc<dyn PartyService> = Arc::new(PartyServiceImpl::new(uow.clone()));
    let item_import_service: Arc<dyn ItemImportService> =
        Arc::new(ItemImportServiceImpl::new(uow.clone(), inventory_service.clone()));
    let backup_service: Arc<dyn BackupService> = Arc::new(BackupServiceImpl::new(db_path));
    let shop_setting_service: Arc<dyn ShopSettingService> =
        Arc::new(ShopSettingServiceImpl::new(uow.clone()));

    // Startup invariant (NFR-02 cache audit): current_stock == Σ lot remainders. A mismatch is
    // repaired from the lots (the source of truth) and logged, never silently ignored.
    {
        let mut tx = uow.begin_transaction()?;
        let repaired = inventory_service.repair_stock_cache(&mut *tx)?;
        tx.commit()?;
        for (item_id, cached, lots_sum) in repaired {
            eprintln!(
                "[inventory] stock cache repaired: item #{item_id} cached {cached} -> {lots_sum} (sum of lots)"
            );
        }
    }

    Ok(AppState {
        unit_of_work: uow,
        search_indexer,
        inventory_allocation_strategy: allocation_strategy,
        inventory_service,
        item_service,
        sale_service,
        purchase_service,
        payment_service,
        accounting_service,
        return_service,
        report_service,
        party_service,
        item_import_service,
        backup_service,
        shop_setting_service,
    })
}
