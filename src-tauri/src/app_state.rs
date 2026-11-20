use std::sync::Arc;
use crate::data::contracts::search_indexer::ItemSearchIndexer;
use crate::data::contracts::unit_of_work::UnitOfWork;
use crate::services::contracts::accounting_service::AccountingService;
use crate::services::contracts::backup_service::BackupService;
use crate::services::contracts::import_export_service::DataExportImportService;
use crate::services::contracts::inventory_allocation_strategy::InventoryAllocationStrategy;
use crate::services::contracts::inventory_service::InventoryService;
use crate::services::contracts::item_service::ItemService;
use crate::services::contracts::migration_service::MigrationService;
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
    pub migration_service: Arc<dyn MigrationService>,
    pub import_export_service: Arc<dyn DataExportImportService>,
    pub backup_service: Arc<dyn BackupService>,
    pub shop_setting_service: Arc<dyn ShopSettingService>,
}
