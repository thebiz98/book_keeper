use std::sync::Arc;

use crate::core::error::AppError;
use crate::data::contracts::search_indexer::{FtsIndex, ItemSearchIndexer};
use crate::data::contracts::unit_of_work::UnitOfWork;
use crate::services::contracts::backup_service::{BackupKind, BackupService};
use crate::services::contracts::data_reset_service::{DataResetService, DeletedRows, ResetScope, ResetSummary};
use crate::services::contracts::inventory_service::InventoryService;

pub struct DataResetServiceImpl {
    testing_mode: bool,
    uow: Arc<dyn UnitOfWork>,
    inventory: Arc<dyn InventoryService>,
    indexer: Arc<dyn ItemSearchIndexer>,
    backup: Arc<dyn BackupService>,
    fts_indexes: Vec<Arc<dyn FtsIndex>>,
}

impl DataResetServiceImpl {
    /// Extra FTS indexes rebuilt after a `ResetScope::All` reset (parties, expense types, ...).
    pub fn with_fts_indexes(mut self, indexes: Vec<Arc<dyn FtsIndex>>) -> Self {
        self.fts_indexes = indexes;
        self
    }

    pub fn new(
        testing_mode: bool,
        uow: Arc<dyn UnitOfWork>,
        inventory: Arc<dyn InventoryService>,
        indexer: Arc<dyn ItemSearchIndexer>,
        backup: Arc<dyn BackupService>,
    ) -> Self {
        Self { testing_mode, uow, inventory, indexer, backup, fts_indexes: Vec::new() }
    }
}

impl DataResetService for DataResetServiceImpl {
    fn reset(&self, scope: ResetScope, backup_dir: &str) -> Result<ResetSummary, AppError> {
        if !self.testing_mode {
            return Err(AppError::OperationNotAllowed(
                "Data reset is only available when testingMode is on in app.config.json".into(),
            ));
        }
        let backup_path = self.backup.backup_to_dir(backup_dir, BackupKind::PreReset)?;

        let mut tx = self.uow.begin_transaction()?;
        let mut counts = tx.maintenance().clear_transactions()?;
        if scope == ResetScope::All {
            counts.extend(tx.maintenance().clear_catalog()?);
        }
        // current_stock / is_low_stock follow the (restored) lots.
        self.inventory.repair_stock_cache(&mut *tx)?;
        tx.commit()?;

        // Index after commit (CHECKPOINT convention); a failure here is repaired at next startup.
        if scope == ResetScope::All {
            if let Err(e) = self.indexer.rebuild_index() {
                eprintln!("[reset] search index rebuild failed (fixed at next start): {e}");
            }
            for idx in &self.fts_indexes {
                if let Err(e) = idx.rebuild() {
                    eprintln!("[reset] {} rebuild failed (fixed at next start): {e}", idx.table());
                }
            }
        }

        Ok(ResetSummary {
            scope,
            deleted: counts.into_iter().map(|(table, rows)| DeletedRows { table, rows }).collect(),
            backup_path,
        })
    }
}
