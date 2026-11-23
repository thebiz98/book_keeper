# Book Keeper — Checkpoint & Resumption Guide

*Last updated: 2026-11-21.* Status per milestone lives in `PROJECT.md` (the single tracker). Test layout and gate are in `TEST_INFRA.md`.

## State
All milestones M1–M6 are done. Gate (`bash run_e2e_tests.sh`): 54 backend tests on real SQLite, clippy clean, `tsc` clean, 101 Vitest tests, `vite build` succeeds.

| Layer | Location |
|---|---|
| Domain, DTOs, contracts | `src-tauri/src/core`, `src-tauri/src/{data,services}/contracts` |
| Services | `src-tauri/src/services/impl` |
| SQLite (pool, schema v4, unit of work, FTS5, 12 repos) | `src-tauri/src/data/sqlite` |
| Bootstrap | `app_state::build_sqlite_app_state(db_path)` |
| IPC (51 commands) | `src-tauri/src/api`, `src-tauri/src/main.rs` |
| Frontend | `src/` — one IPC boundary in `src/api/ipc.ts` (`invokeCmd`, converts snake_case ↔ camelCase) |

## Conventions that matter
- Repositories follow `src-tauri/src/data/sqlite/mod.rs`: table/column naming, enums stored as TEXT, explicit id inserts.
- Never call `uow.begin_transaction()` while a transaction is open (`BEGIN IMMEDIATE` deadlocks until busy_timeout). Use the open `tx`.
- The search indexer writes through its own connection, so index only **after** commit.
- Frontend types are camelCase only. Rust DTOs stay snake_case; conversion happens only in `invokeCmd`.

## Environment
Termux (aarch64): cargo and node need the sandbox disabled. In the sandbox `find`/`grep` fail; use `rg`. A full `tauri build` (window bundle) can't run on Termux because the webview libs are missing; build the desktop bundle on a desktop OS.

Windows 11 ARM64 (verified 2026-11-22): requires VS Build Tools 2022 with the **ARM64** component (`Microsoft.VisualStudio.Component.VC.Tools.ARM64`); x64-only tools fail with `linker link.exe not found`. Run cargo from PowerShell/cmd, not Git Bash, because Git's `/usr/bin/link` shadows MSVC `link.exe`. Dev run without installing: `scripts\dev-launch.cmd` (`npx tauri dev`); DB lives at `%APPDATA%\com.bookkeeper.app\shop.db`.

## Branch feat/backup-reset (2026-11-22, pending review and manual check)
- Compiled-in config `src-tauri/app.config.json` (DD-31). Backup folder stored in the DB and chosen with the native picker (DD-32). Auto backup at startup, hourly, and on close, with rotation (DD-33). Testing-only reset of transactions or all data with a mandatory pre-reset backup (DD-34).
- Schema is now **v2** (backup folder setting). New IPC commands: `get_app_config`, `set_backup_directory`, `reset_data`.
- UI: Settings shows the backup folder, last auto backup, and (only when `testingMode` is true) a Testing tools card with typed confirmation (`RESET` / `DELETE ALL`).
- Docs: `install.txt` section 11 (config, backups, restore), `docs/DEPENDENCY_VULNERABILITIES.md` (npm audit, dev-only, vitest 4 upgrade planned).
- **Manual `tauri dev` checklist (not done yet):** (1) Settings -> Choose folder opens the native picker and the choice survives a restart; Use default works. (2) Startup creates an auto backup when none is recent; Backup Now writes a manual file. (3) Closing after a change writes an auto backup. (4) Old auto/pre-reset files are rotated, manual ones kept. (5) With testingMode true: Clear transactions and Clear all data need the exact phrase, write a prereset backup, and other pages show the cleared data; next sale is SAL-1001. (6) With testingMode false the card is gone and `reset_data` is refused. (7) Restore a backup over shop.db (delete -wal/-shm) and check data.

## Branch feat/quick-create-submit (2026-11-23, pending review and manual check)
- FR-22 / DD-44: shared `FormActions` + `enterToSubmit` (`src/components/ui/FormActions.tsx`). Inline new customer/vendor (PartySelector), Add/Edit Party dialog, Bundle modal and Item drawer now show explicit Save/Cancel. No schema or Rust changes.
- Party dialog: Enter saves, validation/backend errors render inside the dialog, double-submit guarded. Bundle modal: Enter saves except in Bundle Name with suggestions open and in Outer Barcode (scanner-safe); Ctrl+Enter saves anywhere.
- **Manual check list:** (1) Alt+C on New Sale shows Save/Cancel; click Save selects the new customer. (2) Alt+V on New Purchase works the same way. (3) Alt+B: Enter saves; scanning a barcode into Outer Barcode does not submit. (4) Parties dialog: Enter saves, a duplicate-name error shows inside the dialog. (5) Payment dialog (SaleSummaryModal) with inline party create: Enter in the create form must not also complete the sale. (6) Check light and dark themes.

## Branch feat/sticky-table-headers (FR-10 rev., DD-43; frontend only, no schema)
- Shared `Table` has a sticky opaque header and four scroll modes (`bounded` default, `fill`, `page`, `none`). Shipped with the **`border-collapse` + sticky `th` + inset shadow line** variant (the lower-risk fallback); the `border-separate` variant was not used. `DataTable` no longer adds its own scroll box. Full-page lists (Inventory, Sales/Purchase history, Parties, Credit & Dues) fill the page height and scroll inside; cart and inward tables scroll inside and keep the newest line in view; hand-rolled tables moved onto the shared `Table` (nested ones use `none`). `ImportItemsDialog` tables unchanged (already sticky). New tables must use `Table` with a `scroll` mode; nested ones `none`.
- **Manual `tauri dev` checklist (not done yet, light and dark):** (1) Inventory with more than 30 items: the header stays put and is opaque, the pager stays visible, no double scrollbar at 1366x768, and row separator lines look right. (2) Narrow window: horizontal scroll, header scrolls sideways with its columns. (3) Sales and Purchase history, Parties, Credit & Dues behave like 1; expanding a party shows open bills with a plain (non-sticky) header that does not cover the outer header. (4) Inventory row expand shows the lot table correctly. (5) New Sale: add 20 lines; the cart scrolls inside itself, the header stays, the newest line is visible. (6) New Purchase: same. (7) Bill detail and party ledger drawers with many rows: header sticks to the top of the drawer body. (8) Returns forms, Dashboard recent, Reports valuation look right. (9) Very small window: `main` scrolls and tables keep 240px.

## Branch feat/item-type (FR-21, DD-42; migration v3)
- Schema is now **v3**: `items.item_type` (`PHYSICAL` default | `SERVICE`). `ItemType` is on `Item`, `ItemSearchResult` and both item payloads; the services reject `SERVICE` (ValidationError field `item_type`). The UI shows a read-only "Type: Physical (stocked item)" line in the item drawer and never sends `itemType`.
- Frozen schema fixtures now live in `src-tauri/tests/fixtures/schema/vN.sql` (v1 = full aligned schema, v2, ...). Each later migration adds its own `vN.sql` and builds the previous database from fixtures 1..N-1. Never edit a shipped fixture; never assert `latest_version() == N`.
- **Pre-migration backup (DD-29):** a real upgrade (database already has applied migrations and one is pending) writes `<db dir>/backups/book_keeper_premigration_v{from}_to_v{to}_YYYYMMDD_HHMM.db` via `VACUUM INTO` before applying anything; failure aborts the upgrade untouched. Fresh, current and in-memory databases write nothing. Rotation deletes these after `retentionDays` (only in the configured backup folder). Later migration tests must tolerate the backups folder.
- **Manual check:** start the app on an existing v2 `shop.db`: it upgrades, a `book_keeper_premigration_v2_to_v3_*.db` appears in `backups/` next to it, existing items open with "Type: Physical (stocked item)", and Add Item shows the same line.

## Branch feat/live-search-low-stock (FR-05 rev., FR-14 rev.; migration v4)
- Schema is now **v4**: `items.low_stock_threshold` (default 5, `>= 0`, 0 = never; PHYSICAL only) and `party_search_fts`. Fixtures `v3.sql` and `v4.sql` added. IPC commands: 50 (`update_item_low_stock_threshold`).
- **Conventions:** every search field uses `useLiveSearch` (`src/hooks/useLiveSearch.ts`; debounce `TYPEAHEAD_DEBOUNCE_MS` 50 / `LIST_SEARCH_DEBOUNCE_MS` 150; no hand-rolled `setTimeout`); typeaheads are built on `LiveSearchTypeahead` (`onCreate` for inline create). FTS: `FtsIndex` / `FtsSpec` / `SqliteFtsIndex` / `fts_prefix_query` / `sync_after_commit` (post-commit, failures logged); specs in `data/sqlite/fts_specs.rs`; new indexes are pushed to `AppState.fts_indexes`, call `ensure_fts_index` at startup (reset-All rebuilds them via `with_fts_indexes`). Low-stock rule lives only in `core::stock_rules` (`is_low_stock`, `LOW_STOCK_SQL`).
- Party search moved from substring to token-prefix (mid-word "mesh" no longer finds "Ramesh"); 3+ digits also match anywhere in the phone.
- **Manual check:** Inventory: type in the search box (results update without a button, Enter is immediate); Barcode column gone; edit a Low Stock Level cell (Enter moves down, Esc reverts, blur saves, -1 shows an error); Status badge changes; 0 turns the alert off. Drawer has "Low Stock Level". Dashboard shows `stock / level`. Parties page, party selector, sales/purchase history and the return bill picker filter as you type; "9876543210" finds "98765-43210". On an existing v3 `shop.db` the app upgrades and writes a pre-migration backup.

## Branch feat/unrecorded-stock (FR-08 rev., DD-40; migration v5)
- Schema is now **v5**: `stock_adjustments` rebuilt (ids and `sqlite_sequence` preserved) so `reason` accepts `UNRECORDED_STOCK`, with `CHECK (reason <> 'UNRECORDED_STOCK' OR (quantity > 0 AND unit_cost IS NOT NULL))`. Fixture `v5.sql` added. IPC commands: 51 (`set_item_stock`).
- `InventoryService::set_item_stock` computes `delta = counted - current_stock` inside its own transaction: delta > 0 posts an `UNRECORDED_STOCK` lot at the entered cost, delta < 0 a `COUNT_CORRECTION` FIFO write-off, 0 is a no-op. Both it and `create_adjustment` share the private `post_adjustment` and reject inactive or SERVICE items. Reports need no change (tests TC-REP-UNREC-1/2 prove valuation-only effect).
- UI: `SetStockDialog` (row action "Set stock" on PHYSICAL items and the item drawer's "Stock on hand" line), `lotCost.ts` (`latestLotCost` = highest `purchaseItemId`, fixes the old date-sorted prefill), `UNRECORDED_STOCK` reason in the adjustment form (increase only, shows valuation effect).
- A `Transactions` testing reset removes unrecorded-stock lots like any adjustment (accepted).
- **Manual check:** Inventory row "Set stock": raise the count (cost prefilled, preview adds value), lower it (count correction, estimated write-off), leave equal (Save disabled), "Add quantity" mode; Reports valuation rises by qty x cost while Purchase History and cash flow stay unchanged; the low-stock badge flips; on an existing v4 `shop.db` the app upgrades and writes a pre-migration backup.

## Next (not started)
- Package and run with `tauri build` on a desktop machine, then do a manual UI pass in Tauri mode (so far only the mock client and IPC contracts have been checked).
- Optional: tighten the remaining `as any` tab-ID casts in ReportsPage and ReturnsAdjustmentsPage.
