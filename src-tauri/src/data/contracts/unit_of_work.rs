use crate::core::error::AppError;
use super::adjustment_allocation_repository::AdjustmentAllocationRepository;
use super::adjustment_repository::AdjustmentRepository;
use super::allocation_repository::AllocationRepository;
use super::bundle_repository::BundleRepository;
use super::item_repository::ItemRepository;
use super::party_repository::PartyRepository;
use super::purchase_repository::PurchaseRepository;
use super::return_repository::ReturnRepository;
use super::sale_repository::SaleRepository;
use super::setting_repository::SettingRepository;
use super::settlement_repository::SettlementRepository;

pub trait UnitOfWork: Send + Sync {
    /// Begins an immediate write transaction.
    fn begin_transaction(&self) -> Result<Box<dyn TransactionContext>, AppError>;
}

impl<T: UnitOfWork + ?Sized> UnitOfWork for std::sync::Arc<T> {
    fn begin_transaction(&self) -> Result<Box<dyn TransactionContext>, AppError> {
        (**self).begin_transaction()
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
    fn adjustment_allocations(&mut self) -> &mut dyn AdjustmentAllocationRepository;
    fn settings(&mut self) -> &mut dyn SettingRepository;
    fn parties(&mut self) -> &mut dyn PartyRepository;
}
