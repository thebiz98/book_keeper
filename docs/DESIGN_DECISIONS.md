# Design Decisions Record (v1 alignment, 2026-11-21)

This record is authoritative wherever the HLD, LLD, UI LLD or UI/UX spec disagree. All five docs have been updated to match it. `D-xx` refers to `docs/reviews/00_DOC_ALIGNMENT.md`.

## Money

**DD-01 — Payment ledger is the single source of truth (D-01).**
- Every money event is one `PAYMENT_SETTLEMENT` row:
  - `kind` ∈ `UPFRONT | SETTLEMENT | RETURN_CREDIT | REFUND_OUT | REFUND_IN`
  - `transaction_type` ∈ `SALE | PURCHASE`
  - `transaction_id` is nullable (a walk-in or unlinked refund has no bill)
  - `return_id` is nullable (FK to `RETURN_TRANSACTION`)
  - `amount_paid` is always > 0
- Sale and purchase headers are inserted with `paid_amount = 0` and `balance_due = total_amount`. Upfront cash is then applied through the one `PaymentService.record_payment` path as a `UPFRONT` row. No other code writes `paid_amount` or `balance_due`.
- Header invariants (the header holds cached values):
  - `paid_amount = Σ(UPFRONT + SETTLEMENT)`
  - `balance_due = total_amount − paid_amount − Σ(RETURN_CREDIT)`
  - `payment_status`: `PAID` if `balance_due = 0`; else `PARTIAL` if `paid_amount + credits > 0`; else `UNPAID`
- Cash direction is derived from `(transaction_type, kind)`:
  - **Cash in:** SALE·UPFRONT, SALE·SETTLEMENT, PURCHASE·REFUND_IN
  - **Cash out:** PURCHASE·UPFRONT, PURCHASE·SETTLEMENT, SALE·REFUND_OUT
  - **Non-cash:** RETURN_CREDIT
- Cash flow and the live cash drawer are sums over the ledger by `settlement_date`.

**DD-02 — Payment status is cross-checked (D-07, D-08).** The payload keeps `payment_status`, and the backend rejects any contradiction with `paid_amount`:
- `PAID`: sale needs `paid ≥ total`; the excess is change and is not stored, so the stored upfront amount is `total`. Purchase needs `paid = total`.
- `PARTIAL`: `0 < paid < total`.
- `UNPAID`: `paid = 0`.
- A purchase with `paid > total` is always rejected.
- `party_id` is required whenever the status is not `PAID`.

**DD-03 — Return settlement auto-applies to dues first, on both sides (D-02).**
- `settlement_type` ∈ `REFUND | REPLACEMENT` (customer) and `REFUND` (vendor).
- A `REFUND` return's value is used in this order:
  1. It reduces the linked bill's `balance_due` as a `RETURN_CREDIT` row, capped at that balance.
  2. Any remainder is cash: `REFUND_OUT` for customers, `REFUND_IN` for vendors.
- **A linked (tracked) return only ever touches its own bill.** Leftover value is refunded in cash; it is never moved to the party's other bills.
- **Unlinked (untracked) returns only:** with a party selected, the value is applied as `RETURN_CREDIT` across that party's oldest open bills (the same FIFO ordering used for dues), and the remainder is cash. A walk-in return is all cash.

**DD-04 — Settlement entry points (D-15).** Both modes exist:
- **Per-bill "Settle":** on every open bill. Available in Sales History, Purchase History, and the party's bill list on the Credit & Dues page. `transaction_id` is set and the amount must be ≤ that bill's balance.
- **Lump sum ("Receive on account" / "Pay on account"):** on the Credit & Dues page. It clears the party's oldest open bills first and shows a preview of the split. `transaction_id` is null and the amount must be ≤ the party's total outstanding.

## Stock & returns

**DD-05 — Return guards (D-04, D-05).**
- **Linked customer return line** (`sale_item_id` set):
  - the item must match the sale line;
  - `qty ≤ sold − Σ already returned`;
  - `unit_price ≤` the line's effective base price, and defaults to it.
- **Unlinked customer return** (no receipt) is allowed:
  - `RESTOCKABLE` creates a `RETURN`-source lot at the latest lot cost; if the item has never had a lot, the user enters the cost;
  - `DAMAGED_SCRAP` makes no stock change and books no loss.
- **Vendor return line:**
  - with `purchase_item_id` set: `qty ≤` that lot's `remaining_quantity`;
  - without it: FIFO across the item's lots, limited to available stock.
  - In both cases, outflow allocations are logged.
- `SALE_STOCK_ALLOCATION.returned_quantity` tracks returned units for each allocation. Reversal walks allocations in reverse id order and uses `depleted − returned` as each one's capacity, so repeated partial returns can never over-restore.
  - `RESTOCKABLE` lines also add the quantity back to `PURCHASE_ITEM.remaining_quantity`.
  - `DAMAGED_SCRAP` lines only increment `returned_quantity`.
  - The FIFO cost of the reversed units is stored as `RETURN_ITEM.cost_amount`.

**DD-06 — Replacement / exchange is a no-money swap of any item (D-03; revised 2026-11-21).**
- Covers defective swaps (same item) and variant swaps such as colour or size (a different item). `RETURN_ITEM.line_type` ∈ `RETURNED | REPLACEMENT_ISSUED` tells the two kinds of line apart.
- A `REPLACEMENT` return has:
  - one or more **returned** lines, linked to the original bill, each `RESTOCKABLE` (resellable, e.g. a colour swap) or `DAMAGED_SCRAP` (defective);
  - one or more **issued** lines: any active items, any quantities, stock-checked.
- **No money moves and revenue is not reversed.** Price differences are not settled. If the values differ materially, the cashier does a `REFUND` return plus a new sale instead. The UI shows the value difference (at default prices) as a warning only.
- **Returned lines are reversed exactly as in a `REFUND` return** (DD-05):
  - `returned_quantity` is incremented on the sale allocations;
  - `RESTOCKABLE` units go back to their original lots at their original cost;
  - `cost_amount` stores the reversed cost.
- **Issued lines** are depleted FIFO, with allocations logged in `STOCK_OUTFLOW_ALLOCATION (source_type = REPLACEMENT, source_id = return_id)`. Their cost becomes **replacement COGS** on the return date.
- Net effect:
  - **Colour swap:** the old unit's cost comes out of COGS and the new unit's cost goes in. The total cost of goods equals one unit.
  - **Defective swap:** the old unit's cost moves to inventory losses (scrap) and the new unit's cost goes into COGS.

**DD-07 — Lot source (D-06).**
- `PURCHASE_TRANSACTION.source` ∈ `PURCHASE | ADJUSTMENT | OPENING | RETURN`.
- Each source has its own code sequence: `PUR-`, `ADJ-`, `OPN-`, `RTL-`.
- Non-`PURCHASE` headers have `total_amount = 0`, no party, and status `PAID`.
- These views filter to `source = PURCHASE`: purchase history, purchase counts and totals, vendor payables, vendor-return lot pickers, and the dashboard.
- The lot inspector shows every source.

**DD-08 — Positive adjustment cost (D-13).** The lot uses the `unit_cost` of the item's most recent lot, from any source. If the item has never had a lot, `StockAdjustmentPayload.unit_cost` is required and the UI prompts for it. The sale price is never used.

**DD-09 — FIFO order is entry order (D-10).** "Oldest" means the lowest `PURCHASE_ITEM.id`. This is deterministic and never re-costs past sales. Back-dated entries are treated as newest.

**DD-10 — Stock validation adds up quantities per item across all cart lines (D-11),** in both the UI and the backend. Quantities are compared with a tolerance of 1e-9.

**DD-11 — One outflow allocation table.** `ADJUSTMENT_STOCK_ALLOCATION` becomes `STOCK_OUTFLOW_ALLOCATION (source_type ∈ ADJUSTMENT | REPLACEMENT | VENDOR_RETURN, source_id, purchase_item_id, quantity_depleted, unit_cost)`. Sale depletion stays in `SALE_STOCK_ALLOCATION`.

**DD-12 — Stock adjustment payload.** Adds `adjustment_date` and an optional `unit_cost` (see DD-08). The reasons are `DAMAGED | EXPIRED | THEFT_LOST | COUNT_CORRECTION` (v1.1, DD-40: + `UNRECORDED_STOCK`).

## Reporting

**DD-13 — P&L statement (D-25).** The same formula is used for the dashboard's "today" figures and for the P&L report.
- **Gross Sales** = Σ sale `total_amount` in the period.
- **Sales Returns** = Σ `REFUND`-type customer return totals, by return date.
- **Net Sales** = Gross Sales − Sales Returns.
- **COGS** = Σ (quantity_depleted − returned_quantity) × unit_cost over allocations of sales in the period, + Σ `REPLACEMENT` outflow cost (by return date).
- **Gross Profit** = Net Sales − COGS.
- **Inventory Losses** = negative-adjustment outflow cost (by adjustment date) + Σ `RETURN_ITEM.cost_amount` for customer `RETURNED` lines with `condition = DAMAGED_SCRAP`, in both `REFUND` and `REPLACEMENT` returns (by return date).
- **Net Profit** = Gross Profit − Inventory Losses.
- **Purchases report** = Σ `PURCHASE`-source totals − Σ vendor return totals.

**DD-14 — Trend periods (D-14).** The API takes `7d | 30d | 1y`. The UI toggle shows `7D / 30D / 1Y`. Reports take a from/to range and have quick presets for Today, Week, Month and Year.

**DD-15 — Rounding.** Line totals and bill totals are rounded to 2 decimals. Every money comparison uses those rounded values.

## Scope

**DD-16 — FR-11 v1 scope (D-17).**
- **In v1:**
  - Items import (CSV and XLSX): a dry-run validation report, then execution with a `SKIP`, `OVERWRITE` or `ADD_STOCK` strategy. Opening stock creates `OPENING`-source lots at the row's Purchase Cost.
  - One-click backup plus an integrity check.
  - **Amended 2026-11-22:** an *inventory-only* VedBill migration (item master + open stock lots from a `.vb` backup) is pulled into v1, sharing the items-import dry-run, strategy and summary. Rules are in LLD §6.14a.
- **Deferred to v2:** every export, parties import, opening dues, VedBill sales/purchases/parties history, and restore from backup.
- In the fix phase, the existing code for the deferred features is **removed**: IPC commands, services, UI buttons and tests.

**DD-17 — FR-17 archiving is deferred to v2 (D-22).** In v1 there is no archive and no delete for items or parties. The `is_active` columns stay (default 1) and read queries still filter on them. `archive_item` and `archive_party` are removed in the fix phase.

## UI & platform

**DD-18 — Checkout keyboard flow (D-20, D-21).**
1. Search → Qty (default 1) → Unit (base unit, or last used; switch with the arrow keys; `Alt+B` adds a new bundle) → Rate (prefilled) → Enter commits the line, and focus returns to Search.
2. Enter on an empty Search, or `Ctrl+Enter`, opens the **Payment dialog before commit**. It shows:
   - the Grand Total;
   - the status (default `PAID`; `PARTIAL` or `UNPAID` require a customer);
   - Cash Received (auto-focused);
   - live Change.
3. Enter commits:
   - **PAID:** `paid_amount` = cash received, which must be ≥ total.
   - **PARTIAL:** `paid_amount` = cash received, which must be < total.
   - **UNPAID:** `paid_amount` = 0.
4. The dialog then shows the result: code, balance due, and change.
5. Enter resets to a blank bill. Esc in the payment dialog goes back to the cart without saving.

The footer payment-status inputs are removed. Notes stay in the footer.

**DD-19 — Hotkeys (D-24).**

| Key | Action |
|---|---|
| F1 | Dashboard |
| F2 | Sale |
| F3 | Purchase |
| F4 / Ctrl+F | Item lookup |
| F5 | Credit & Dues |
| F6 | Inventory |
| Alt+B | New bundle |
| Alt+C | New customer |
| Alt+V | New vendor |
| Ctrl+B | Backup |
| Ctrl+Enter | Complete bill |
| Esc | Close / back |

**DD-20 — DB location (D-23).** The database file is `shop.db` in the app data dir. The `BOOK_KEEPER_DB_PATH` environment variable overrides the path.

**DD-21 — IPC additions (D-16, D-19).**
- New commands:
  - `get_item_lots(item_id)`
  - `get_recent_transactions(limit)`
  - `list_returns(filter)`
  - `update_bundle`
  - `delete_bundle` (always allowed; see DD-28)
- `get_party_ledger` is exposed in the UI.
- The sales and purchase history row DTOs carry the party name, the line count and the date.
- `SalesHistoryFilter.search_query` matches the code or the party name.

**DD-22 — Atomicity.** Every mutating command runs in `BEGIN IMMEDIATE`. That includes vendor returns, items import, and item and bundle creation.

**DD-23 — Enums unified (D-09).**
- `return_type`: `CUSTOMER | VENDOR`
- `condition`: `RESTOCKABLE | DAMAGED_SCRAP`
- `settlement_type`: per DD-03
- `kind`: per DD-01
- `source`: per DD-07
- `source_type`: per DD-11

**DD-24 — FTS sync (D-18).** The search index is updated in application code, after commit, by the item service. No SQLite triggers are used.

**DD-25 — Supplier invoice reference.** `PURCHASE_TRANSACTION.supplier_invoice_ref` is its own column. It is no longer folded into `notes`.

**DD-26 — Error contract.** `invokeCmd` rejects with the typed `AppError` `{ type, details }` defined in LLD §10.1. The UI shows `details.message`.

**DD-27 — Mock is a thin fixture client (revised 2026-11-21).** The browser-mode mock serves seed data so UI layout and flows can be worked on without Rust. It does **not** re-implement business rules: no FIFO, ledger, guards or validation. All behaviour is verified against the real Rust backend. The mock must still match the typed IPC contract (method names, camelCase shapes, `AppError` shape for errors it chooses to simulate).

**DD-29 — Schema strategy: rewrite v1 in place (2026-11-21).**
- No user database exists yet, so `SCHEMA_V1` is replaced by the aligned schema (HLD §6, LLD §5.3).
- A `schema_version` table and a numbered, forward-only migration runner are introduced:
  - each step runs in its own transaction;
  - foreign keys are turned off during table rebuilds;
  - a backup is taken before migrating (implemented with migration v3): when the database already has applied migrations and at least one is pending, `run_migrations` runs `VACUUM INTO '<db dir>/backups/book_keeper_premigration_v{from}_to_v{to}_YYYYMMDD_HHMM.db'` on the same connection before applying any step (`_2`, `_3` on a name clash). If the snapshot fails, a `MigrationError` is returned and nothing is applied. A fresh database, an already-current database and in-memory databases create no backup. Rotation (DD-33) deletes `book_keeper_premigration_` files after `retentionDays` like pre-reset files;
  - the app refuses to open a database newer than it understands.
- A legacy development database (pre-alignment v1) is detected and rejected with a clear message.
- Every schema change after this one is a real v2+ migration, covered by a frozen-fixture upgrade test.

**DD-30 — Name snapshots on bills (2026-11-21).** Names are stored at write time, the same way as `pack_label`, so later edits to the directory or catalog never change historical bills:
- `party_name` on the sale, purchase and return headers (null for walk-ins);
- `item_name` on `SALE_ITEM`, `PURCHASE_ITEM` and `RETURN_ITEM`.

Bill views read the snapshot. Directory and catalog screens read the live record.

**DD-28 — Bills are decoupled from bundles (2026-11-21).** A bundle is only a data-entry convenience.
- `SALE_ITEM` and `PURCHASE_ITEM` store:
  - `quantity` (base units);
  - `unit_price` / `unit_cost` per base unit;
  - `total_amount`;
  - a display-only snapshot: `pack_label` (e.g. "Box of 10", null when entered in base units) and `pack_quantity` (e.g. 2).
- There is **no `bundle_id` column and no FK** from bill lines to `ITEM_BUNDLE`.
- Payloads may still send `bundle_id`. The backend uses it only at write time, to resolve the multiplier and label (the bundle must belong to the item), then snapshots both.
- Bundles can be edited, including the multiplier, or deleted at any time; history is unaffected.
- FIFO, returns and reports work purely in base units.

**DD-31 — Compiled-in app config (2026-11-22).** `src-tauri/app.config.json` (`testingMode`, `backup.intervalHours`, `backup.retentionDays`, `backup.onExit`) is embedded at build time, not read from disk or environment variables at run time.
- A shop owner cannot flip `testingMode` (which enables destructive resets) by editing a file or setting an env var; it takes a rebuild.
- One build behaves the same on every machine; there is no config file to lose or corrupt.
- Cost: changing a value needs a rebuild or a `tauri dev` restart. A real shop build sets `testingMode` to false.

**DD-32 — Backup folder: stored in the database, chosen with a native picker (2026-11-22).**
- The folder path is a setting in the database (schema v2), so it follows the data. Empty means the default, `%APPDATA%\com.bookkeeper.app\backups`, which is created automatically.
- The UI has no typed-path field. It uses the Tauri dialog plugin's native folder picker, which needs `capabilities/default.json` to grant `core:default` and `dialog:allow-open`.
- `set_backup_directory(null)` resets to the default.

**DD-33 — Auto backup and rotation (2026-11-22).**
- Triggers: at startup when the last auto backup is at least `intervalHours` old, then an hourly check; on close when `onExit` is true and the database changed since the last auto backup.
- File names carry a minute stamp: `book_keeper_backup_` (manual), `book_keeper_auto_`, `book_keeper_prereset_` + `YYYYMMDD_HHMM.db`.
- "Database changed" on close is detected by comparing the database file's mtime with the last auto backup, with no change counter to maintain.
- Rotation deletes auto and pre-reset files older than `retentionDays`, always keeps the newest auto backup, and never deletes manual backups.

**DD-34 — Testing-only data reset (2026-11-22).**
- Two scopes: `Transactions` (sales, purchases except opening stock lots, payments, returns, stock adjustments; stock returns to opening stock; items, bundles, parties and shop profile stay; invoice codes restart) and `All` (also items, bundles, parties and opening stock; only the shop profile and the backup folder setting remain; ids restart).
- The backend is the gate: `reset_data` returns `OperationNotAllowed` when `testingMode` is off. The UI hides the card, but that is only convenience. The UI also requires typing `RESET` or `DELETE ALL`.
- A pre-reset backup (`book_keeper_prereset_...`) is mandatory and written before anything is deleted; if it fails, the reset does not run.
- `MaintenanceRepository` does the bulk deletes child tables first, inside one transaction, so a failure leaves the data untouched.
- The stock cache is repaired from the restored OPENING lots, so on-hand quantities match the opening stock again.
- The FTS search index is rebuilt after the commit (the indexer uses its own connection, see CHECKPOINT conventions).

## v1.1 feature set (2026-11-22)

Full requirements are in the HLD (FR-05 rev., FR-08 rev., FR-14 rev., FR-18 – FR-22, NFR-03 rev., §10 delivery plan). The decisions below record the choices made between alternatives.

**DD-35 — Expenses are their own entity with a searchable type master (FR-18).** `EXPENSE_TYPE` (unique name, FTS-indexed, created inline from the type field) and `EXPENSE` (date, type, amount, notes, type-name snapshot, `EXP-` code). Expenses are always cash, paid on their date; credit expenses are out of scope.

**DD-36 — Every search field is live and index-backed (FR-14 rev.).** Debounced search as you type, no Search button. FTS5 per master entity (items, parties, expense types, charge types) with the DD-24 sync rule; paginated history lists may use indexed `LIKE`. One shared frontend hook/component.

**DD-37 — Expenses join cash flow outside the ledger (amends DD-01).** DD-01 still governs every *bill* money event. An `EXPENSE` row is itself a cash-out event and writes no `PAYMENT_SETTLEMENT` row. Cash out = ledger cash out + Σ expenses by date; the live cash drawer is today's cash in − today's cash out.

**DD-38 — Bill charges and discounts are bill-level lines from a reusable type master (FR-19).** `CHARGE_TYPE` (name, `effect` ∈ `CHARGE | DISCOUNT`, `applies_to` ∈ `SALE | PURCHASE | BOTH`, optional default amount) and `BILL_CHARGE` (polymorphic `transaction_type` + `transaction_id`, name and effect snapshots, amount > 0). Headers cache `items_subtotal`, `charges_total`, `discounts_total`; `total_amount = subtotal + charges − discounts ≥ 0`. Fixed amounts only.
  - v7 decisions: no seed charge types; several lines of one type allowed (max 50); duplicate type name is ValidationError(name); dashboard/trend use items_subtotal; refund cap applies to every linked return line via net_unit_price.
- **Chosen (user, 2026-11-22):** purchase charges are **Operating Expenses** and purchase discounts are **Other Income**; lot costs are not changed. *Rejected:* landed-cost allocation into lot `unit_cost`.

**DD-39 — P&L v1.1 (amends DD-13).** Gross Sales = Σ sale `items_subtotal`. Net Sales = Gross − Sales Discounts − Sales Returns. Gross Profit = Net Sales − COGS. Net Profit = Gross Profit + Other Income (sale charges + purchase discounts) − Inventory Losses − Operating Expenses (expenses + purchase charges). Purchases report = Σ purchase `items_subtotal` − vendor returns. Linked-return refund cap on a discounted sale = line rate × (1 − discounts_total / items_subtotal).
- **Chosen (user, 2026-11-22):** expenses reduce Net Profit, not just cash.

**DD-40 — Unrecorded stock entry (FR-08 rev.).** New adjustment reason `UNRECORDED_STOCK` (positive only) creates an `ADJUSTMENT`-source lot at a user-entered cost (prefilled from the latest lot; amends DD-08 for this reason only). "Set stock on hand" computes the delta: positive → `UNRECORDED_STOCK`, negative → `COUNT_CORRECTION`. Never a purchase, never income. Add mode = `create_stock_adjustment(UNRECORDED_STOCK)`; Set mode = `set_item_stock`, which computes the delta server-side. A `Transactions` testing reset (DD-34) removes unrecorded-stock lots like any adjustment. Schema v5 rebuilds `stock_adjustments`.

**DD-41 — Bills are correctable (FR-20; supersedes v1 NFR-03 immutability).** Sales and purchases can be edited (reverse-then-repost in one transaction, same id/code) or deleted; returns can be deleted; expenses edited/deleted. No revision history in v1.1 (`updated_at` only).
- **Chosen (user, 2026-11-22):** *guard, and allow return delete.* A bill with linked returns is locked until the returns are deleted. A purchase lot cannot shrink below its consumed quantity, and a consumed line cannot change item or be removed; a cost change re-costs dependent allocations. A bill's new total cannot fall below its later settlements + return credits. *Rejected:* cascading re-allocation; immutable returns.

**DD-42 — Item type (FR-21).** `ITEM.item_type` ∈ `PHYSICAL | SERVICE`, default `PHYSICAL`. Entity only in v1.1; services reject `SERVICE` until service billing is designed.
- Added by migration v3. The UI shows a read-only type line and never sends `itemType` (the backend defaults it).
- Later service billing must bypass the `cartStore.addItem` rule that rejects `currentStock <= 0` (out of scope now).

**DD-43 — Sticky table headers (FR-10 rev.).** Implemented once in the shared `Table`/`DataTable` primitives (scroll container + `position: sticky` header), so every list gets it.

**DD-44 — Visible Save/Cancel on quick-create forms (FR-22).** Enter/Escape remain, but every quick-create form also shows buttons.

**DD-45 — Per-item low stock level (FR-05 rev.; amends HLD Assumption 6).** `ITEM.low_stock_threshold` (default 5, ≥ 0, 0 = never low); `is_low_stock = PHYSICAL AND current_stock < threshold`. Editable inline in the Item List (replaces the Barcode column) and in the item drawer. `constants::LOW_STOCK_THRESHOLD` becomes only the default for new items.

**DD-46 — Delivery order and migration numbers.** See HLD §10. Branches merge to `main` in that order; migrations v3 – v8 are pre-assigned per branch.


**DD-47 - Expense type create is idempotent; expense codes and reset scope.** `create_expense_type` returns the existing row on a case-insensitive name match (inline create races with search; there is no 409 in `AppError`). Rename (`update_expense_type`) clashing with another type is a `ValidationError` on `name`; `list_expense_types` feeds the list filter. Codes never reuse (migration v6 `code_sequences`, all prefixes). `Transactions` reset deletes `expenses` and restarts `EXP-`; `All` also deletes `expense_types`; the FTS index is rebuilt after `All`.


**DD-48 — Bill correction mechanics (FR-20 / DD-41; schema v8).** How edit and delete stay consistent. All of it runs in one `BEGIN IMMEDIATE` transaction per command; failure rolls everything back.
- **Referenced vs consumed.** A lot is *referenced* when any `sale_stock_allocation`, `stock_outflow_allocation` or `return_items.purchase_item_id` row points at it, even when nothing is consumed (every sold unit was returned restockable). Changing the item, removing the line or deleting the purchase needs an unreferenced lot. The quantity floor is `consumed = quantity - remaining_quantity`.
- **Vendor-return cost lock.** A vendor return's value, `cost_amount` and cash/credit settlement come from the lot cost. A cost change on a lot with a `VENDOR_RETURN` outflow is rejected (`VENDOR_RETURNED_COST_CHANGE`); delete the vendor return first.
- **Return-credit lock.** An unlinked return applies `RETURN_CREDIT` rows to the party's open bills without being "linked" to them. Those bills cannot be deleted or change party, and the credits count toward the total floor (`TOTAL_BELOW_APPLIED`).
- **Linked-return lookups filter `return_type`.** `original_transaction_id` is polymorphic with no FK, so a sale id and a purchase id can collide.
- **Newest-first return delete.** The customer-return reversal walk (latest allocation first, capacity `depleted - returned`) is deterministic, so it is *replayed* from the sale line's `RETURNED` items in id order instead of storing a per-return table. The replay is valid only while allocations are immutable and returns on the bill are deleted newest first; both are guarded (`LINKED_RETURNS`, `NEWER_RETURN`). Used to undo a return and to recompute `RETURN_ITEM.cost_amount` after a re-cost. `REPLACEMENT_ISSUED.cost_amount` is recomputed from its outflow allocations in id order.
- **`RETURN` lot link.** An unlinked restockable customer line records its `RETURN` lot in `return_items.purchase_item_id` (v1.1). Older rows fall back to the `RTL-` header notes `Return RET-xxxx`, claimed in line order by lowest lot id.
- **In-place purchase lot edits.** Allocations point at lot ids, so a purchase edit compares old and new lots: kept lines update the lot (id kept), removed lines are deleted if unreferenced, new lines create lots. Header, charges and UPFRONT ledger are reversed and reposted. A sale edit is a full reverse-then-repost because an unlocked sale line has no references.
- **Edit payment semantics.** Status and paid describe the new upfront payment against `open = new_total - applied` (HLD FR-20). `applied` is `Σ SETTLEMENT + Σ RETURN_CREDIT`.
- **Codes.** v6 `code_sequences` already retires codes on issue, so v8 adds no sequence table and no `retire_code`.
- **`BillLocked { code, message }`.** New `AppError` variant. Reason codes: `LINKED_RETURNS`, `RETURN_CREDITS`, `NOT_A_PURCHASE`, `CONSUMED_LOTS`, `TOTAL_BELOW_APPLIED`, `PARTY_CHANGE_WITH_CREDITS`, `PARTY_REQUIRED_WITH_PAYMENTS`, `LOT_BELOW_CONSUMED`, `CONSUMED_LINE_CHANGED`, `CONSUMED_LINE_REMOVED`, `VENDOR_RETURNED_COST_CHANGE`, `NEWER_RETURN`, `RESTOCKED_CONSUMED`, `RETURN_LOT_USED`.
- *Rejected:* a per-return reversal table (old data lacks it); cascading re-allocation. Lots created later at "latest lot cost" keep their cost after a purchase re-cost.

## v1.2 decisions (2026-11-23)

**DD-49 — Item history: denormalized bill date, one row per bill, separate tabs (FR-23; schema v9).** `sale_items.transaction_date` and `purchase_items.transaction_date` copy the header date (back-filled in v9, written on insert, updated in `update_header_for_edit`; invariant: line date = header date). Indexes `(item_id, transaction_date DESC, <bill fk> DESC)` serve each 10-row page with an ordered index scan (`SELECT DISTINCT bill, date ... ORDER BY date DESC, bill DESC LIMIT/OFFSET`, joined to the header for code/party). A query-plan test forbids `USE TEMP B-TREE`. Sales and Purchases are separate tabs (two single-arm queries, no merge). Purchases list `source = PURCHASE` only. *Rejected:* joining lines to headers and sorting per page (cost grows with the item's history); one merged Sales+Purchases list.

**DD-50 — Inline item quick create (FR-24).** One `ItemQuickCreateModal` on New Sale (stock on hand + purchase price required → an opening-stock `OPN-` lot via `create_item.initial_stock`) and on New Purchase (no stock fields). Opened by `+ Create`, Enter on no match, or Alt+N. Category and barcode leave the item UI only; an edit re-sends the stored values, so the backend, import and FTS keep them.

**DD-51 — Lot purchase price edit (FR-25).** `update_lot_purchase_price` is a targeted correction, not a full bill repost:
- **PURCHASE lots:** bill guards → `edit_purchase_lot` (re-costs allocations, vendor-return lock) → header totals from the *stored* line totals + existing charges (no rebuild from rates, so no rounding drift) → UPFRONT replaced → `recompute_bill`.
- **Payment rule:** `applied = Σ SETTLEMENT + Σ RETURN_CREDIT`, `open = new_total − applied`.
  - `open < 0` → `TOTAL_BELOW_APPLIED`.
  - A bill paid entirely upfront (balance 0 and applied 0) stays PAID with `upfront = new_total`.
  - Otherwise `upfront = min(old_upfront, open)`, with the status derived from it.
  - No upfront is ever invented.
- **OPENING / ADJUSTMENT lots:** re-cost only. For ADJUSTMENT, the matching `stock_adjustments.unit_cost` is updated (found via the `Stock adjustment #id` header note).
- **RETURN lots:** read-only.
- **Sale bills:** never change; only COGS does.

**DD-52 — One settlement path; settlements are correctable (FR-26).**
- `settle_bills_batch` (one `BEGIN IMMEDIATE`, 1..=200 unique allocations, all or nothing) is the core. `settle_bill_payment` wraps it: a bill id is a batch of one, and a null bill gets a server-side FIFO allocation. `apply_return_credit` shares the FIFO allocation builder.
- `update_settlement` and `delete_settlement` act on `kind = SETTLEMENT` only. The new amount must be ≤ balance + old amount. Both end in `recompute_bill`.
- The bill details drawer shows Settle whenever a balance and a party exist.
- No grouping id for lump sums (deferred).

**DD-53 — Printing via the webview (FR-27).** A pure `BillPrintDocument` is rendered into a `#print-root` portal, then `window.print()` is called. Print CSS hides the app and sets `@page A4`. There is no plugin and no Rust PDF. The header is the existing Shop Profile, with no new settings. Rate is shown per pack (`total / pack_quantity`).

**DD-54 — Terminology, discounts, deletes.**
- *Purchase Price* = lot `unit_cost`. *Sale Price* = item `default_sale_price`, a suggestion only; lots carry no sale price.
- Discounts stay bill-level (FR-19). Only the derived `net_unit_price` refund cap is per line.
- A hardened two-step delete is deferred until accidental deletes are seen. Bill deletes keep the typed code; settlement delete uses a danger confirm.

**DD-55 — Integer paise money model (deferred; separate feature).**
- **Storage and types:** money is stored as `INTEGER` paise, uses a `Paise(i64)` newtype in Rust (no `From<f64>`), and crosses IPC as paise. Input is parsed as a string.
- **Totals and rates:** totals are primary; rates are derived for display only.
- **Proration:** cumulative allocation, `cost(n) = round(total × n / qty)`, where a slice costs `cost(b) − cost(a)`. A fully consumed lot sums exactly to its total, and no "extra paisa" unit is tracked.
- **Quantities:** stay `REAL`; milli-units are an option.
- **Migration:** money columns are rebuilt with `CAST(ROUND(x*100) AS INTEGER)`, checked against sum invariants, and the invariant suite moves to exact equality.
- *Why deferred:* it touches every money field. It should be done mechanically in its own branch set, not mixed with v1.2 features.

**DD-56 — Business rules only in the backend; previews are dry runs (2026-11-24).**
- **Rule:** the frontend must not compute or copy money, payment-status, settlement-split, refund-split, FIFO-cost or stock rules (owner decision). Copies drift as the codebase grows.
- **How previews work:** each screen that shows a figure before Save calls a `preview_*` command. It runs the same service code as the real command inside `BEGIN IMMEDIATE`, then drops the transaction (rollback on drop). It returns the outcome and the same `{field, message}` errors.
- **Totals while typing:** bill totals on screen come from a debounced `preview_sale` / `preview_purchase`. Calls inside the app are local, a few ms.
- **Tests:** Rust tests assert `preview(x) == command(x)` and that a preview leaves no rows behind. The TS rule copies and their tests are deleted (`lotPriceEffect.ts`, `allocateFifo`, `validatePayment` / `effectivePaid`, the return credit/cash split, `estimateFifoCost`, `latestLotCost`, `billTotals`).
- **Mock client:** stays a thin fixture client (DD-27). It returns canned previews, with no rules.
- *Amends* DD-51's shared TS/Rust case table (Rust only now).

**DD-57 — Bill edit is in-drawer, no page hand-off (2026-11-24, v1.2 C1).**
- **Decision:** `Edit` in the bill drawer turns the drawer into a form (header, lines, charges, payment). The sale / purchase entry pages no longer have an edit mode, and `billEditHandoff.ts` is deleted.
- **Why:** the hand-off queued a request that the page consumed in a mount effect. Under StrictMode the effect runs twice, the second `takeBillEdit()` returned null and the page showed a blank bill (same class as `bc66322`). The owner also wanted to add, remove and re-price lines where the bill is shown.
- **Rules kept:** DD-56 (all figures from `previewPurchase` / `previewSale`), FR-20 guards via `getBillEditGuard`, payload and backend commands unchanged. No schema or IPC change.
- **Side effect:** the edit never touches the sale cart store, so a sale in progress on the billing screen stays as it is (the "Discard current cart?" confirm is gone).
