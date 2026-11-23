use crate::core::error::AppError;
use super::adjustment_repository::AdjustmentRepository;
use super::allocation_repository::AllocationRepository;
use super::bundle_repository::BundleRepository;
use super::expense_repository::ExpenseRepository;
use super::item_repository::ItemRepository;
use super::maintenance_repository::MaintenanceRepository;
use super::outflow_allocation_repository::OutflowAllocationRepository;
use super::party_repository::PartyRepository;
use super::purchase_repository::PurchaseRepository;
use super::report_repository::ReportRepository;
use super::return_repository::ReturnRepository;
use super::sale_repository::SaleRepository;
use super::setting_repository::SettingRepository;
use super::settlement_repository::SettlementRepository;

pub trait UnitOfWork: Send + Sync {
    /// Begins an immediate write transaction (`BEGIN IMMEDIATE`). Every state-mutating service
    /// method calls this exactly once, at the top, and passes the context down (never nested).
    fn begin_transaction(&self) -> Result<Box<dyn TransactionContext>, AppError>;

    /// Begins a read-only snapshot (`BEGIN DEFERRED`): it takes no write lock, so it never
    /// blocks (nor is blocked by) a concurrent writer in WAL mode. Every read-only service
    /// method uses this.
    ///
    /// Calling a write method (`&mut self` repository method, `commit`) on a read context is a
    /// programmer error. The SQLite implementation enforces it: the connection is put in
    /// `query_only` mode, so any write fails with a database error, and `commit` returns an
    /// error (use `rollback`, or just drop the context, to end a read).
    fn begin_read(&self) -> Result<Box<dyn TransactionContext>, AppError>;
}

impl<T: UnitOfWork + ?Sized> UnitOfWork for std::sync::Arc<T> {
    fn begin_transaction(&self) -> Result<Box<dyn TransactionContext>, AppError> {
        (**self).begin_transaction()
    }

    fn begin_read(&self) -> Result<Box<dyn TransactionContext>, AppError> {
        (**self).begin_read()
    }
}

pub trait TransactionContext {
    /// Commits all staged mutations atomically.
    fn commit(self: Box<Self>) -> Result<(), AppError>;

    /// Explicitly aborts the transaction (automatic on drop if not committed).
    fn rollback(self: Box<Self>) -> Result<(), AppError>;

    // Repositories scoped to the live transaction handle:
    fn items(&mut self) -> &mut dyn ItemRepository;
    fn bundles(&mut self) -> &mut dyn BundleRepository;
    fn purchases(&mut self) -> &mut dyn PurchaseRepository;
    fn sales(&mut self) -> &mut dyn SaleRepository;
    fn allocations(&mut self) -> &mut dyn AllocationRepository;
    fn settlements(&mut self) -> &mut dyn SettlementRepository;
    fn returns(&mut self) -> &mut dyn ReturnRepository;
    fn adjustments(&mut self) -> &mut dyn AdjustmentRepository;
    fn outflow_allocations(&mut self) -> &mut dyn OutflowAllocationRepository;
    fn settings(&mut self) -> &mut dyn SettingRepository;
    fn parties(&mut self) -> &mut dyn PartyRepository;
    fn expenses(&mut self) -> &mut dyn ExpenseRepository;
    fn reports(&mut self) -> &mut dyn ReportRepository;
    /// Bulk deletes for the testing-only data reset.
    fn maintenance(&mut self) -> &mut dyn MaintenanceRepository;
}
