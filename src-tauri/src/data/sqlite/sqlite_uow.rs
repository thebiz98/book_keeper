//! SQLite Unit of Work: one transaction per context. Writers use `BEGIN IMMEDIATE`, readers a
//! `BEGIN DEFERRED` snapshot on a `query_only` connection.
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

impl SqliteUnitOfWork {
    fn begin(&self, read_only: bool) -> Result<Box<dyn TransactionContext>, AppError> {
        let pooled = self
            .pool
            .get()
            .map_err(|e| AppError::DatabaseError(format!("pool checkout failed: {e}")))?;
        if read_only {
            pooled.execute_batch("BEGIN DEFERRED")?;
            // Pooled connections are reused, so this is reset whenever the context ends.
            if let Err(e) = pooled.execute_batch("PRAGMA query_only = ON") {
                let _ = pooled.execute_batch("ROLLBACK");
                return Err(e.into());
            }
        } else {
            pooled.execute_batch("BEGIN IMMEDIATE")?;
        }
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
            outflow_allocations: SqliteOutflowAllocationRepository::new(conn.clone()),
            settings: SqliteSettingRepository::new(conn.clone()),
            parties: SqlitePartyRepository::new(conn.clone()),
            reports: SqliteReportRepository::new(conn.clone()),
            conn,
            finished: false,
            read_only,
        }))
    }
}

impl UnitOfWork for SqliteUnitOfWork {
    fn begin_transaction(&self) -> Result<Box<dyn TransactionContext>, AppError> {
        self.begin(false)
    }

    fn begin_read(&self) -> Result<Box<dyn TransactionContext>, AppError> {
        self.begin(true)
    }
}

pub struct SqliteTransactionContext {
    conn: SqliteConn,
    finished: bool,
    read_only: bool,
    items: SqliteItemRepository,
    bundles: SqliteBundleRepository,
    purchases: SqlitePurchaseRepository,
    sales: SqliteSaleRepository,
    allocations: SqliteAllocationRepository,
    settlements: SqliteSettlementRepository,
    returns: SqliteReturnRepository,
    adjustments: SqliteAdjustmentRepository,
    outflow_allocations: SqliteOutflowAllocationRepository,
    settings: SqliteSettingRepository,
    parties: SqlitePartyRepository,
    reports: SqliteReportRepository,
}

impl SqliteTransactionContext {
    /// Ends a read context's snapshot and lifts `query_only` before the connection returns to the pool.
    fn end_read(&self) {
        let _ = self.conn.execute_batch("ROLLBACK");
        let _ = self.conn.execute_batch("PRAGMA query_only = OFF");
    }
}

impl Drop for SqliteTransactionContext {
    fn drop(&mut self) {
        if self.read_only {
            // Idempotent: also covers a read context that was already ended by `rollback`/`commit`.
            self.end_read();
        } else if !self.finished {
            let _ = self.conn.execute_batch("ROLLBACK");
        }
    }
}

impl TransactionContext for SqliteTransactionContext {
    fn commit(mut self: Box<Self>) -> Result<(), AppError> {
        if self.read_only {
            self.finished = true;
            self.end_read();
            return Err(AppError::DatabaseError(
                "commit() called on a read-only context (begin_read); use begin_transaction for writes".to_string(),
            ));
        }
        self.finished = true;
        if let Err(e) = self.conn.execute_batch("COMMIT") {
            let _ = self.conn.execute_batch("ROLLBACK");
            return Err(e.into());
        }
        Ok(())
    }

    fn rollback(mut self: Box<Self>) -> Result<(), AppError> {
        self.finished = true;
        if self.read_only {
            self.end_read();
            return Ok(());
        }
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
    fn outflow_allocations(&mut self) -> &mut dyn OutflowAllocationRepository { &mut self.outflow_allocations }
    fn settings(&mut self) -> &mut dyn SettingRepository { &mut self.settings }
    fn parties(&mut self) -> &mut dyn PartyRepository { &mut self.parties }
    fn reports(&mut self) -> &mut dyn ReportRepository { &mut self.reports }
}
