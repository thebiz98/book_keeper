# Phase 2 — Per-Requirement End-to-End Review Plan

**Source of truth:** the aligned design, as of 2026-11-21: `docs/DESIGN_DECISIONS.md` (DD-01…DD-28), then the HLD, Backend LLD, UI LLD and UI/UX spec. The code was written against the *old* design, so many deviations are expected. The reviewers' job is to measure the **gap between the code and the aligned design**, precisely enough that the fix phase can act on it.

Each review is one Sonnet agent. It is **read-only** except for writing its own report `docs/reviews/<ID>.md`. Agents run 3 at a time and do no builds. Running a single `vitest` file is allowed only to settle a specific question.

**Resumption:** a batch is done once all its report files exist. Continue from the first one that is missing.

## Agents (21)

| Batch | ID | Requirement | Primary trace (start here, follow calls anywhere) | DDs to verify |
|---|---|---|---|---|
| 1 | FR-02 | Sales | `NewSalePage`, `RapidBillingRow`, `CartTable`, `SaleSummaryModal`, `useCartStore`, `ipc.ts`, `mockClient` · `sale_controller` → `sale_service_impl` → `sale_repo`, `payment_service_impl` · tests: `sale_service_test`, `cartStore.test`, `SaleSummaryModal.test`, `RapidBillingRow.test` | 01, 02, 10, 15, 18, 27, 28 |
| 1 | FR-01 | Purchases | `NewPurchasePage`, `PurchaseHistoryPage` · `purchase_controller` → `purchase_service_impl` → `purchase_repo`, `inventory_service_impl.add_stock_for_purchase`, `payment_service_impl` · `purchase_service_test` | 01, 02, 07, 15, 25, 28 |
| 1 | FR-03 | FIFO depletion & COGS | `fifo_strategy`, `inventory_service_impl`, `purchase_repo` lot queries, `allocation_repo`, COGS SQL in `report_service_impl` · `fifo_strategy_test`, `inventory_service_test`, `e2e_lifecycle_test` | 05, 09, 10, 11, 13 |
| 2 | FR-04 | Negative stock prevention | UI pre-check (`RapidBillingRow`, cart store), `validate_stock_availability` in every caller (sale, negative adjustment, vendor return, replacement), float tolerance | 10 |
| 2 | FR-06 | Credit & dues | `CreditDuesPage`, `SettlementModal`, settle actions on history pages · `dues_controller` → `accounting_service_impl` → `payment_service_impl`, `settlement_repo`, `party_repo` due summary · `accounting_service_test`, `payment_service_test` | 01, 03, 04 — per-bill Settle on every open bill + lump-sum on account |
| 2 | FR-07 | Returns & replacements | `ReturnsAdjustmentsPage` customer and vendor tabs · `return_controller` → `return_service_impl` → `inventory_service_impl`, `fifo_strategy` reversal, `payment_service_impl`, `accounting_service_impl`, `return_repo` · every test touching returns | 03, 05, 06, 07, 11, 13, 23 — incl. linked-return-only-own-bill, any-item replacement, colour swap restock |
| 3 | FR-08 | Stock adjustments | `StockAdjustmentModal`, Returns page tab 3, dashboard quick action · `adjustment_controller` → `inventory_service_impl.create_adjustment`, strategy outflow, `adjustment_repo`, adjustment-allocation repo · `inventory_service_test` | 07, 08, 11, 12 |
| 3 | FR-09 | Financial reports | `ReportsPage`, `DashboardPage` KPIs, `DateRangePicker`, header cash pill · `report_controller` → `report_service_impl` (every SQL statement) · `report_service_test` | 01, 07, 13, 14 |
| 3 | FR-05 | Low stock | Dashboard low-stock pane, Inventory filter, `StockStatusBadge` · every `update_stock_cache` caller, `get_low_stock_alerts` | — |
| 4 | FR-13 | Bundles / multi-UOM | `BundleCreationModal`, unit selector and `Alt+B` handling in `RapidBillingRow`, Inventory bundle table, `QuantityInput` · bundle IPC → `item_service_impl` → `bundle_repo` (suggestions, free update/delete), multiplier resolution at write time and pack snapshot on sale/purchase lines, any `bundle_id` FK/coupling on bill lines | 21, 28 |
| 4 | FR-14 | FTS5 typeahead | `ItemSearchTypeahead`, header F4 popover · `search_items_fts` → `fts_indexer` (sanitisation, prefix and multi-word queries, bundle-barcode rows, archived exclusion, reindex after commit) · `search_indexer_test` | 24 |
| 4 | FR-15+NFR-06 | Keyboard checkout & ergonomics | `NewSalePage`, `RapidBillingRow`, `SaleSummaryModal`, `Modal`, `useGlobalHotkeys`, `Sidebar`, `HotkeyBar`, `Header` · `create_sale` response contract | 18, 19 |
| 5 | FR-10 | Lists & filters | Sales/Purchase History, Credit & Dues, Parties, Inventory, `DataTable` · history, catalog and party list commands, repo list SQL, pagination | 07, 21 |
| 5 | FR-11 | Items import & backup | `SettingsPage`, `InventoryPage` import, `MigrationPage` · `migration_controller`, `settings_controller` → import/export, migration and backup services · `migration_service_test` | 16, 20, 22 |
| 5 | FR-12 | Invoicing foundation | `transaction_code` value object; code generation per prefix (concurrency, gaps, ADJ/OPN/RTL sequences); `shop_setting_service_impl`; `SettingsPage` profile; sale and purchase detail views | 07, 25 |
| 6 | FR-16 | MRP | MRP through item and bundle create/update, search results, cart display, import | — |
| 6 | DEFERRED | v1 scope removal inventory (FR-17 + deferred FR-11) | Every piece of code for archive, exports, VedBill migration, parties import and restore: IPC registrations, services, repos, UI pages and buttons, `ipc.ts`/mock methods, tests. Produce an exact removal list and its knock-on effects. | 16, 17 |
| 6 | NFR-03 | Immutability & audit | Every mutating SQL statement in the repos (any UPDATE or DELETE on transaction tables beyond the allowed cached fields); schema FKs and CHECKs; IPC surface for edits; mock parity | 01, 22 |
| 7 | NFR-02 | Latency & cache | `current_stock` and `is_low_stock` consistency against lot sums on every mutation path; single dashboard query; indexes in `migrations.rs`; SWR and debounce in the UI | 10, LLD §5.3 |
| 7 | NFR-05 | Pluggable strategy | Strategy trait vs `fifo_strategy`; FIFO assumptions leaking outside the strategy; DI in `app_state` | 05, 09, 11 |
| 7 | NFR-01+NFR-04 | Embedded DB & typed migrations | `connection.rs` pragmas, `sqlite_uow` BEGIN IMMEDIATE, `main.rs` DB path, `migrations.rs` versioning and the schema delta required by the DDs (new columns and tables) | 20, 22, LLD §5.3 |

## Standard agent brief

Each agent receives:
1. the aligned requirement text from the HLD, copied verbatim;
2. the relevant DD entries and LLD sections, copied verbatim;
3. the trace list above;
4. this instruction block.

**Instructions:**
- Trace the requirement end to end, including every test that touches the path:
  - UI page or component
  - store
  - `src/api/ipc.ts`, plus `src/api/mock/mockClient.ts` for behavioural parity
  - Tauri command in `main.rs`
  - controller
  - service(s)
  - repository SQL
  - schema in `migrations.rs`
- For each acceptance clause and each DD rule, classify the code as one of:
  - ✅ matches the aligned design
  - ⚠️ partially matches
  - ❌ missing or contradicts the design
  - 🐞 bug, independent of the design change: logic error, edge case (zero/negative/fractional qty, rounding, empty input, repeated operation), broken atomicity or rollback, error not propagated, wrong UI↔Rust contract, layering violation
- Assess test coverage. Which LLD §11 test IDs exist and actually assert the right thing? Which are missing? Are any assertions weak?
- Code quality: dead code, duplication, responsibility-matrix violations (LLD §6.1).
- **Read-only.** Edit only your own report. Do not run cargo.
- Write `docs/reviews/<ID>.md` with these sections:
  1. Scope traced (files)
  2. Acceptance and DD checklist (status + `file:line` evidence)
  3. Findings table: `ID | Severity (Critical/High/Medium/Low) | Kind (design-gap / bug / contract / test-gap / quality / ux) | file:line | What's wrong | Concrete failure scenario | Change needed (in aligned-design terms) | Overlaps (req IDs / DD IDs)`
  4. Test coverage: what is tested, what is missing, which LLD TC-IDs to add
  5. Schema and contract delta: columns, tables, DTO fields and IPC commands this requirement needs that don't exist yet
- **Final message** to the main session, at most 40 lines:
  - counts by severity;
  - each Critical or High finding on one line (ID, `file:line`, summary, overlaps);
  - any cross-requirement issues spotted outside the agent's own scope.

## After all batches
Write `docs/reviews/99_CONSOLIDATED.md`:
- Deduplicate findings by root cause.
- Group them into fix themes, e.g. "payment ledger", "lot sources", "return reversal", "checkout dialog", "scope removal", "schema migration v2".
- Order the themes by dependency (schema → services → IPC → UI → tests).
- For each theme, list the files touched, the requirements and DDs it closes, and the LLD test IDs to implement.

This file is the input to the fix phase.
