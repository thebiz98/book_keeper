# NFR-03 Review: Transaction Immutability & Audit Integrity

Scope: every write statement in `src-tauri/src` (rg for UPDATE/DELETE/INSERT/REPLACE/ON CONFLICT; no other raw SQL exists outside `data/sqlite/**`), IPC surface, mock client, UI affordances, DB-level protection, audit completeness, tests. Read-only review; no cargo run.

## 1. Verdict summary

Application-level immutability is largely upheld: there is no IPC command, repository method, service method, mock method or UI control that deletes or edits a committed sale, purchase, return, settlement or adjustment row. The weaknesses are (a) zero DB-level enforcement, (b) header cached-field updates that break the DD-01 invariant and leave no ledger row (returns with ReduceDue), (c) stock-cache/lot changes with no audit row on some paths, (d) synthetic, unaudited "purchases" created for stock, and (e) no tests for immutability.

Counts: Critical 0, High 3, Medium 6, Low 4.

## 2. Write-SQL table

| # | File:line | Statement | Table.columns | Caller chain | Verdict |
|---|---|---|---|---|---|
| 1 | sale_repo.rs:98 | INSERT | sale_transactions (all) | SaleService.create_sale -> create_sale | creation, OK |
| 2 | sale_repo.rs:110 | INSERT | sale_items | create_sale | creation, OK |
| 3 | sale_repo.rs:158 | UPDATE | sale_transactions paid_amount, balance_due, payment_status | PaymentService.record_payment (create_sale upfront, settle_bill_payment); PaymentService.reduce_transaction_balance (create_customer_return ReduceDue) | allowed cached fields, but VIOLATION of DD-01 for the return path (F-01) |
| 4 | purchase_repo.rs:99,111 | INSERT | purchase_transactions, purchase_items | create_purchase; also synthetic: item_service create_item (INIT-), inventory create_adjustment (ADJ-), import_export (IMP-), migration (PUR-OPEN-) | creation; synthetic ones are fake purchases (F-04) |
| 5 | purchase_repo.rs:170 | UPDATE | purchase_items remaining_quantity | fifo_strategy.rs:59,147,183; inventory_service_impl.rs:132 (vendor return) | allowed cached field; no audit row on vendor-return path (F-03) |
| 6 | purchase_repo.rs:181 | UPDATE | purchase_transactions paid_amount, balance_due, payment_status | record_payment, reduce_transaction_balance (create_vendor_return ReduceDue) | same as #3 |
| 7 | allocation_repo.rs:40 | INSERT | sale_stock_allocations | fifo deplete_lots_for_sale | creation, OK |
| 8 | adjustment_allocation_repo.rs:40 | INSERT | adjustment_stock_allocations | deplete_lots_for_adjustment | creation, OK |
| 9 | adjustment_repo.rs:36 | INSERT | stock_adjustments | create_stock_adjustment | creation, OK |
| 10 | settlement_repo.rs:44 | INSERT | payment_settlements | record_payment, record_refund | creation, OK (typed strings / negative amounts: FR-02 F-04) |
| 11 | return_repo.rs:56,73 | INSERT | return_transactions, return_items | create_customer_return / create_vendor_return | creation; total_amount written 0 and never updated for customer returns (FR-07 F-02) |
| 12 | item_repo.rs:93 | INSERT | items | create_item, import, migration | creation |
| 13 | item_repo.rs:105 | UPDATE | items name..updated_at incl. current_stock, is_low_stock, is_active, created_at | update_item, items import (existing match) | master edit; full-row write overwrites created_at and cached stock (F-08) |
| 14 | item_repo.rs:118 | UPDATE | items current_stock, is_low_stock | InventoryService (5 sites), create_adjustment | allowed cached field; clamped (FR-08 F-05) |
| 15 | item_repo.rs:125 | UPDATE | items is_active=0 | archive_item | soft delete, OK |
| 16 | party_repo.rs:59 | INSERT | parties | create_party, migration | creation |
| 17 | party_repo.rs:67 | UPDATE | parties all incl. created_at, party_type | update_party | master edit; party_type change can flip a customer with sales to vendor (F-09) |
| 18 | party_repo.rs:77 | UPDATE | parties is_active=0 | archive_party (blocked if dues) | soft delete, OK |
| 19 | bundle_repo.rs:68 | INSERT | item_bundles | create_bundle | creation |
| 20 | bundle_repo.rs:76 | UPDATE | item_bundles all | no caller (update_bundle not implemented, FR-13) | dead; bundle edit would also alter history display |
| 21 | bundle_repo.rs:87 | DELETE | item_bundles | no caller (delete_bundle not implemented) | dead; with bundle_id FKs it would fail (F-05) |
| 22 | setting_repo.rs:42 | INSERT ... ON CONFLICT DO UPDATE | shop_settings | update_shop_settings | config, OK (no history, Low F-12) |
| 23 | fts_indexer.rs:35,41,171,182,215,228 | INSERT/DELETE | item_search_fts | item service, indexer rebuild | derived index, OK |
| 24 | migrations.rs:12 | INSERT OR IGNORE | shop_settings | startup | OK |

No UPDATE or DELETE exists on sale_transactions (other than cached fields), sale_items, purchase_items (other than remaining_quantity), sale_stock_allocations, payment_settlements, return_*, stock_adjustments, adjustment_stock_allocations. Confirmed by rg.

## 3. Surface checks

- IPC commands (main.rs, api/*): no update_sale, delete_*, void or cancel. Mutating commands are create_*, settle_bill_payment, archive_item, archive_party, update_item, update_party, update_shop_settings, import/migration. OK. Mock client (mockClient.ts) likewise has none.
- UI: Trash2 icons are only in cart tables (CartTable.tsx:134, NewPurchasePage.tsx:340, NewSalePage.tsx:267), pre-commit. History pages have no edit/delete control. Edit exists only for parties (PartiesPage.tsx:189) and items.
- Backup: backup_service_impl.rs uses SQLite online Backup API to a new file, plus integrity_check; no restore path exists, so no bulk overwrite. migration_service execute_migration is additive-only (items, parties, opening stock) from a read-only source DB.
- PRAGMA foreign_keys=ON: set in `with_init` (connection.rs:11), so applies to every pooled connection. Backup/verify open their own plain connections (read-only use, acceptable).
- FK delete behaviour: every FK is default NO ACTION (no ON DELETE anywhere), so no cascades can wipe history. Parent rows with children cannot be deleted. Leaf tables (payment_settlements, allocations, stock_adjustments, return_items) have no protection.
- Missing FKs: payment_settlements.transaction_id (polymorphic, 0 used for unlinked refund, record_refund), return_transactions.original_transaction_id (polymorphic). Orphans are possible.
- No triggers, no CHECK constraints at all (migrations.rs). LLD §5.3 CHECKs (amount > 0, balance_due >= 0, remaining_quantity >= 0, quantity > 0, enums) are absent.
- Audit: created_at exists on all transaction tables, items, parties. Missing: sale_items, purchase_items, return_items, both allocation tables (inherit from parent; acceptable). No "who" (single user, acceptable). `created_at` is client-clock-free (server UTC): good.

## 4. Findings

| ID | Sev | Location | Finding | Failure scenario | Fix | Overlaps |
|---|---|---|---|---|---|---|
| F-01 | High | payment_service_impl.rs:104-145; sale_repo.rs:158; purchase_repo.rs:181 | ReduceDue returns call `update_balance_due`, whose SQL does `paid_amount = paid_amount + MAX(balance_due - new, 0)`. A return credit therefore inflates paid_amount and no ledger row (RETURN_CREDIT) is written. DD-01 invariants (`paid = Σ(UPFRONT+SETTLEMENT)`, ledger as source of truth) break and the money change is unauditable. | Sale 1000 unpaid, customer returns 300 with ReduceDue: header paid_amount=300, balance 700, ledger empty. Cash-flow reports show 0 in; any ledger-derived rebuild gives paid=0/balance=1000. | Split SQL: payment path adds paid; credit path only lowers balance; always insert RETURN_CREDIT row; single writer in PaymentService. | FR-06 F-12, FR-07, FR-02 F-04 |
| F-02 | High | migrations.rs (whole), 24-row table | No DB-level protection of committed history: no triggers, no CHECKs, leaf tables deletable with plain SQL; header money fields writable by any UPDATE. NFR-03 relies purely on absence of code paths. | A future repo method or a SQLite tool run `DELETE FROM payment_settlements` or `UPDATE sale_items SET total_amount` succeeds silently; balances diverge from ledger. | Migration v2: BEFORE UPDATE/DELETE triggers raising ABORT on sale_items, purchase_items (except remaining_quantity), allocations, settlements, return_*, stock_adjustments, and on sale/purchase headers except cached columns; add CHECKs per LLD §5.3. | FR-01, FR-02 |
| F-03 | High | inventory_service_impl.rs:110-139; fifo_strategy.rs:93-150 | Stock changes with no audit row: (a) vendor return decrements lot remaining_quantity (clamped `.max(0.0)`) while cache drops the full qty, with no allocation row; if `purchase_item_id` is None or the lot is no longer "active" the lot is silently skipped and only the cache changes; (b) customer-return restoration raises remaining_quantity with no reverse-allocation/returned marker, and repeating a return on the same sale line is only capped by lot capacity, not by prior returns, so it can restore twice after the lot was re-depleted. | Buy 10 lot, sell 4 (alloc 4), return 4 (rem 10), sell 4 again (rem 6), return the original line 4 again: rem 10, cache 10 although only 4 net were sold twice and returned twice; returned_quantity per line is not tracked. Vendor return of 5 with lot rem 2: lot 0, cache -5, lots sum 0 vs cache. | Track returned_quantity on sale_items/purchase_items (HLD §9.4), cap returns, write allocation/movement rows for vendor returns, error instead of skip. | FR-07, FR-08 F-05 |
| F-04 | Medium | item_service_impl.rs:90-125; inventory_service_impl.rs:166-200; import_export_service_impl.rs:240-275; migration_service_impl.rs:196-226 | Stock is created via synthetic "purchase" headers (INIT-, ADJ-, IMP-, PUR-OPEN-) that are inserted PAID with paid_amount set and no ledger row, with invented cost (`default_sale_price*0.7`). They appear in purchase history and COGS. Migration header has total 0 while lines carry cost, status Paid with paid 0. | Positive adjustment of 10 shows as a 700 "Paid" purchase with no PAYMENT_SETTLEMENT; cash-out ledger and purchase totals disagree; P&L cost basis invented. | Flag lots with `source` (DD-07) and exclude from purchase history/cash; no header fabrication or header totals equal to line sums. | FR-06, FR-08 |
| F-05 | Medium | migrations.rs:76,106; bundle_repo.rs:87 | `bundle_id` FK on sale_items/purchase_items (NO ACTION). Once delete_bundle is implemented per DD-28 ("always allowed") the delete fails with an FK error for any bundle used on a bill; dropping the FK naively via ON DELETE CASCADE would wipe lines, so the DD-28 snapshot (pack_label/pack_quantity, no FK) must land first. Bundle edit (update SQL exists) would also rewrite displayed history because details join the live bundle. | Delete "Box of 10" used by a sale: constraint failed. Edit multiplier to 12: old sale detail shows 12. | Implement DD-28 schema change (FR-01 F-13, FR-02 F-18); never use cascade. | FR-01 F-13, FR-02 F-18, FR-13 |
| F-06 | Medium | payment_service_impl.rs:20-75 | record_payment does not verify `payload.party_id` matches the bill's party or that transaction_type exists consistently; settlement row stores caller-supplied party. | Settlement against sale 5 (customer A) recorded with party B: ledger attributed to B, A's header cleared. | Derive party from the bill; reject mismatch. | FR-06 |
| F-07 | Medium | settlement_repo.rs:44; payment_service_impl.rs:90-101 | Refunds stored as negative amount_paid with `transaction_type = "SALE_REFUND"` and `transaction_id = 0` for unlinked refunds; no FK. Violates DD-01 (`amount_paid > 0`, kind, nullable id) and lets orphan references exist. | `transaction_id=0` row is indistinguishable from a real link; sums by sign rather than kind. | Add kind, return_id, nullable id, CHECK amount_paid > 0. | FR-02 F-04 |
| F-08 | Medium | item_repo.rs:105; party_repo.rs:67 | UPDATE rewrites created_at and (items) current_stock/is_low_stock/is_active from an in-memory copy. Safe today because loaded inside the same BEGIN IMMEDIATE, but the repository contract permits overwriting creation timestamp and stock cache from any caller; import path reuses it. | A caller passing a stale Item struct resets stock to an old value with no audit row. | Narrow UPDATE column list; stock only via update_stock_cache. | FR-08 |
| F-09 | Medium | party_service_impl.rs:60-68 | update_party allows party_type change with no check for existing transactions. | Customer with 20 sales set to Vendor: dues views and ledger filter by type, history is orphaned from the customer list. | Block type change when transactions exist. | FR-09 |
| F-10 | Low | migration_service_impl.rs:196 | Migration is not idempotent: second run duplicates all items and opening lots (code per-second `PUR-OPEN-`), doubling stock; no dedupe and no record of the import source. | Operator clicks import twice; stock doubled with two opening lots and no way to reverse (no delete allowed). | Dedupe by name/barcode or record import fingerprint. | FR-14 |
| F-11 | Low | tests (src-tauri/tests) | No test asserts immutability: only e2e_tier3 `test_tc_t3_cmb_010` (price change leaves old sale total). No test that DELETE/UPDATE of committed rows fails, that foreign_keys is ON for pooled connections, or that invariants (paid = Σ ledger) hold after returns. | F-01 passes the suite. | Add LLD §11 tests plus trigger tests. | |
| F-12 | Low | setting_repo.rs:42 | Shop name/address overwritten in place with no history; printed bills reprint with current shop details. | Rename shop; old receipts reprinted show new name. | Snapshot on bill or accept as documented. | FR-12 F-07 |
| F-13 | Low | mockClient.ts:637-675 | Mock returns write no return record, no ledger row, no party balance change for ReduceDue; mock stock changes leave no audit rows; getReturnDetails is a stub. Parity with DD-27 is not met for audit. | UI tested against mock shows no history of a return that the backend would record. | Mirror backend records in mock. | FR-07 |

## 5. Audit trail completeness

Money changes with a ledger row: upfront payment (via record_payment), settlement, cash refund (typed). Money changes without one: ReduceDue return credit on both sale and purchase headers (F-01); synthetic Paid purchase headers (F-04).
Stock changes with an audit row: purchase (lot), sale (allocations), adjustment negative (adjustment allocations), adjustment positive (synthetic lot), customer return (return_items, but no restoration mapping, F-03). Without: vendor-return lot decrement and cache-only decrement; migration/import lots exist but are fabricated.
Cached values `current_stock`, `is_low_stock` are updated by incremental arithmetic, never recomputed from lots, so any missed audit row is permanent drift.

## 6. Cross-requirement issues

- FR-06: PaymentService is not the sole money writer (F-01), unaudited synthetic Paid headers.
- FR-07: return total/returned_quantity tracking and re-return cap (F-03).
- FR-01/FR-02/FR-13: DD-28 schema drift blocks safe bundle delete (F-05).
- FR-08: stock cache clamp and lot/cache divergence.
- FR-14: migration non-idempotent (F-10).
