# Project: Book Keeper Production Readiness

## Architecture
Offline-first retail accounting and inventory management desktop application built with Tauri v2 + Rust + SQLite (WAL mode) + React 19 + TypeScript + Tailwind CSS.
- **Backend Architecture**: MVCS (Model-View-Controller-Service)
  - `src-tauri/src/core/`: Domain entities, Value Objects, DTOs, and error types.
  - `src-tauri/src/services/`: Service contracts and modular concrete domain logic (`src/services/impl/`).
  - `src-tauri/src/data/`: Data access repository contracts and SQLite persistence engine (`src/data/sqlite/`).
  - `src-tauri/src/api/`: Thin Tauri IPC command controllers delegating to `AppState`.
  - `src-tauri/src/app_state.rs`: Dependency injection container holding `Arc<dyn Service + Send + Sync>` trait objects.
- **Frontend Architecture**: Component-first, custom lightweight primitives with Lucide icons, dark/light dual theme, Zustand state stores, typed `ApiClient` layer (Mock + Tauri IPC), and 12 counter billing screens.

## Feature Inventory
| # | Feature | Description | Milestone | Source |
|---|---------|-------------|-----------|--------|
| 1 | `FifoStrategy` | FIFO inventory lot allocation, sale depletion, return restoration | M1 | Survey Backend |
| 2 | `PaymentServiceImpl` | Sale/purchase payment recording, refund issuance, settlement | M1 | Survey Backend |
| 3 | `InventoryServiceImpl` | Stock depletion, inward purchase addition, return restore, adjustments | M1 | Survey Backend |
| 4 | `SaleServiceImpl` | Code generation, sale creation with atomic FIFO depletion, history | M1 | Survey Backend |
| 5 | `PurchaseServiceImpl` | Purchase batch ingestion, lot creation, payment record | M1 | Survey Backend |
| 6 | `ReturnServiceImpl` | Customer & vendor returns with lot restoration and dues adjustment | M1 | Survey Backend |
| 7 | `AccountingServiceImpl` | Khata dues overview, party due details, FIFO debt settlement | M1 | Survey Backend |
| 8 | `ReportServiceImpl` | P&L calculation, inventory valuation, cash flow, dashboard metrics | M1 | Survey Backend |
| 9 | `MigrationServiceImpl` | VedBill `.vb` zip archive scanning, inspection, and atomic migration | M1 | Survey Backend |
| 10 | `ItemServiceImpl` | Master item CRUD, packaging bundles, FTS5 sync | M1 | Survey Backend |
| 11 | `PartyServiceImpl` | Customer/vendor master data, soft delete dues check guard | M1 | Survey Backend |
| 12 | `ShopSettingServiceImpl` | Single-row shop setting profile upsert | M1 | Survey Backend |
| 13 | `BackupServiceImpl` | SQLite online zero-downtime backup and integrity check | M1 | Survey Backend |
| 14 | `ImportExportServiceImpl` | Excel workbook import/export via calamine and rust_xlsxwriter | M1 | Survey Backend |
| 15 | `SqliteConnectionPool` | Connection pooling with WAL mode, foreign keys, synchronous NORMAL | M2 | Survey Backend |
| 16 | Schema Migrations v1 | 13 tables, indices, FTS5 table, default shop setting row | M2 | Survey Backend |
| 17 | `SqliteUnitOfWork` | Atomic transactions with `BEGIN IMMEDIATE` write locking | M2 | Survey Backend |
| 18 | `SqliteFts5SearchIndexer` | Item and bundle barcode search indexing with prefix matching | M2 | Survey Backend |
| 19 | 12 SQLite Repositories | Concrete SQL repositories implementing all repository traits | M2 | Survey Backend |
| 20 | `build_sqlite_app_state` | AppState bootstrapping function connecting pool, UoW, and services | M2 | Survey Backend |
| 21 | Sales Controller | Tauri IPC commands: `get_next_sale_code`, `create_sale`, `get_sales_history`, `get_sale_details` | M3 | Survey API |
| 22 | Purchase Controller | Tauri IPC commands: `get_next_purchase_code`, `create_purchase`, `get_purchases_history`, `get_purchase_details` | M3 | Survey API |
| 23 | Item Controller | Tauri IPC commands: `search_items_fts`, `get_items_catalog`, `create_item`, `update_item`, `archive_item`, `create_bundle`, `get_bundle_suggestions` | M3 | Survey API |
| 24 | Dues Controller | Tauri IPC commands: `get_party_dues_summary`, `settle_bill_payment`, `get_party_ledger` | M3 | Survey API |
| 25 | Return Controller | Tauri IPC commands: `get_next_return_code`, `create_customer_return`, `create_vendor_return`, `get_return_details` | M3 | Survey API |
| 26 | Adjustment Controller | Tauri IPC commands: `create_stock_adjustment`, `list_stock_adjustments` | M3 | Survey API |
| 27 | Report Controller | Tauri IPC commands: `get_dashboard_metrics`, `get_low_stock_alerts`, `get_sales_purchase_trend`, `get_pnl_report`, `get_stock_valuation_report`, `get_cash_flow_report` | M3 | Survey API |
| 28 | Party Controller | Tauri IPC commands: `create_party`, `update_party`, `archive_party`, `get_party`, `list_parties`, `search_parties` | M3 | Survey API |
| 29 | Migration Controller | Tauri IPC commands: `scan_backup_directory`, `inspect_backup_file`, `execute_migration`, `export_items_to_excel`, `import_items_from_excel`, `export_transactions_to_excel` | M3 | Survey API |
| 30 | Settings Controller | Tauri IPC commands: `get_shop_settings`, `update_shop_settings`, `backup_database`, `verify_database_integrity` | M3 | Survey API |
| 31 | `main.rs` IPC Registration | Register all 46 commands with `invoke_handler(tauri::generate_handler![...])` and `.manage(app_state)` | M3 | Survey API |
| 32 | `useCartStore` | Zustand cart store with bundle multipliers, stock protection, cart merging | M4 | Survey Frontend |
| 33 | `useAppStore` | Zustand app store with live cash drawer balancing formula and route state | M4 | Survey Frontend |
| 34 | `useThemeStore` | Zustand theme store with dark class toggling and localStorage persistence | M4 | Survey Frontend |
| 35 | `ApiClient` & Mock/Tauri Client | Typed API client interface with 160+ stationery seed items and Tauri IPC fallback | M4 | Survey Frontend |
| 36 | Retail Molecules | `CurrencyDisplay` (en-IN, font-mono), `StockStatusBadge` (0 danger, <5 warn, >=5 success) | M5 | Survey Frontend |
| 37 | Domain Organisms | `RapidBillingRow` (sequential focus cycle, Alt+B modal), `SaleSummaryModal` (hero total, change calc) | M5 | Survey Frontend |
| 38 | Screen Pages | 12 pages (Billing, Purchases, Inventory, Dues, Dashboard, Reports, Settings, Migration, etc.) | M5 | Survey Frontend |
| 39 | Application Shell | `App.tsx`, `main.tsx`, `index.html` with hotkey bindings (F1-F6) | M5 | Survey Frontend |
| 40 | Dual-Track E2E Test Suite | Automated verification suite across backend and frontend, validating production readiness | M6 | Survey E2E |
| 41 | Phased Commits & Checkpointing | Logical, phased git commits and updated `CHECKPOINT.md` reflecting clean production build | M6 | Survey Lifecycle |

## Milestones
| # | Name | Scope | Dependencies | Status |
|---|------|-------|-------------|--------|
| M1 | Backend Concrete Services | `src-tauri/src/services/impl/` (14 services + mod.rs) | none | DONE (e30be0c) |
| M2 | SQLite Persistence & Repositories | `src-tauri/src/data/sqlite/` (pool, migrations, UoW, FTS5, 11 repos, app_state bootstrap; ReportRepository dropped as unused) | M1 | DONE (09797d5) |
| M3 | Tauri IPC Command Controllers | `src-tauri/src/api/` (10 controllers + mod.rs) & `src-tauri/src/main.rs` | M1, M2 | DONE (938f0a2) |
| M4 | Frontend State & Stores | `src/types/`, `src/store/` (3 Zustand stores), `src/api/` (MockApiClient + TauriClient) | none | DONE (bc0dcb4) |
| M5 | Frontend Components & Screen Pages | `src/components/`, `src/pages/`, `src/App.tsx`, `src/main.tsx`, `index.html` | M4 | DONE (1324537) |
| M6 | E2E Integration & Phased Commits | All backend tests on real SQLite, `vite build`, phased commits, CHECKPOINT.md | M1-M5 | DONE (62aa612 + final) |

**Gate per milestone (run once, by integrator):** `cargo test --no-fail-fast` + `cargo clippy -- -D warnings` (lib) + `tsc --noEmit` + `vitest run` + `vite build`. Subagents run only scoped checks. Full `tauri build` not possible on Termux (no webview libs).

## Interface Contracts
### Services ↔ Repositories (`UnitOfWork` & `TransactionContext`)
- `UnitOfWork::begin_transaction(&self) -> Result<Box<dyn TransactionContext>, AppError>`
- `TransactionContext::commit(self: Box<Self>) -> Result<(), AppError>`
- `TransactionContext::rollback(self: Box<Self>) -> Result<(), AppError>`
- Repository accessors: `item_repo()`, `bundle_repo()`, `party_repo()`, `purchase_repo()`, `sale_repo()`, `allocation_repo()`, `settlement_repo()`, `return_repo()`, `adjustment_repo()`, `adjustment_allocation_repo()`, `setting_repo()`, `report_repo()`.

### Controllers ↔ Services (`AppState`)
- Controllers access services through `State<'_, AppState>`:
  `app_state.sale_service.create_sale(...)`, etc.
- Commands return `Result<T, String>` where domain errors are mapped via `.map_err(|e| e.to_string())`.

### Frontend ↔ Backend (IPC Contracts)
- `ApiClient` defines async methods matching Tauri commands with identical payload shapes.
- Browser mode / dev fallback uses `MockApiClient` with local in-memory mutations.
- Tauri mode uses `TauriClient` calling `@tauri-apps/api/core::invoke`.

## Code Layout
```text
src-tauri/
├── Cargo.toml
├── src/
│   ├── main.rs
│   ├── lib.rs
│   ├── app_state.rs
│   ├── api/
│   │   ├── mod.rs
│   │   ├── sale_controller.rs
│   │   ├── purchase_controller.rs
│   │   ├── item_controller.rs
│   │   ├── dues_controller.rs
│   │   ├── return_controller.rs
│   │   ├── adjustment_controller.rs
│   │   ├── report_controller.rs
│   │   ├── party_controller.rs
│   │   ├── migration_controller.rs
│   │   └── settings_controller.rs
│   ├── services/
│   │   ├── contracts/
│   │   └── impl/
│   │       ├── mod.rs
│   │       ├── fifo_strategy.rs
│   │       ├── payment_service_impl.rs
│   │       ├── inventory_service_impl.rs
│   │       ├── sale_service_impl.rs
│   │       ├── purchase_service_impl.rs
│   │       ├── return_service_impl.rs
│   │       ├── accounting_service_impl.rs
│   │       ├── report_service_impl.rs
│   │       ├── migration_service_impl.rs
│   │       ├── item_service_impl.rs
│   │       ├── party_service_impl.rs
│   │       ├── shop_setting_service_impl.rs
│   │       ├── backup_service_impl.rs
│   │       └── import_export_service_impl.rs
│   ├── data/
│   │   ├── contracts/
│   │   └── sqlite/
│   │       ├── mod.rs
│   │       ├── connection.rs
│   │       ├── migrations.rs
│   │       ├── sqlite_uow.rs
│   │       ├── fts_indexer.rs
│   │       └── repositories/
│   │           ├── mod.rs
│   │           ├── item_repo.rs
│   │           ├── bundle_repo.rs
│   │           ├── party_repo.rs
│   │           ├── purchase_repo.rs
│   │           ├── sale_repo.rs
│   │           ├── allocation_repo.rs
│   │           ├── settlement_repo.rs
│   │           ├── return_repo.rs
│   │           ├── adjustment_repo.rs
│   │           ├── adjustment_allocation_repo.rs
│   │           ├── setting_repo.rs
│   │           └── report_repo.rs
src/
├── index.html
├── src/
│   ├── main.tsx
│   ├── App.tsx
│   ├── index.css
│   ├── types/
│   ├── store/
│   │   ├── useCartStore.ts
│   │   ├── useAppStore.ts
│   │   └── useThemeStore.ts
│   ├── api/
│   │   ├── index.ts
│   │   ├── ipc.ts
│   │   └── mock/
│   │       └── mockClient.ts
│   ├── components/
│   │   ├── common/
│   │   │   ├── CurrencyDisplay.tsx
│   │   │   └── StockStatusBadge.tsx
│   │   ├── domain/
│   │   │   ├── RapidBillingRow.tsx
│   │   │   └── SaleSummaryModal.tsx
│   │   ├── layout/
│   │   └── ui/
│   └── pages/
│       ├── DashboardPage.tsx
│       ├── BillingPage.tsx
│       ├── PurchasesPage.tsx
│       ├── InventoryPage.tsx
│       ├── DuesPage.tsx
│       ├── SalesHistoryPage.tsx
│       ├── PurchaseHistoryPage.tsx
│       ├── ReturnsAdjustmentsPage.tsx
│       ├── ReportsPage.tsx
│       ├── PartiesPage.tsx
│       ├── SettingsPage.tsx
│       └── MigrationPage.tsx
```
