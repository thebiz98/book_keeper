//! SQLite Unit of Work: one `BEGIN IMMEDIATE` transaction per context.
use std::rc::Rc;

use crate::core::error::AppError;
use crate::data::contracts::*;
use super::connection::SqlitePool;
use super::repositories::*;
use super::SqliteConn;

pub struct SqliteUnitOfWork {
    pool: SqlitePool,
}

impl SqliteUnitOfWork {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

impl UnitOfWork for SqliteUnitOfWork {
    fn begin_transaction(&self) -> Result<Box<dyn TransactionContext>, AppError> {
        let pooled = self
            .pool
            .get()
            .map_err(|e| AppError::DatabaseError(format!("pool checkout failed: {e}")))?;
        pooled.execute_batch("BEGIN IMMEDIATE")?;
        let conn: SqliteConn = Rc::new(pooled);
        Ok(Box::new(SqliteTransactionContext {
            items: SqliteItemRepository::new(conn.clone()),
            bundles: SqliteBundleRepository::new(conn.clone()),
            purchases: SqlitePurchaseRepository::new(conn.clone()),
            sales: SqliteSaleRepository::new(conn.clone()),
            allocations: SqliteAllocationRepository::new(conn.clone()),
            settlements: SqliteSettlementRepository::new(conn.clone()),
            returns: SqliteReturnRepository::new(conn.clone()),
            adjustments: SqliteAdjustmentRepository::new(conn.clone()),
            adjustment_allocations: SqliteAdjustmentAllocationRepository::new(conn.clone()),
            settings: SqliteSettingRepository::new(conn.clone()),
            parties: SqlitePartyRepository::new(conn.clone()),
            conn,
            finished: false,
        }))
    }
}

pub struct SqliteTransactionContext {
    conn: SqliteConn,
    finished: bool,
    items: SqliteItemRepository,
    bundles: SqliteBundleRepository,
    purchases: SqlitePurchaseRepository,
    sales: SqliteSaleRepository,
    allocations: SqliteAllocationRepository,
    settlements: SqliteSettlementRepository,
    returns: SqliteReturnRepository,
    adjustments: SqliteAdjustmentRepository,
    adjustment_allocations: SqliteAdjustmentAllocationRepository,
    settings: SqliteSettingRepository,
    parties: SqlitePartyRepository,
}

impl Drop for SqliteTransactionContext {
    fn drop(&mut self) {
        if !self.finished {
            let _ = self.conn.execute_batch("ROLLBACK");
        }
    }
}

impl TransactionContext for SqliteTransactionContext {
    fn commit(mut self: Box<Self>) -> Result<(), AppError> {
        self.finished = true;
        if let Err(e) = self.conn.execute_batch("COMMIT") {
            let _ = self.conn.execute_batch("ROLLBACK");
            return Err(e.into());
        }
        Ok(())
    }

    fn rollback(mut self: Box<Self>) -> Result<(), AppError> {
        self.finished = true;
        self.conn.execute_batch("ROLLBACK")?;
        Ok(())
    }

    fn items(&mut self) -> &mut dyn ItemRepository { &mut self.items }
    fn bundles(&mut self) -> &mut dyn BundleRepository { &mut self.bundles }
    fn purchases(&mut self) -> &mut dyn PurchaseRepository { &mut self.purchases }
    fn sales(&mut self) -> &mut dyn SaleRepository { &mut self.sales }
    fn allocations(&mut self) -> &mut dyn AllocationRepository { &mut self.allocations }
    fn settlements(&mut self) -> &mut dyn SettlementRepository { &mut self.settlements }
    fn returns(&mut self) -> &mut dyn ReturnRepository { &mut self.returns }
    fn adjustments(&mut self) -> &mut dyn AdjustmentRepository { &mut self.adjustments }
    fn adjustment_allocations(&mut self) -> &mut dyn AdjustmentAllocationRepository { &mut self.adjustment_allocations }
    fn settings(&mut self) -> &mut dyn SettingRepository { &mut self.settings }
    fn parties(&mut self) -> &mut dyn PartyRepository { &mut self.parties }
}
