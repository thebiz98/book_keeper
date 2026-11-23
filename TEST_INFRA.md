# Test Infra: Book Keeper

## Principles
- **Real persistence.** Every backend test runs on the production SQLite stack (`SqliteUnitOfWork`, schema migrations, FTS5 indexer) against its own temporary database file (`tests/common::TestDb`). No in-memory repository doubles: they hid foreign-key, locking and transaction bugs.
- **Isolated and deterministic.** One DB per test, deleted on drop. Report tests pin "today" with `ReportServiceImpl::with_fixed_today`.
- **One suite per concern, no duplicate matrices.** The former Tier 1/2 suites were deleted (2026-11-21): they did not compile against the real API and duplicated the LLD §11 suites.

## Backend suites (`src-tauri/tests/`)
| Suite | Covers |
|---|---|
| `fifo_strategy_test` | FIFO depletion, split lots, return restore, scrap, adjustment write-off (LLD §11.2) |
| `payment_service_test` | Payments, refunds, settlement (§11.3) |
| `inventory_service_test` | Stock cache, inward, restore, adjustments, unrecorded stock & set_item_stock, low-stock flag (§11.4) |
| `sale_service_test` | Billing, bundles, credit, validation, code sequence (§11.5) |
| `purchase_service_test` | Inward receipts and lots (§11.6) |
| `search_indexer_test` | FTS5 prefix and barcode search (§11.7); generic `FtsIndex` (TC-FTS-01..05) |
| `bill_charges_test` | Bill charges and discounts (FR-19): totals, payment cross-check, applies_to, rounding, purchase lot cost, charge type service, linked refund cap, P&L fields; unit tests in `bill_math`; schema v7 upgrade in `schema_test`; reset in `data_reset_service_test` |
| `bill_correction_test` | FR-20 sale/purchase edit and delete: FIFO repost, own-unit stock check and rollback, payment transitions, party re-pointing, credit/total floors, lot floors, referenced lots, re-costing (COGS, losses, return costs, replay over two lots), vendor-return cost lock, code non-reuse; every case ends with `common/invariants.rs` (ledger, stock cache, lot flow) |
| `return_delete_test` | FR-20 return delete: linked refund/cash/scrap, restocked-consumed, unlinked RETURN lot (incl. legacy notes lookup), credit over two bills, replacement, vendor FIFO/named lot, newest-first rule, code non-reuse; schema v8 upgrade in `schema_test`; `replay_reversals` in `fifo_strategy_test` |
| `expense_service_test` | Store expenses: types, codes, list, validation (TC-EXP-001..010); reports/reset/startup repair in report, reset and smoke suites (TC-EXP-011..015) |
| `party_search_test` | FTS-backed party search (TC-PSRCH-01..08) |
| `accounting_service_test` | Khata dues and FIFO settlement (§11.8) |
| `report_service_test` | Dashboard (today-scoped), P&L, valuation (§11.9) |
| `e2e_lifecycle_test`, `e2e_scenarios_test` | Lifecycle and real-world counter scenarios S1–S4 |
| `e2e_tier3_combinations_test` | Cross-feature combinations |
| `schema_test` | Migration runner, CHECK/enum rules, frozen upgrade fixtures under `tests/fixtures/schema/` (v1, v2, ...), pre-migration backup |
| `item_service_test` | Item/bundle master data, MRP, item type (TC-TYP-01..04) |
| `sqlite_smoke_test` | `build_sqlite_app_state` bootstrap end to end |

Shared fixtures live in `tests/common/fixtures.rs`. Low-level strategy and repository tests use `seed_sale_line` / `ensure_item` / `ensure_purchase` / `ensure_lot` / `seed_adjustment` to create the parent rows that foreign keys require.

## Frontend suites (`src/__tests__/`, Vitest + happy-dom)
Stores (cart, app, theme), components (RapidBillingRow, SaleSummaryModal, StockStatusBadge, CurrencyDisplay, Table sticky header and scroll modes incl. the DataTable single-scroll-container regression in `components/Table.test.tsx`), the API mock client, and the case conversion at the IPC boundary. New suites: `components/FormActions.test.tsx` (Save/Cancel primitive and `enterToSubmit`), `pages/parties.test.tsx` (Add Party dialog: Enter, Save, Cancel, in-dialog errors); `PartySelector` and `BundleCreationModal` suites also cover the FR-22 buttons and Enter rules. The environment is configured in `vite.config.ts` (`test.environment = happy-dom`, `setupFiles`).

## Gate
`bash run_e2e_tests.sh` runs, in order: `cargo test`, `cargo clippy -- -D warnings` (lib), `tsc --noEmit`, `vitest run`, `vite build`. Run it once per milestone. While iterating, run only the affected `cargo test --test <suite>` or vitest file.

On Termux, cargo and node need the sandbox disabled. A full `tauri build` cannot run here because the webview libraries are missing.
