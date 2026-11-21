# NFR-02 Review: Fast Query & Dashboard Latency (cached stock, single dashboard payload, sub-ms typeahead)

Reviewer: Sonnet agent, read-only. Aligned design: DESIGN_DECISIONS (DD-10, DD-11), HLD NFR-02 and 9.2, LLD 5.3, 6.3 (invariant `current_stock == sum(remaining)`), 10.2, UI LLD 7.2.
Already-known items are referenced by report and ID and not re-derived: FR-09 F-09, FR-05 F5-01/F5-05, FR-14, FR-10 F10-05/06, FR-03 F-04, FR-08 F-05.

## 1. Scope traced
- Backend: `src-tauri/src/data/sqlite/{connection,sqlite_uow,migrations,fts_indexer}.rs`, repos `item_repo, purchase_repo, sale_repo, allocation_repo, settlement_repo, adjustment_repo, return_repo`.
- Services: `inventory_service_impl, fifo_strategy, item_service_impl, purchase_service_impl, return_service_impl, report_service_impl, import_export_service_impl, migration_service_impl, accounting_service_impl`; `api/report_controller.rs`, `main.rs`, `app_state.rs`.
- Frontend: `DashboardPage.tsx`, `ItemSearchTypeahead.tsx`, `Header.tsx`, `package.json`.
- Tests: `report_service_test.rs:20-53`, `search_indexer_test.rs:31-42`, `e2e_tier3_combinations_test.rs:501-504`.

## 2. Checklist
| Clause | Status | Evidence |
|---|---|---|
| Cached `current_stock`/`is_low_stock` exist on item | OK | `migrations.rs:19-30` |
| Low-stock widget reads cached flag | Partial | `report_service_impl.rs:125`, `item_repo.rs:80-81`: flag used, but no index, capped 50 (FR-05 F5-01) |
| Dashboard single payload, < 2 ms | No | `report_service_impl.rs:36-45` loads whole tables (FR-09 F-09). Page still issues 5 IPC calls, `DashboardPage.tsx:34-40` |
| Cache moves with lots in same txn, same delta | Partial | see section 3 |
| Invariant check/repair | Missing | no startup check, no test (rg finds none in `src-tauri/tests`) |
| LLD 5.3 indexes | Partial | see section 4 |
| WAL / synchronous / busy_timeout | OK | `connection.rs:9-11` |
| Reads avoid write lock | No | every read uses `begin_transaction` = BEGIN IMMEDIATE |
| SWR / no full-page loaders (UI LLD 7.2) | No | no SWR layer; refetch-on-mount only |
| Perf claims tested | Weak | section 6 |

## 3. Cache consistency audit
Every path that creates a lot or changes `remaining_quantity`, and the cache write beside it:

| # | Path | Lot change | Cache write | Same txn? | Same delta? | Mode |
|---|---|---|---|---|---|---|
| 1 | Sale `deplete_stock_for_sale` `inventory_service_impl.rs:41-60` + `fifo_strategy.rs:56-59` | lot -= take (sum = qty) | `max(item.current_stock - qty, 0)` | yes | not if cache already drifted (clamp) | increment from cached |
| 2 | Purchase `add_stock_for_purchase` `:62-80` | create lot qty | `+ quantity` | yes | yes | increment |
| 3 | Customer return `restore_stock_for_return` `:83-103`, `fifo_strategy.rs:135-147` | lot += restore | `+ qty` | yes | yes | increment |
| 4 | Vendor return `:105-140` | only if `purchase_item_id` is Some, lot `.max(0)`; else NO lot change | always `- qty` | yes | NO (see F-N02-03) | increment |
| 5 | Negative adjustment `:166-168` | lots via strategy | `max(cur - qty, 0)` | yes | clamp can diverge | increment |
| 6 | Positive adjustment `:196-204` | create synthetic lot | `+ qty` | yes | yes | increment |
| 7 | Create item with initial stock `item_service_impl.rs:78-113` | create lot (bypasses InventoryService) | item row created with same stock | yes | yes | direct |
| 8 | Excel import `import_export_service_impl.rs:240-272` | lot create result discarded (`if let Ok`, `let _ =`) | item row created with stock | yes | NO if lot create fails | direct |
| 9 | VedBill migration `migration_service_impl.rs:223-240` | lot per item | item row | yes | yes | direct |
| 10 | Replacement issue (LLD 6.3) | not implemented | n/a | n/a | n/a | see FR-07 |
| 11 | `ItemRepository::update` `item_repo.rs:103-109` writes `current_stock`/`is_low_stock` from the passed struct | none | arbitrary | n/a | n/a | any caller can overwrite the cache (item update reads, then writes, in one txn so safe today, but a latent race and a layering hole vs LLD 6.3 "only InventoryService writes") |

No path recomputes from `SUM(lots)`; all are read-modify-write increments of the cached value. The 5.0 threshold is hard-coded in 8 places (FR-05 F5-05).

Where an invariant check belongs: a pure `InventoryService::verify_stock_cache()` (one SQL: `SELECT i.id, i.current_stock, COALESCE(SUM(p.remaining_quantity),0) FROM items i LEFT JOIN purchase_items p ON p.item_id=i.id GROUP BY i.id HAVING ABS(i.current_stock - COALESCE(SUM(p.remaining_quantity),0)) > 1e-9`). Call (a) from `build_sqlite_app_state` after migrations, repairing by recompute (log count), (b) as a helper asserted at the end of every `e2e_*` test and TC-INV-008, (c) from a Settings "verify database" action next to `verify_database_integrity`. Better: have `update_stock_cache` take a delta and apply `current_stock = current_stock + ?` in SQL, with `is_low_stock` set by one SQL expression, so the value cannot be derived from a stale read.

## 4. Index audit
Created (migrations.rs): items(barcode, name, category); item_bundles(item_id, barcode); parties(party_type); purchase_transactions(transaction_date, party_id) + UNIQUE(transaction_code); purchase_items(purchase_transaction_id), (item_id, remaining_quantity); sale_transactions(date, party_id); sale_items(tx, item_id); sale_stock_allocations(sale_item_id, purchase_item_id); return_*(date, party, tx, item); payment_settlements(party_id, (transaction_type, transaction_id), settlement_date); stock_adjustments(item_id, date); adjustment_stock_allocations(adj, purchase_item).

| LLD 5.3 index | Present? |
|---|---|
| purchase_item(item_id, remaining_quantity) | yes (`migrations.rs:84`) |
| sale_stock_allocation(sale_item_id) | yes (`:122`) |
| stock_outflow_allocation(source_type, source_id) | no table yet (DD-11) |
| payment_settlement(settlement_date), (transaction_type, transaction_id) | yes (`:167-168`) |
| sale_transaction(transaction_date) | yes (`:99`) |
| purchase_transaction(source, transaction_date) | no, `source` column absent; only (transaction_date) |
| item(barcode), item_bundle(barcode) | yes |
| return_item(sale_item_id), return_item(purchase_item_id) | no (only item_id) |

Hot query vs index:
- FIFO active lots `purchase_repo.rs:159` `WHERE item_id=? AND remaining_quantity>0 ORDER BY id`: uses (item_id, remaining_quantity) range, then a temp B-tree sort by id. The lot id order is FIFO only by accident of insertion (LLD wants purchase date; FR-03). Better index `(item_id, id) WHERE remaining_quantity > 0` (partial, no sort, small).
- Allocations by sale_item `allocation_repo.rs:55`: indexed.
- Low stock `item_repo.rs:80-81`: `is_active=1 AND is_low_stock=1 ORDER BY name COLLATE NOCASE, id`: full scan of items plus sort; idx_items_name is BINARY collation so it is unused by any query (all name queries use NOCASE: `item_repo.rs:54`, `:81`). Needs `CREATE INDEX ... ON items(name COLLATE NOCASE) WHERE is_active=1` and partial `ON items(name COLLATE NOCASE, id) WHERE is_active=1 AND is_low_stock=1`.
- Catalog list `item_repo.rs:80`: `(?1 IS NULL OR category=?1) AND (?2=0 OR is_low_stock=1)` constant-folding OR patterns defeat idx_items_category; scan + sort of all items. `idx_items_category` is effectively unused.
- Sale/purchase history `sale_repo.rs:84-179`: range on `transaction_date` but `ORDER BY id`; with date range the planner may use the date index then sort, with no filter it is PK scan (fine). Pagination count `:211` is a second full COUNT. Newest-first ordering (users expect it) would be `ORDER BY transaction_date DESC, id DESC` served by idx_sale_tx_date.
- Next transaction code `sale_repo.rs:144`, `purchase_repo.rs:145`, `return_repo.rs:97`: `WHERE transaction_code LIKE 'SAL-%'` pulls ALL codes into Rust and parses them on every checkout/purchase (LIKE is case-insensitive so the UNIQUE BINARY index cannot be used for range). O(n) on the hottest write path.
- Ledger by party `settlement_repo.rs:60` indexed on party; `list_all` `:85` and `allocations.list_all` `:65`, `list_all_lots` `purchase_repo.rs:229`: full-table loads (dashboard).
- Settlement date-sum `:78`: `settlement_date = ?` with `transaction_type = ?`: uses date index; OK.
- FTS: MATCH on `item_search_fts` joined to items by PK: fine plan, but see F-N05.
- Unused: idx_items_name (collation), idx_items_category, idx_parties_type (low cardinality), idx_sale_items_item (no query found).

## 5. Query plan reasoning (summary)
- `get_dashboard_metrics`: 5 full-table scans (sales, purchases, lots, allocations, settlements) plus per-sale-today N+1 line fetch, all materialized in Rust. Linear in lifetime history. Should be: `SUM(...) WHERE transaction_date >= today` on indexed columns, `SUM(remaining_quantity*unit_cost)` over the partial active-lot index, `SUM(balance_due)` over a partial index `WHERE balance_due > 0`.
- `get_low_stock_alerts`: full scan + sort of items (+ BEGIN IMMEDIATE).
- `get_catalog`: runs the page query AND a second query with `limit usize::MAX` just to count (`item_service_impl.rs:36-43`), i.e. whole table materialized per page (FR-10 F10-05); `i64::try_from(usize::MAX)` saturates so it works but loads everything.
- `restore_lots_for_return` `fifo_strategy.rs:103-129`: to find ONE lot by id it calls `list_purchases(..., 0, 10000)` then `list_items_by_transaction_id` for each purchase (N+1, up to 10000 queries), then `list_synthetic`, then `list_all_lots`. See F-N01.

## 6. Connection and transaction config
- Pool `connection.rs:6-18`: r2d2 max_size 4, per-connection init: WAL, busy_timeout 5000, foreign_keys ON, synchronous NORMAL (matches LLD 5.3/10.2). Missing: `cache_size` (default 2 MB), `mmap_size`, `temp_store=MEMORY`, `wal_autocheckpoint`/periodic `PRAGMA optimize`, pool `connection_timeout` default 30s (a stalled reader blocks UI for 30s).
- `begin_transaction` always runs `BEGIN IMMEDIATE` (`sqlite_uow.rs:27`). There is no read-only path. Read-only services using it: report_service_impl (6 of 6 methods, lines 36,125,143,168,186,203), `get_catalog` and `get_bundle_suggestions` (item_service_impl), sales/purchase history and details (sale_service_impl 188/203, purchase_service_impl 126/141), `get_next_return_code`, `calculate_live_cash_drawer`, party reads, `shop_setting` get, return details. That is roughly 20 of ~45 call sites. Every read takes the single SQLite write lock, defeating WAL concurrency; the 5 parallel dashboard IPCs serialize behind each other and behind any checkout (busy_timeout 5 s).
- Tauri commands are plain `pub fn` (no `async`, no `spawn_blocking`; rg finds no `async fn` in `src-tauri/src/api`). In Tauri v2, sync commands run on the main thread; all SQLite work, including FTS and dashboard scans, executes on the UI/event-loop thread, so a long query freezes the window, and the 150 ms typeahead debounce is moot while a scan runs.

## 7. Frontend
- Dashboard `DashboardPage.tsx:31-51`: `useEffect` on mount, `Promise.all` of 5 IPCs (metrics, low-stock, trend, sales history, customers list), no SWR/stale-while-revalidate, no cache across route changes (state lives in the component), nothing renders from cache; no loading skeleton either (metrics default 0, which shows false zeros: `:53-56`). HLD 9.2 wants KPIs first, trend later; code awaits all five before ANY state update, so KPIs are gated on the slowest call (trend loads all sales+purchases).
- `getSalesHistory({limit: 6})` and `listParties` are whole-table paths (FR-10).
- Typeahead debounce 150 ms (`ItemSearchTypeahead.tsx:56-72`), min 2 chars; no request-sequence guard (stale response can overwrite newer: results from "ab" arriving after "abc"); barcode scans (fast keystroke bursts) still pay 150 ms.
- Header clock `setInterval(1000)` `Header.tsx:43-46` re-renders the header every second; harmless unless the header subtree is heavy, but it re-renders on every route; low-priority.
- Bundle: dependencies are minimal (react, zustand, lucide-react); no route-level `React.lazy`/`Suspense` (rg: none). `lucide-react` named imports are tree-shaken. No concern beyond missing lazy routes.

## 8. Findings
| ID | Sev | Kind | file:line | What's wrong | Failure scenario | Change needed | Overlaps |
|---|---|---|---|---|---|---|---|
| F-N01 | High | bug/perf | fifo_strategy.rs:103-129 | Customer-return lot lookup scans `list_purchases(limit 10000)`, then each purchase's lines, then synthetic, then ALL lots | Shop with 5k purchase bills: one return line = up to 5k queries inside BEGIN IMMEDIATE (seconds) holding the write lock; also lots beyond the first 10000 bills (or synthetic-header lots) are only found by the last fallback; purchase_repo needs `find_item_by_id(lot_id)` | Add `PurchaseRepository::find_lot_by_id` (PK lookup) | FR-03, FR-07 |
| F-N02 | High | design-gap | report_service_impl.rs:36-45,125,143,168,186,203; sqlite_uow.rs:27 | All reads use BEGIN IMMEDIATE (about 20 sites) | Dashboard's 5 parallel IPCs serialize; during a long import/checkout the dashboard waits up to 5 s then fails with `database is locked`, caught by `.catch(() => null)` and shown as zeros (silent) | Add `begin_read()` (`BEGIN DEFERRED` / no txn) for read services; keep IMMEDIATE for writes | FR-12/13 (seen), NFR-01 |
| F-N03 | High | design-gap | main.rs commands, report_controller.rs:5-8 | Sync `#[tauri::command]` fns run SQLite on the main thread | Any slow query (dashboard on big DB, FTS fallback LIKE over 1000 rows, import) freezes the whole window including typing | Make commands `async` with `tauri::async_runtime::spawn_blocking` (or run services on a worker) | NFR-01 |
| F-N04 | High | bug | inventory_service_impl.rs:105-140 | Vendor return decrements cache by qty but lot only if `purchase_item_id` is Some and clamps `.max(0)` | Return 5 with no lot id: cache 20 -> 15, lots unchanged (sum 20); later sale of 18 passes lot validation (sum 20) then cache = `max(15-18,0)=0`; invariant permanently broken, low-stock flag wrong | Use strategy outflow (DD-11) for all vendor returns; delta applied once | FR-08 F-05, FR-03 F-04 |
| F-N05 | Medium | perf | fts_indexer.rs:94-165 | Loads every match, then `query_row` per id plus bundle query per id (N+1), LIKE fallback `%q%` up to 1000 rows scanning items | 20k-item catalog, 2-char query: hundreds of rows x 2 queries on the main thread; sub-ms claim false | `LIMIT ?` in the FTS SQL, join items in one query, drop LIKE fallback or cap at limit | FR-14 |
| F-N06 | Medium | design-gap | report_service_impl.rs:125-139; item_repo.rs:80 | Low-stock has no supporting index; sort by NOCASE name | 10k items: full scan + sort on each dashboard open | Partial index `items(name COLLATE NOCASE, id) WHERE is_active=1 AND is_low_stock=1`; `idx_items_name` should be NOCASE | FR-05 |
| F-N07 | Medium | perf | sale_repo.rs:144, purchase_repo.rs:145, return_repo.rs:97 | `get_next_transaction_code` selects all codes with case-insensitive LIKE and parses in Rust | Every sale checkout O(total sales) inside the write txn; at 100k sales ~100 ms+ added to Enter-to-complete | `SELECT MAX(CAST(substr(code,5) AS INTEGER))` or a counter row (FR-12) | FR-12, FR-02 |
| F-N08 | Medium | design-gap | inventory_service_impl.rs:41-60 and 7 other sites | Cache updated by read-modify-write of stale `item.current_stock` and clamped; no `SUM(lots)` check/repair | Any single drift (F-N04, import lot swallowed at `import_export_service_impl.rs:262-272`) persists forever and spreads | Delta-in-SQL update; `verify_stock_cache` at startup + tests (section 3) | NFR-02 LLD 6.3, FR-03 |
| F-N09 | Medium | bug | import_export_service_impl.rs:262,272 | Lot creation errors swallowed (`if let Ok`, `let _ =`) while item row already has stock | Import item with stock 50: header insert fails silently; item shows 50, sale of 1 fails validation ("available 0") because lots are empty | Propagate the error / fail the row | FR-11 |
| F-N10 | Medium | design-gap | migrations.rs (index list) | Missing LLD 5.3 indexes: `purchase_transaction(source, date)`, `return_item(sale_item_id)`, `return_item(purchase_item_id)`, `stock_outflow_allocation(source_type, source_id)`; unused: idx_items_name (collation), idx_items_category | Return validation ("already returned qty for this sale line") scans return_items | Add in schema v2 with the DD-11/DD-25 columns | NFR-04, FR-07 |
| F-N11 | Medium | ux/perf | DashboardPage.tsx:31-56 | No SWR; five calls awaited together; zeros rendered as real data; errors swallowed into empty values | Open the dashboard on a slow DB: blank page then zeros; a transient lock error shows "Rs 0 sales" | `useDashboard` hook: cached last payload shown immediately, background revalidate, KPI call separate from trend, error state; single `get_dashboard_snapshot` (KPIs + low stock + 6 recent sales) per HLD 9.2 | FR-09, UI LLD 7.2 |
| F-N12 | Medium | test-gap | report_service_test.rs:20-53; search_indexer_test.rs:41; e2e_tier3:504 | Perf tests run on 1 sale/1 purchase/1 lot, thresholds 50 ms / 10 ms (not the 2 ms / 1 ms claims) | The O(n) dashboard passes with 1 row; fails (seconds) at 50k rows and nothing detects it | Seeded-volume bench (10k items, 50k sale lines, 5k purchases): dashboard < 20 ms, search p95 < 5 ms in CI harness | FR-09, FR-14 |
| F-N13 | Medium | design-gap | connection.rs:6-18 | No `cache_size`, `mmap_size`, `temp_store`, pool timeout default 30 s; max_size 4 shared by UI + indexer | Stalled write holds conns; reads queue up to 30 s with no message | `cache_size=-20000`, `mmap_size=268435456`, `temp_store=MEMORY`, `connection_timeout(3s)`, `PRAGMA optimize` on close | NFR-01 |
| F-N14 | Low | quality | ItemSearchTypeahead.tsx:56-72 | No stale-response guard; fixed 150 ms debounce also delays barcode scans | Type "abc"; response for "ab" arrives last and replaces the list | Request counter; skip debounce when input looks like a scan (fast burst/Enter) | FR-14, FR-15 |
| F-N15 | Low | quality | item_repo.rs:103-109 | `update()` writes `current_stock`/`is_low_stock` from the caller's struct | Future caller passing a stale Item overwrites live stock | Exclude cache columns from `update()` | NFR-03, LLD 6.3 |
| F-N16 | Low | quality | routing (App.tsx) | No `React.lazy` route splitting | Larger first paint on a low-end POS machine | Lazy-load Reports/Settings/Migration pages | none |
| F-N17 | Low | quality | Header.tsx:43-46 | 1 s `setInterval` re-render | Constant redraw of header | Isolate the clock in its own component | none |

Known items (not re-counted): FR-09 F-09 dashboard whole-table + N+1; FR-05 F5-01 low-stock cap 50/name order; FR-14 FTS full-load; FR-10 F10-05/06; FR-03 F-04 / FR-08 F-05 clamp and vendor-return drift; FR-05 F5-05 literal 5.0.

## 9. Test coverage
- Exists: dashboard latency (tiny data, 50 ms), FTS prefix latency (10 ms), barcode scan (10 ms), TC-INV-001..007 style cache asserts.
- Missing: TC-INV-008 stock cache invariant over a mixed sequence (no `SUM(remaining_quantity)` compare anywhere in `src-tauri/tests`); startup repair test; volume benchmark; read-does-not-block-write test (reader during open writer); typeahead stale-response test; dashboard SWR test (vitest).

## 10. Schema and contract delta
- Indexes: `items(name COLLATE NOCASE) WHERE is_active=1`; partial `items(...) WHERE is_low_stock=1`; `purchase_items(item_id, id) WHERE remaining_quantity>0`; `sale_transactions/purchase_transactions(balance_due) WHERE balance_due>0`; `purchase_transactions(source, transaction_date)`; `return_items(sale_item_id)`, `(purchase_item_id)`; `stock_outflow_allocation(source_type, source_id)`.
- Contracts: `begin_read()` on UnitOfWork; `PurchaseRepository::find_lot_by_id`; `InventoryService::verify_stock_cache()`; `update_stock_cache` by delta; one `get_dashboard_snapshot` IPC (KPIs + low-stock + recent sales); `ReportRepository` aggregate SQL methods.
