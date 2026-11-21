# Consolidated Review → Fix Plan

*2026-11-21. Inputs: the 21 reports in this folder (FR-01 … NFR-01_NFR-04, DEFERRED), all measured against `docs/DESIGN_DECISIONS.md` (DD-01…DD-28) and the aligned HLD and LLD. Finding IDs are cited as `REPORT:ID` (for example `FR-03:F-02`).*

## 1. Overall verdict

The current code implements the **pre-alignment** design. Several core paths are also broken outright, not just out of date:

- Purchases can't be recorded from the UI.
- The payment dialog ignores the cash entered.
- The second customer return fails.
- The vendor payables view is empty.
- The P&L, cash-flow report and cash drawer are wrong.
- Import isn't atomic.

The good parts are worth keeping:

- the transaction boundary: one `BEGIN IMMEDIATE`, rollback on drop;
- FIFO depletion order and per-lot cost;
- FTS query sanitisation;
- the online-backup mechanism;
- the absence of any edit or delete path for committed transactions;
- foreign keys switched on for every pooled connection;
- a `core/` layer free of Tauri imports;
- a DI graph with no cycles.

**Raw counts** (before deduplication): 28 Critical, 105 High, 142 Medium, 66 Low, about 340 findings in total. Many are the same root cause seen from different requirements. After deduplication they collapse into the **14 fix themes** below.

| Report | C | H | M | L | Report | C | H | M | L |
|---|---|---|---|---|---|---|---|---|---|
| FR-01 | 2 | 6 | 8 | 3 | FR-11 | 4 | 8 | 7 | 3 |
| FR-02 | 3 | 7 | 9 | 2 | FR-12 | 0 | 4 | 5 | 5 |
| FR-03 | 2 | 6 | 5 | 2 | FR-13 | 0 | 6 | 8 | 2 |
| FR-04 | 1 | 6 | 6 | 1 | FR-14 | 0 | 6 | 7 | 3 |
| FR-05 | 0 | 2 | 6 | 5 | FR-15+NFR-06 | 2 | 7 | 8 | 3 |
| FR-06 | 3 | 6 | 8 | 4 | FR-16 | 0 | 3 | 6 | 6 |
| FR-07 | 4 | 6 | 9 | 2 | NFR-01+04 | 1 | 5 | 7 | 3 |
| FR-08 | 1 | 4 | 7 | 2 | NFR-02 | 0 | 4 | 9 | 4 |
| FR-09 | 4 | 4 | 7 | 4 | NFR-03 | 0 | 3 | 6 | 4 |
| FR-10 | 1 | 7 | 7 | 2 | NFR-05 | 0 | 5 | 7 | 6 |

## 2. Cross-cutting root causes

Each of these shows up in five or more reports.

| # | Root cause | Seen in |
|---|---|---|
| RC-1 | The schema is the pre-alignment v1. It has none of: ledger `kind`/`return_id`, lot `source`, `returned_quantity`, outflow table, pack snapshot, CHECKs, UNIQUE barcode/name. It still has `bundle_id` FKs on bill lines. Enums are stored PascalCase. | NFR-01_04:N-01..N-05, FR-01:F-02/F-07, FR-02:F-04/F-18, FR-03:F-02/F-05, NFR-03:F-02 |
| RC-2 | Bill money fields have several writers. `update_balance_due` derives paid from a balance delta. Synthetic bills are inserted paid with no ledger row. Refunds are stored as negative typed strings. | FR-02:F-03/F-05, FR-06:F-12, NFR-03:F-01/F-07, FR-07:F-03 |
| RC-3 | Synthetic lots are ordinary purchases (`ADJ-{id}`, `INIT-`, `IMP-`, `PUR-OPEN-`) with invented costs (0.7 × sale price) and non-zero totals. They leak into purchase history, counts, the trend, payables and COGS. | FR-01:F-02/F-05, FR-03:F-08, FR-08, FR-11:F11-03, NFR-03:F-04 |
| RC-4 | Stock is written outside `InventoryService`. The cache is moved by increments with a `.max(0)` clamp, and vendor returns bypass FIFO, so the cache drifts from the sum of lot remainders. No invariant check exists. | FR-03:F-04, FR-04:F-02, FR-08:F-05, NFR-02:F-N04, NFR-05:F-04/F-06 |
| RC-5 | Validation is missing at the service boundary: quantity > 0, finite numbers, price ≥ 0, status/paid cross-check, party required and of the right type, item active, bundle belongs to the item, MRP null or > 0, multiplier > 1. | FR-01:F-03/F-04, FR-02:F-02/F-14, FR-04:F-01/F-05, FR-07:F-04, FR-08:F-04, FR-13:F-02, FR-16:F16-02 |
| RC-6 | No rounding (DD-15) and no tolerance of 1e-9. Float residue leaves bills PARTIAL and lots with slivers. | FR-01:F-06, FR-02:F-06, FR-03:F-07, FR-06:F-07, FR-05:F5-04 |
| RC-7 | Codes are generated ad hoc. The client supplies the code; customer returns fall back to a constant `RET-1001`; vendor returns use `COUNT+1001`; code previews are computed inside a dropped write transaction. | FR-02:F-07, FR-07:F-01/F-12, FR-12:F-01..F-04 |
| RC-8 | Typed `AppError` is stringified in the controllers, and the UI swallows errors into empty lists or zeros. | FR-02:F-11, FR-10:F10-09, FR-09:F-08, FR-11, NFR-02:F-N02 |
| RC-9 | The UI uses UTC dates (`toISOString`), the backend uses Local dates, and some services use UTC. | FR-02:F-17, FR-06, FR-08:F-03, FR-09:F-06/F-10, FR-12:F-13 |
| RC-10 | Every read opens `BEGIN IMMEDIATE`, and every command is synchronous on the main thread. | NFR-02:F-N02/F-N03, NFR-01_04:N-08, FR-12:F-10 |
| RC-11 | The mock client enforces none of the business rules (DD-27), which hides several critical bugs in mock mode. | FR-01:F-10, FR-02:F-12, FR-06, FR-07:F-18, FR-09:F-19, FR-13:F-09, FR-14, FR-16:F16-10 |
| RC-12 | Tests give false confidence. `inventory_service_test.rs` tests a local copy of the service. Tests hand-wire `FifoStrategyImpl`. Some tests lock in forbidden behaviour (TC-SALE-002, TC-SSM-*, TC-RBR-*). Most LLD §11 cases are missing. | FR-03:F-06, NFR-05:F-08, FR-02, FR-15, FR-09:F-15 |
| RC-13 | Lists load page 1 only, sort by id ascending, filter on the client, and have no total/pager. | FR-10:F10-01/02, FR-01:F-15, FR-02:F-10 |
| RC-14 | The UI flows were never built to spec: checkout row, payment dialog, purchase catalog picker, dues bill list, returns linking, adjustment item search, bundle CRUD, item edit, import dialog, hotkey registry. | FR-15, FR-01:F-01, FR-06:F-09, FR-07:F-05, FR-08:F-01, FR-13:F-04/F-10, FR-11:F11-12 |

## 3. Fix themes, in dependency order

Each theme lists what it closes, the main files and the LLD tests to implement. **Backend themes T1–T8 come before frontend themes T10–T12**, because the UI contracts depend on them.

### T0 — Scope removal and import extraction (DD-16, DD-17)
- Follow the `DEFERRED.md` ordered plan:
  1. **Move** the items import into `ItemImportService` and `import_controller` (to be rewritten in T9).
  2. **Remove** exports, VedBill migration, `MigrationPage` and its routes, archive_item/archive_party (backend, ipc, mock, PartiesPage UI) and the Reports Export button.
  3. Drop the `zip` and `rust_xlsxwriter` crates, **after** removing the three test helpers that use `zip`.
  4. Delete 5 Rust tests and 1 Vitest test; adapt TC-APP-006. Update the docs (PROJECT.md, TEST_INFRA.md, CHECKPOINT.md, LLD:784).
- Risk: do not delete `ImportExportServiceImpl` until `ItemImportService` exists.

### T1 — Schema rewrite, migration runner, enums, DB path (RC-1; DD-01/05/06/07/11/12/20/23/25/28)
- **Decided (DD-29):** no real user DB exists, so rewrite `SCHEMA_V1` in place to the aligned design. Add a `schema_version` table and a numbered runner: each step in its own transaction, FKs off during table rebuilds, a backup taken first, and a refusal to open a newer version. Detect a legacy dev DB and fail with a clear message. Add a frozen-fixture upgrade test.
- Schema contents:
  - `payment_settlements`: `kind`, `return_id`, nullable `transaction_id`, `CHECK amount > 0`.
  - `purchase_transactions`: `source`, `supplier_invoice_ref`.
  - `sale_stock_allocations.returned_quantity`.
  - `return_items`: `line_type`, `cost_amount`, `purchase_item_id`.
  - New `stock_outflow_allocations` table, replacing the adjustment allocations table.
  - `stock_adjustments.unit_cost`.
  - Name snapshots (DD-30): `party_name` on sale/purchase/return headers, `item_name` on all bill lines.
  - Bill lines: `pack_label`/`pack_quantity` with **no** `bundle_id`.
  - All CHECKs and indexes from LLD §5.3, including the four missing indexes and a NOCASE index on item name.
  - UNIQUE item name. Barcode uniqueness across items and bundles is enforced in the service.
- Enums: add `serde(rename_all = "SCREAMING_SNAKE_CASE")`. Remove legacy variants (SettlementType, AdjustmentReason, ReturnType). Add PaymentKind, LotSource, OutflowSourceType, ReturnLineType, TransactionType. Ship the TS unions, mock seed data and repo column constants **in the same commit**.
- DB path: `shop.db` in the app-data dir, plus a `BOOK_KEEPER_DB_PATH` override with directory creation. Fix the `shop.db` label in Header.tsx.
- Pragmas: set `busy_timeout` before WAL; add `cache_size`, `temp_store` and a short pool timeout.
- Tests: TC-SCH-001..012, TC-PATH-001..003.

### T2 — Data-access foundation (RC-7, RC-10, parts of RC-4)
- `UnitOfWork::begin_read()` (deferred transaction) for every read path; `BEGIN IMMEDIATE` only for writes.
- Make Tauri commands `async` and run them with `spawn_blocking`.
- Add repo helpers: `find_lot_by_id`, `find_sale_item_by_id`, `list_open_for_party` (ORDER BY `transaction_date`, `id`) for both sides, `get_latest_lot_cost`, the returned-quantity sums, and column constants instead of positional reads.
- One `next_code(prefix/source)` helper, called **inside** the write transaction by every creator. Ignore or drop any client-sent codes. Previews are display-only.
- Replace the stubs and recursive default trait methods (NFR-05:F-16).

### T3 — Payment ledger (RC-2, RC-6; DD-01/02/03/04/15)
- `PaymentService` is the **only** writer of the ledger and the bill money fields: `record_payment` (UPFRONT, SETTLEMENT, RETURN_CREDIT), `record_refund` (REFUND_OUT, REFUND_IN) and `recompute_bill`. Delete `update_balance_due`, `reduce_transaction_balance` and every direct write of `paid_amount`/`balance_due`.
- Sale and purchase: cross-check status against the amount paid; party required for credit and of the right type; purchase overpayment rejected; sale change computed; `round2` everywhere.
- `AccountingService`: per-bill and lump-sum settlement for **both** sides, with up-front over-settlement rejection, bill/party/type checks and `apply_return_credit`.
- `get_party_ledger`: a real implementation.
- Closes FR-01:F-03/04/06/07, FR-02:F-02/03/04/05/06/13/20, all of FR-06, NFR-03:F-01/F-06/F-07.
- Tests: TC-PAY-001..007, TC-DUES-001..007, TC-SALE-001..011, TC-PUR-001..005.

### T4 — Inventory engine and strategy (RC-3, RC-4, RC-6; DD-05/07/08/09/10/11; NFR-05)
- Inject the strategy from `app_state` (NFR-05:F-01). The trait becomes: `validate`, `deplete_lots_for_sale`, `reverse_sale_allocations(restock)` and `deplete_lots_for_outflow`, with EPS applied. Remove the `FifoStrategyImpl` alias.
- `InventoryService` is the sole stock writer:
  - aggregated `validate_stock_availability`;
  - `reverse_sale_line`;
  - `create_source_lot` (ADJUSTMENT, OPENING, RETURN; zero-value header; own sequence);
  - `issue_stock_outflow`;
  - `remove_stock_for_vendor_return` (named lot with qty ≤ remaining, otherwise FIFO; logs outflow rows).
- Stock cache updates use the delta the strategy actually moved, with no clamp. A single `LOW_STOCK_THRESHOLD` constant. `verify_stock_cache()` runs at startup (repair and log).
- Remove stock writes from ItemService, import and migration (NFR-05:F-04), and stop `ItemRepository::update` from overwriting cache columns.
- Closes FR-03 (all), FR-04:F-02/F-04/F-05, FR-05:F5-05, FR-08:F-05, NFR-02:F-N01/F-N04, NFR-05:F-01/F-03..06.
- Tests: TC-FIFO-001..008, TC-INV-001..009 (rewritten against the **production** service).

### T5 — Returns and replacements (DD-03/05/06)
- Rewrite `ReturnService` per LLD §6.9:
  - validation before writes (line belongs to the bill, item matches, qty ≤ sold − returned summed across duplicates, price cap, party defaulting);
  - linked returns credit only their own bill, with the remainder as cash;
  - unlinked returns go to the party's oldest bills first, or all cash for a walk-in;
  - vendor returns are symmetric (RETURN_CREDIT, then REFUND_IN);
  - REPLACEMENT issues any item, with no money; issued cost goes to COGS;
  - totals stored and rounded; `list_returns` and `get_return_details`; `ReturnResult` with the credited and cash amounts.
- Closes FR-07 (all), FR-04:F-01/F-03, FR-12:F-02/F-03.
- Tests: TC-RET-001..012.

### T6 — Stock adjustments (DD-08/12)
- Validation: quantity non-zero and finite, date valid, item active, the four reason values. Negative adjustments go through `issue_stock_outflow` and return the loss value. Positive adjustments go through `create_source_lot`: latest lot cost, or the cost the user entered; never the sale price.
- `list_stock_adjustments` paginated in SQL, newest first.
- Closes FR-08 (backend).

### T7 — Reports (DD-13/14; RC-2 consumers)
- Implement the LLD §6.10 metric table exactly:
  - the full P&L statement, date-filtered, net of returns, with replacement COGS and losses;
  - cash flow from ledger buckets (fixes the today-only filter and the sign errors);
  - a single shared `live_cash_in_drawer`;
  - trend with period buckets, zero-fill and `source = PURCHASE`;
  - valuation counts and categories;
  - low-stock ordered by `current_stock ASC` and uncapped (or with an explicit total);
  - `get_recent_transactions`.
- Move SQL into a real `ReportRepository` or delete that trait. No whole-table loads and no N+1 queries.
- Closes FR-09 (all), FR-05:F5-01, NFR-02 dashboard items.
- Tests: TC-REP-001..006, TC-DUES-003.

### T8 — Lists and DTOs (RC-13; DD-21)
- One `Paginated<T> { items, total, page, limit }` for every list. Filtering, sorting (newest first) and paging done in SQL, with page/limit validation.
- `SaleHistoryRow`/`PurchaseHistoryRow` carry the party name, line count and `search_query` (code or party name; purchases also match the invoice ref).
- `PartyListRow` carries the outstanding balance and transaction count for both sides. The dues summary is a single query with total invoiced and total paid.
- Detail views: pack snapshot, `supplier_invoice_ref`, ledger rows, stored `unit_price`.
- Closes FR-10 (backend), FR-01:F-15, FR-02:F-10/F-19, FR-12:F-07.
- Tests: TC-LIST-01..09.

### T9 — Items, bundles, MRP, search, import, backup (backend)
- **Items and bundles** (DD-21/28):
  - `update_bundle` and `delete_bundle` with no reference guard;
  - validation: name, multiplier > 1, prices ≥ 0, MRP null or > 0, barcode unique across items and bundles, UNIQUE (item, bundle_name);
  - suggestions query: case-insensitive, run as a read;
  - reindex after commit, with errors logged rather than swallowed;
  - rebuild the search index at startup on mismatch.
- **Search:**
  - `matched_bundle_id` in results;
  - SQL LIMIT with batched bundle loading;
  - remove or escape the LIKE fallback.
- **Import:** rewrite per LLD §6.14:
  - CSV and XLSX, exact header map, dry run with row errors, SKIP/OVERWRITE/ADD_STOCK;
  - atomic, one `OPN-` header, cost from the Purchase Cost column, index after commit.
- **Backup:**
  - never overwrite: `_HHMM` name with a `_2` suffix on collision;
  - destination must be an existing writable directory;
  - reject the live DB path.
- Closes FR-13, FR-14 (backend), FR-16:F16-02/04/05, FR-11 (all backend), NFR-02:F-N09.
- Tests: TC-SRCH-*, TC-BUN-001..009, TC-MRP-*, TC-IMP-001..004.

### T10 — Error contract and dates (RC-8, RC-9; DD-26)
- Controllers return the serialized `AppError` (`{type, details}`). `invokeCmd` rejects with a typed error. Every page shows `details.message` and stops treating a failure as "empty" or "0".
- A shared `todayLocal()` / `toLocalDate()` helper in the UI. The backend uses `chrono::Local` for business dates everywhere, and `created_at` is ISO with an offset.

### T11 — Frontend flows (RC-14; DD-04/18/19/21/27/28)
- **Checkout:**
  - mount a production `RapidBillingRow` (Search → Qty → Unit → Rate → commit; aggregated stock check; pack display);
  - Enter on an empty search or `Ctrl+Enter` opens the payment dialog. The dialog has payment and result modes: status selector, customer rule with Alt+C, cash sent as `paidAmount`, submit guard, errors shown inside the dialog;
  - no client code; delete `recordCashFlow` and refetch metrics instead.
- **Purchase:** catalog picker and bundle selection, real item ids, `Ctrl+Enter`, the payment cross-check.
- **Hotkeys:** one registry (F1–F6, Ctrl+F, Alt+B, Alt+C/Alt+V, Ctrl+B runs a backup, Esc layering) that drives the HotkeyBar and Sidebar.
- **Modal:** stable effect dependencies, a focus trap, and a single Enter handler. `PartySelector` becomes keyboard-operable with inline create.
- **Typeahead:** Enter selects even while results are still loading (barcode scanners), a stale-response guard, a 50 ms debounce, limit 8.
- **Dues:**
  - a per-party open-bill list with a per-bill **Settle**, plus **Receive/Pay on account** with a split preview;
  - vendor tab wording;
  - a ledger view.
- **Returns:** bill/party pickers, returnable quantities, a credit-vs-cash preview, the REPLACEMENT issue list, the vendor lot picker, and a returns history list.
- **Adjustments:** item search, a date field, the four reasons, decimal quantities, unit cost when required, and history with a pager.
- **Inventory:** item edit (including clearing MRP), a bundles sub-table with edit and delete, a lot inspector, the import dialog, server-side filters, and a pager.
- **Dashboard and Reports:**
  - all KPIs, recent transactions, a 7D/30D/1Y toggle, the Stock Adjustment action;
  - working date presets, no Export button;
  - shop settings loaded on start, with no fake defaults.
- **Lists:** adopt the paginated contract, with a pager on every page.

### T12 — Mock client (DD-27 thin fixtures)
- Strip business logic from `mockClient.ts`; keep seed data and contract-shaped responses (camelCase, typed `AppError` where simulated). Delete mock-behaviour tests (`mockApiClient.test.ts` rule assertions); keep a contract-shape test.

### T13 — Test suite overhaul (RC-12)
- One AppState-based harness on real SQLite for all backend tests. Delete the local-copy inventory tests and the hand-wired strategy. Add a strategy contract test.
- Implement every LLD §11 TC-ID listed in T1–T9, plus the immutability tests (NFR-03:F-11) and concurrency/rollback tests.
- Replace the TC-SSM/TC-RBR tests that lock in old behaviour. Add frontend tests for the checkout keyboard flow, payment dialog, hotkeys, modal focus, typeahead race, settlement modal and returns form.
- Perf tests at realistic volume against the stated targets (NFR-02:F-N12).

## 4. Decisions (resolved 2026-11-21: Q1 → DD-29 rewrite in place, Q2 → DD-27 thin fixture mock, Q3 → DD-30 name snapshots)

| # | Question | Options | Recommendation |
|---|---|---|---|
| Q1 | Schema strategy | (a) rewrite SCHEMA_V1 in place plus a versioned runner; (b) incremental v2..v5 migrations with lossy backfills | **(a)**. No user DB exists, and nearly every table needs rebuilding anyway. |
| Q2 | Mock client | (a) full business-rule parity (DD-27); (b) thin fixture mock for UI layout only, with behaviour tested against the real backend | **(b)**. Parity doubles every rule and has already diverged everywhere. Revise DD-27. |
| Q3 | Names on historical bills | (a) live join (renaming a party or item changes old bills); (b) snapshot `party_name`/`item_name` on headers and lines | **(b)** for item names on lines and party name on headers, matching the pack-label snapshot. |

## 5. Suggested fix-phase execution

Commit per phase on `main`, with Sonnet agents on disjoint files and at most 3 concurrent, as in the earlier milestones:

| Phase | Themes | Parallelism |
|---|---|---|
| F1 | T0 scope removal, then T1 schema, enums and paths | sequential (schema touches everything) |
| F2 | T2 data-access foundation + T10 backend error contract | 2 agents |
| F3 | T3 ledger · T4 inventory/strategy | 2 agents (disjoint services), then integrate |
| F4 | T5 returns · T6 adjustments · T9 items/bundles/search/import/backup | 3 agents |
| F5 | T7 reports · T8 lists/DTOs | 2 agents |
| F6 | T11 frontend: (a) checkout, payment and hotkeys, (b) purchase, inventory and import, (c) dues, returns, adjustments, reports and lists + T10 UI | 3 agents |
| F7 | T12 mock · T13 test overhaul and perf · docs | 2–3 agents; full gate |

Gate at the end of each phase: targeted `cargo test --test …` plus `tsc` for that phase. The full `run_e2e_tests.sh` runs only at F3, F5 and F7.
