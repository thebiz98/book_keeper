//! Concrete SQLite repositories. See `data::sqlite` module docs for shared conventions.
//! Agent A owns: item, bundle, party, setting. Agent B owns the rest.

pub mod adjustment_repo;
pub mod allocation_repo;
pub mod bill_charge_repo;
pub mod bundle_repo;
pub mod charge_type_repo;
pub mod expense_repo;
pub mod item_repo;
pub mod maintenance_repo;
pub mod outflow_allocation_repo;
pub mod party_repo;
pub mod purchase_repo;
pub mod item_analytics_sql;
pub mod report_repo;
pub mod return_repo;
pub mod sale_repo;
pub mod service_component_repo;
pub mod setting_repo;
pub mod settlement_repo;

pub use adjustment_repo::SqliteAdjustmentRepository;
pub use allocation_repo::SqliteAllocationRepository;
pub use bill_charge_repo::SqliteBillChargeRepository;
pub use bundle_repo::SqliteBundleRepository;
pub use charge_type_repo::SqliteChargeTypeRepository;
pub use expense_repo::SqliteExpenseRepository;
pub use item_repo::SqliteItemRepository;
pub use maintenance_repo::SqliteMaintenanceRepository;
pub use outflow_allocation_repo::SqliteOutflowAllocationRepository;
pub use party_repo::SqlitePartyRepository;
pub use purchase_repo::SqlitePurchaseRepository;
pub use report_repo::SqliteReportRepository;
pub use return_repo::SqliteReturnRepository;
pub use sale_repo::SqliteSaleRepository;
pub use service_component_repo::SqliteServiceComponentRepository;
pub use setting_repo::SqliteSettingRepository;
pub use settlement_repo::SqliteSettlementRepository;
