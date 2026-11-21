# DEFERRED — v1 scope removal inventory (FR-17 archiving + deferred parts of FR-11)

Read-only inventory; no code was changed. Basis: DD-16 (v1 FR-11 = items import with dry-run + backup/integrity only), DD-17 (no archive/delete of items or parties; `is_active` stays default 1 with read filters), HLD FR-11/FR-17, LLD §6.14, `docs/reviews/FR-11.md`.

## 1. Summary

- Deferred code is concentrated in four places: `migration_controller.rs` + `MigrationServiceImpl` (VedBill), `ImportExportServiceImpl` (exports **and** the only items-import code), the `archive*` chain (controller → service → repo → trait → ipc → mock → PartiesPage), and `MigrationPage.tsx` (VedBill wizard + Excel import/export card; the only import UI).
- Nothing exists for: parties import, opening dues, restore-from-backup (no restore code anywhere; `restore_*` hits are FIFO "return restore" and unrelated). No CSV reading code exists (`csv` crate is declared but never used) and no export/restore UI other than MigrationPage and the ReportsPage stub button.
- The only in-scope code living with deferred code is `import_items_from_excel` (`import_export_service_impl.rs:102-297`) and its IPC wrapper/`ImportSummary`. It is not DD-16 compliant (see §5), so it should be **rewritten as `ItemImportService`** (LLD §6.14) rather than moved verbatim; at most its header-matching/cell-parsing helpers are worth reusing.
- `is_active` columns, `WHERE is_active = 1` filters, FTS exclusion (`fts_indexer.rs:31,98,114,127`), the `search_indexer_test` TC-SRCH-003 and `BackupService` (backup + integrity, SettingsPage) are KEEP.
- Test impact: 5 Rust tests are deleted (3 in `migration_service_test.rs`, `test_scenario_s5...`, `test_tc_t3_cmb_004...`) so the 59-test gate becomes 54; 1 Vitest test (`TC-MOCK-010`) is deleted/rewritten, and `TC-APP-006` is adapted. Existing Rust tests never exercised items import or any export or archive path.

## 2. Inventory

| Path:line(s) | Symbol / Element | Feature | Action | Notes / knock-on |
|---|---|---|---|---|
| src-tauri/src/api/migration_controller.rs:1-42 | whole file: `scan_backup_directory`, `inspect_backup_file`, `execute_migration`, `export_items_to_excel`, `export_transactions_to_excel`, `import_items_from_excel` | VedBill / exports / items import | REMOVE file; MOVE `import_items_from_excel` (:23-26) | New `import_controller.rs` with dry-run + execute commands (HLD names). Delete `pub mod migration_controller` in `api/mod.rs:4`; `use crate::core::dtos::*` goes with it. |
| src-tauri/src/main.rs:17 | `item_controller::archive_item` | FR-17 | REMOVE | handler list |
| src-tauri/src/main.rs:18 | `party_controller::archive_party` | FR-17 | REMOVE | |
| src-tauri/src/main.rs:28-30 | `migration_controller::execute_migration`, `export_items_to_excel`, `export_transactions_to_excel` | VedBill/export | REMOVE | |
| src-tauri/src/main.rs:51,52,55 | `import_items_from_excel` / `inspect_backup_file` / `scan_backup_directory` | items import / VedBill | MOVE (import) / REMOVE (other two) | Register new import commands instead. |
| src-tauri/src/main.rs:19,62 | `backup_database`, `verify_database_integrity` | FR-11 backup | KEEP | |
| src-tauri/src/api/item_controller.rs:29-31 | `archive_item` | FR-17 | REMOVE | |
| src-tauri/src/api/party_controller.rs:15-17 | `archive_party` | FR-17 | REMOVE | |
| src-tauri/src/services/contracts/item_service.rs:17,41-43 | `archive_item` (trait + Arc forward) | FR-17 | REMOVE | |
| src-tauri/src/services/contracts/party_service.rs:8,31-33 | `archive_party` (trait + Arc forward) | FR-17 | REMOVE | |
| src-tauri/src/services/impl/item_service_impl.rs:200-215 | `archive_item` (+ `search_indexer.remove_item`) | FR-17 | REMOVE | `remove_item` stays in indexer trait (used on update to inactive? verify) — leave trait alone. |
| src-tauri/src/services/impl/party_service_impl.rs:73-114 | `archive_party` incl. 3 outstanding-dues guards | FR-17 | REMOVE | `get_party_due_summary`, `list_unpaid_by_party_id`, `list_purchases` are used elsewhere; do not remove repo methods. |
| src-tauri/src/data/contracts/item_repository.rs:23 / party_repository.rs:19 | `fn archive` | FR-17 | REMOVE | |
| src-tauri/src/data/sqlite/repositories/item_repo.rs:124-127 / party_repo.rs:76-82 | `archive` impls | FR-17 | REMOVE | Check `unit_of_work`/test fakes implementing the traits (none found in tests). |
| src-tauri/src/data/sqlite/repositories/item_repo.rs:80,106 / party_repo.rs:46 ; fts_indexer.rs:31,98,114,127 ; migrations.rs:24,50 | `is_active` filters/columns/update | DD-17 | KEEP | Item create paths hard-set `is_active: true` (item_service_impl.rs:80, party_service_impl.rs:35). |
| src-tauri/tests/search_indexer_test.rs:70-87 | TC-SRCH-003 inactive item excluded | DD-17 read filter | KEEP | Builds an inactive item directly; no archive call. Rename comment only (optional). |
| src-tauri/src/services/contracts/migration_service.rs (all) | `MigrationService` trait | VedBill | REMOVE | `contracts/mod.rs:7,24` |
| src-tauri/src/services/impl/migration_service_impl.rs (all, 1-300) | `MigrationServiceImpl` (scan/inspect/execute, `.vb` zip, party import, `OPEN` purchase header) | VedBill, parties import | REMOVE | `impl/mod.rs:7,22`. Contains the only indexer-after-commit pattern for bulk items (:295-299): replicate in `ItemImportService`. |
| src-tauri/src/services/contracts/import_export_service.rs (all) | `DataExportImportService` trait (3 methods) | exports + import | REMOVE; MOVE import method into new `ItemImportService` trait | `contracts/mod.rs:3,18` |
| src-tauri/src/services/impl/import_export_service_impl.rs:24-100 | `export_items_to_excel` | export | REMOVE | uses `rust_xlsxwriter`; includes "ARCHIVED" status string (:87-90) |
| src-tauri/src/services/impl/import_export_service_impl.rs:299-end | `export_transactions_to_excel` | export | REMOVE | `rust_xlsxwriter` |
| src-tauri/src/services/impl/import_export_service_impl.rs:102-297 | `import_items_from_excel` (calamine, header match, create/update, synthetic `IMP-<id>` purchase per item) | FR-11 items import | MOVE (rewrite) -> `ItemImportService(Impl)` | Not DD-16 compliant; see §5. Then delete file; `impl/mod.rs:4,19`, `tests/common/mod.rs:97-98` re-exports (`ImportExportServiceImpl`, `DataExportImportServiceImpl`). |
| src-tauri/src/core/dtos/migration_dto.rs:1-60; dtos/mod.rs:4,18 | `TableStats`, `MigrationPreview`, `MigrationResult`, `BackupFileInfo` | VedBill | REMOVE | `ImportSummary` (:50+) is in-scope: MOVE/replace with dry-run report + result DTOs (`import_dto.rs`). |
| src-tauri/src/core/error.rs:29-30 | `AppError::MigrationError` | VedBill + schema migrations | KEEP | Also used by `data/sqlite/migrations.rs:212` (schema migrations). |
| src-tauri/src/app_state.rs:6,10 | `use ... DataExportImportService`, `MigrationService` | DI | ADAPT | |
| src-tauri/src/app_state.rs:32-33 | fields `migration_service`, `import_export_service` | DI | ADAPT | Replace with `item_import_service: Arc<dyn ItemImportService>` (LLD :1606). |
| src-tauri/src/app_state.rs:76-79,97-98 | construction of both services | DI | ADAPT | New service needs `uow`, `search_indexer`, item service/inventory deps (LLD :1650). Both old services construct with FTS indexer only for Migration; ImportExport got none. |
| src-tauri/src/app_state.rs:51 / migrations.rs | `run_migrations` | schema | KEEP | Name collision only. |
| src-tauri/Cargo.toml:30 | `zip = "0.6"` | VedBill | REMOVE | Only used in `migration_service_impl.rs:89,165` and 3 test helpers (tests listed in §4). Not needed by items import. |
| src-tauri/Cargo.toml:31 | `csv = "1.3"` | CSV import | KEEP | Currently unused anywhere in `src`; needed by v1 CSV items import (DD-16 requires CSV). |
| src-tauri/Cargo.toml:32 | `calamine = "0.24"` | XLSX import | KEEP | Needed by items import (XLSX read). |
| src-tauri/Cargo.toml:33 | `rust_xlsxwriter = "0.64"` | export | REMOVE | Only in export fns (:28, :323). Not needed unless an XLSX template download is added (not in v1 scope). |
| src-tauri/Cargo.toml:21 | `rusqlite` features `backup` | BackupService | KEEP | |
| src-tauri/Cargo.toml (uuid) | `uuid` | tests/migration temp names | KEEP | Check other uses before touching; also used by non-migration code? only temp-file names found in migration + tests — low value to remove; confirm with a build. |
| src-tauri/tests/migration_service_test.rs (all, 137 lines) | TC-MIG-001/002/003 | VedBill | REMOVE (delete file) | |
| src-tauri/tests/e2e_scenarios_test.rs:15,344-417 | `MigrationService` import, `create_scenario_vb_archive`, `test_scenario_s5_legacy_vedbill_cutover_and_reconciliation` | VedBill | REMOVE | Header comment :2 "S1 to S5" -> S1-S4; `zip`/`File` imports. S4 (:288-342) is named "bulk catalog import" but does NOT call the import service: KEEP, optionally rewrite to call `ItemImportService`. |
| src-tauri/tests/e2e_tier3_combinations_test.rs:22,36,50,47,66-107,307-356 | `MigrationService` import, `migration_service` harness field, `create_test_vb_archive`, `test_tc_t3_cmb_004_legacy_migration_reindex_checkout_cash_flow` | VedBill | REMOVE (harness field ADAPT) | Combo 4 also covered migrate -> reindex -> checkout; re-cover with items-import equivalent. |
| src-tauri/tests/common/mod.rs:27,97-98,107 | `MigrationService` import; `ImportExportServiceImpl`, `DataExportImportServiceImpl`, `MigrationServiceImpl` alias | DI/test | ADAPT | Replace with `ItemImportServiceImpl` alias. `BackupServiceImpl` (:96) KEEP. |
| src/pages/MigrationPage.tsx (all, 244 lines) | VedBill card (:112-180), Excel Import/Export card (:190-228), RunMigration confirm (:233) | VedBill + exports + import UI | REMOVE page; MOVE import UI | Rebuild items-import dialog on InventoryPage (UI spec :166: file pick -> dry-run report -> strategy -> execute). |
| src/App.tsx:21,42,43 | `MigrationPage` import; routes `migration`, `data-migration` | route | REMOVE | |
| src/components/layout/Sidebar.tsx:109-113 (+ `Database` icon import :14) | `SYSTEM_ITEMS` "Data Migration" entry | route/nav | REMOVE | Check `Database` icon isn't used elsewhere (only :112). |
| src/components/layout/Header.tsx:27-28 | title map `migration`, `data-migration` | route | REMOVE | |
| src/store/useAppStore.ts / src/types/store.ts | route type | route | ADAPT | rg found no explicit `migration` union there; confirm `activeRoute` type is string. |
| src/__tests__/stores/appStore.test.ts:60-72 | TC-APP-006 uses `data-migration` route | route | ADAPT | Use another secondary view. |
| src/api/ipc.ts:132-134 / :160-162 | `archiveItem`, `archiveParty` | FR-17 | REMOVE | |
| src/api/ipc.ts:5,6,18,22,40 ; :237-262 | imports + `scanBackupDirectory`, `inspectBackupFile`, `executeMigration`, `exportItemsToExcel`, `exportTransactionsToExcel` | VedBill/export | REMOVE | `importItemsFromExcel` (:252) MOVE -> dry-run/execute methods. |
| src/api/ipc.ts:230-235 | `backupDatabase`, `verifyDatabaseIntegrity` | backup | KEEP | |
| src/types/api.ts:217-261 | `BackupFileInfo`, `TableStats`, `BackupInspection`, `MigrationReport`, `TransactionExportFilter` | VedBill/export | REMOVE | `ImportSummary` (:251) ADAPT to import dry-run/result types. |
| src/types/api.ts:281,293,324-334 | `archiveItem`, `archiveParty`, scan/inspect/execute/export methods in `ApiClient` | FR-17/VedBill/export | REMOVE | `importItemsFromExcel` (:329) MOVE/ADAPT. |
| src/api/mock/mockClient.ts:250-253, 513-516 | `archiveItem`, `archiveParty` | FR-17 | REMOVE | |
| src/api/mock/mockClient.ts:713-770 | scan/inspect/execute/exportItems/exportTransactions mocks; `importItemsFromExcel` (:759) | VedBill/export/import | REMOVE; import mock ADAPT | Mock imports at :3,4,16,20 and `TransactionExportFilter`. Seed name `vedbill_backup.db` goes with scan. |
| src/api/mock/seedData.ts:63,382-489 | `isActive: true` | DD-17 | KEEP | No archived seed data. |
| src/__tests__/api/mockApiClient.test.ts:110-121 | TC-MOCK-010 archive then search | FR-17 | REMOVE | Replace with a test that inactive items (seed with `isActive:false`) are excluded, if filter is retained in mock. |
| src/__tests__/api/mockApiClient.test.ts:253-275 | TC-MOCK-020 backup | backup | KEEP | |
| src/pages/PartiesPage.tsx:2 (`Archive` icon), :40 (`archiveTarget` state), :93-103 (`handleArchive`), :192-194 (Archive button), :252-259 (`ConfirmDialog`) | party archive UI | FR-17 | REMOVE | `ConfirmDialog` import (:6) only if unused elsewhere in file. Calls `api.archiveParty` at :96. Page then has edit-only actions. |
| src/pages/ReportsPage.tsx:2 (`Download`), :71-78 | `Export` button with `alert('Report export initiated.')` | exports | REMOVE | Dead stub; remove `Download` import. |
| src/pages/SettingsPage.tsx:41-123,162-191 | Backup Now + Verify Integrity | backup | KEEP | No migration card here (migration is only a sidebar route). |
| src/components/layout/HotkeyBar.tsx:49 / App.tsx:76 | `Ctrl+B` -> settings/backup | backup | KEEP | No hotkey belongs to deferred features. |
| src/pages/InventoryPage.tsx | no import/export/archive UI | - | ADAPT (add) | Nothing to remove; the in-scope "Import Items" button/dialog (UI spec :166) does not exist yet. |
| PROJECT.md:24,29,38,43,44,53,105,119,124,184 | tracker rows/tree mentioning `MigrationServiceImpl`, `ImportExportServiceImpl`, `archive_item`, `archive_party`, Migration Controller, `MigrationPage.tsx` | docs | ADAPT | Update after removal (rows 9, 14, 23, 28, 29, tree lines). |
| TEST_INFRA.md:19-20 | `migration_service_test` row; "S1-S5" | docs | ADAPT | Drop row; S1-S4. |
| CHECKPOINT.md:7 | "59 backend tests" | docs | ADAPT | -> 54 (and Vitest count if TC-MOCK-010 removed). |
| docs/book_keeper_data_import_export_and_migration.md (whole; banner already notes v1 = §4 only; LLD :6 links it; §6.1-6.4 UI, IPC `export_data`, `inspect_vedbill_backup`, `execute_vedbill_migration`, Cargo `zip = "2.1"` at :329) | spec study | docs | KEEP (already scoped) | Optionally trim; no code consequence. |
| docs/book_keeper_HLD.md, LLD.md, UI_LLD.md, UI_UX_Specification.md, DESIGN_DECISIONS.md | already aligned (deferred markers at HLD :64-67,100; LLD :40-41,625,697,1188,1448,1520; UI spec :166,175,263,271,285,326) | docs | KEEP | LLD :784 `PartyService` row still says "archive guards" -> ADAPT; LLD :291 comment "Soft-archived" fine. |
| README.md, install.txt | no references | docs | KEEP | `install.txt` zip hits are Node download only. |

## 3. Ordered removal plan

1. **Schema first (prerequisite for MOVE).** DD-16 needs `OPENING`-source lots/`OPN-` headers. `purchase_transactions` (migrations.rs:56) has no `source` column and the `PurchaseTransaction` entity has none; add per the FR-11/FR-05 review decisions before writing the importer. This is independent of removals but blocks a correct import.
2. **MOVE (build new before deleting old).**
   - Create `services/contracts/item_import_service.rs` + `services/impl/item_import_service_impl.rs` (dry-run, execute with SKIP / OVERWRITE / ADD_STOCK; CSV via `csv`, XLSX via `calamine`; create items via ItemService validation; one `OPN-` header; Purchase Cost per lot; index each created/updated item in FTS **after commit**).
   - Create `api/import_controller.rs` (dry-run + execute commands); register in `main.rs`, `api/mod.rs`; add `item_import_service` to `AppState` / `app_state.rs`.
   - Frontend: new import types in `types/api.ts`, `ipc.ts`, mock; build the Import Items dialog on `InventoryPage` (hook into `loadItems`).
   - Write the new tests (LLD TC-IMP-001..003+) and make them pass.
3. **DELETE (pure removal, no dependants after step 2).**
   - Rust: `migration_controller.rs`, `migration_service.rs` (+impl), `import_export_service.rs` (+impl), `migration_dto.rs` types (keep/replace `ImportSummary`), `zip` and `rust_xlsxwriter` from Cargo.toml.
   - Rust tests: `migration_service_test.rs`; S5 and Combo-4 tests + their `.vb` helpers; harness/`common` aliases.
   - Archive chain bottom-up: repo impls -> repo traits -> service impls -> service traits -> controllers -> `main.rs` entries.
   - Frontend: `MigrationPage.tsx`; PartiesPage archive UI; ReportsPage Export button; archive/migration/export methods in `ipc.ts`, `types/api.ts`, `mockClient.ts`; TC-MOCK-010.
4. **ADAPT (shared files).** `main.rs` handler list, `app_state.rs` DI, `services/impl/mod.rs`, `services/contracts/mod.rs`, `core/dtos/mod.rs`, `api/mod.rs`, `App.tsx` (routes), `Sidebar.tsx` (SYSTEM_ITEMS, `Database` icon), `Header.tsx` (title map), appStore TC-APP-006, `tests/common/mod.rs`, then docs (PROJECT.md, TEST_INFRA.md, CHECKPOINT.md, LLD :784).

Dependency notes: `main.rs` must change in the same commit as controller deletion (the `tauri::generate_handler!` list fails to compile otherwise). Trait method removal (`archive`) must precede/accompany impl removal in one commit because `Arc<T>` forwarding impls in the contracts call them. The frontend `ApiClient` interface, `ipc.ts` and `mockClient.ts` must change together (TS interface conformance). Remove `zip` from Cargo.toml only together with the 3 test helpers that use it (`migration_service_test.rs`, `e2e_scenarios_test.rs:373`, `e2e_tier3_combinations_test.rs:87`).

## 4. Tests to delete / adapt

Delete:
- `src-tauri/tests/migration_service_test.rs` (TC-MIG-001/002/003).
- `e2e_scenarios_test.rs`: `test_scenario_s5_legacy_vedbill_cutover_and_reconciliation` (:392-417) and helper `create_scenario_vb_archive` (:344-390).
- `e2e_tier3_combinations_test.rs`: `test_tc_t3_cmb_004_legacy_migration_reindex_checkout_cash_flow` (:307-356) and helper `create_test_vb_archive` (:66-107); drop the `migration_service` harness field (:22/:36/:47/:50).
- `src/__tests__/api/mockApiClient.test.ts` TC-MOCK-010 (:110-121).

Adapt:
- `appStore.test.ts` TC-APP-006 (:60-72) route list.
- `tests/common/mod.rs` re-exports/aliases.
- `e2e_scenarios_test.rs` header (:2) and unused imports (`MigrationService`, `File`, `zip`, `rusqlite` if no longer used); `e2e_tier3` imports likewise.
- Optional: S4 (:288) to drive the real importer.

Keep: `search_indexer_test` TC-SRCH-003 (inactive exclusion), TC-MOCK-020 (backup), all FIFO/restore-for-return tests (the word "restore" there is unrelated).

Count: 59 backend tests -> 54; Vitest 102 -> 101 (before new import tests).

## 5. Risks (in-scope behaviour that would be lost or is already wrong)

- **HIGH — Items import would be lost entirely if the old service is deleted before `ItemImportService` exists.** `import_items_from_excel` is the only importer and `MigrationPage` the only UI. No backend test covers it, so deletion is silent. Mitigation: plan step 2 strictly before step 3.
- **HIGH — The existing importer cannot be moved as-is; it violates DD-16.** (a) No dry-run and no SKIP/OVERWRITE/ADD_STOCK strategy: matches by barcode or name and always overwrites master fields, silently. (b) Opening lot unit cost is `default_sale_price * 0.7` (:197), not Purchase Cost; no `Purchase Cost` column. (c) One synthetic `IMP-<id>` purchase header per item with `Paid` status and hard-coded notes, not one `OPN-` header with `OPENING` source (no `source` column exists). (d) Lot-creation errors are swallowed (`if let Ok`, `let _ =` at :207-221), leaving items with `current_stock > 0` but no lot (FIFO integrity break). (e) Header detection uses `contains("name")` so "Item Name"/"Category Name"/"Base Unit" collisions are possible, no `.csv` support despite `csv` dependency. (f) No bundle/packaging columns (LLD :1511).
- **MEDIUM — FTS indexing.** `ImportExportServiceImpl::new(uow)` has no indexer, so imported/overwritten items are never indexed and are unsearchable at billing (only `MigrationServiceImpl` indexed, :295-299, after commit). The new service must index after commit (indexer uses a separate connection; see S4 comment in `e2e_scenarios_test.rs:307`). Overwritten items must be re-indexed too.
- **MEDIUM — Cargo.toml removal ordering.** Dropping `zip` before deleting the three test helpers breaks `cargo test` compile; `csv` is unused today but required by v1 (do not remove).
- **MEDIUM — Dead/broken UI left behind if only backend is removed.** PartiesPage Archive button, MigrationPage route (Sidebar/Header/App), ReportsPage alert button, `ipc.ts`/mock methods; TS conformance with `ApiClient` forces them to go together.
- **LOW — `is_active` can no longer be set to 0 by any v1 path** (intended). The mock client's TC-MOCK-010 coverage of the filter disappears; keep TC-SRCH-003 and consider a seeded inactive item test.
- **LOW — `AppError::MigrationError` and `run_migrations`/`migrations.rs`** are schema-migration code that match the grep "migration"; they must not be removed.
- **LOW — Docs drift:** PROJECT.md tracker rows 9/14/23/28/29/38, TEST_INFRA.md, CHECKPOINT.md test counts, LLD :784 "archive guards".
