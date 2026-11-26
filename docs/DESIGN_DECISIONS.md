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
  - without it: FIFO across the item's lots, limited to available stock. *(Superseded in v1.7 by DD-86: the lot is required.)*
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
- **In-place purchase lot edits.** Allocations point at lot ids, so a purchase edit compares old and new lots: kept lines update the lot (id kept), removed lines are deleted if unreferenced, new lines create lots. Header, charges and UPFRONT ledger are reversed and reposted. A sale edit is a full reverse-then-repost because an unlocked sale line has no references. *(Superseded in v1.7 by DD-89: change-only sale edit.)*
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
  - A bill paid entirely upfront (balance 0 and applied 0) stays PAID with `upfront = new_total`. *(Superseded in v1.7 by DD-96: any fully paid bill stays paid.)*
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

**DD-58 — Entry dates cannot be in the future (2026-11-25, v1.3 FR-28).**
- **Decision:** one backend helper, `check_entry_date(field, date, today)`, runs next to `check_date` in every create / update / preview that takes an entry date (sale, purchase, expense, settlement single / batch / edit, returns, stock adjustment, set stock, opening balance). "Today" is `chrono::Local` behind an injectable clock, so tests pin it.
- **UI:** date inputs get `max = todayLocal()` as input help only (DD-56: the rule and its message come from the backend / preview).
- *Why:* manual delayed entry (Assumption 4) is always backwards in time. A future date silently moves money and FIFO into days that have not happened yet.

**DD-59 — Bundle create lives in the Unit dropdown; one-line entry rows (2026-11-25, v1.3 FR-29/FR-30).**
- The Unit select ends with a sentinel option "+ New bundle…". Selecting it opens `BundleCreationModal` and reverts the select to the previous value until the new bundle is saved, so the sentinel is never a real unit. `Alt+B` does the same. The entry-row Bundle buttons are removed. The Item List gets a row icon for the same modal.
- Entry-row fields use one shared field wrapper: a fixed one-line label slot, the control, and a reserved helper slot. All controls in a row then share the same top and bottom edge, whatever the label or helper text is.
- No schema or IPC change.

**DD-60 — Sale-row lot prices come from a lot-draw dry run (2026-11-25, v1.3 FR-31).**
- `preview_lot_draw { item_id, quantity, prior_quantity }` runs the real `InventoryAllocationStrategy` read-only for `prior_quantity + quantity` and reports the slice used by `quantity`, together with all open lots in FIFO order. The UI only formats it (DD-56). `prior_quantity` is the base-unit sum of the same item's lines already in the cart (the same base-unit mapping the payload already uses).
- *Why:* the old "Rate: ₹x" on the info line only echoed the input. The useful fact at the counter is what the units cost, and that can span lots.

**DD-61 — Dashboard grouped cards (2026-11-25, v1.3 FR-32).**
- Three cards: Today (sales, purchases), Dues (receivables, payables) and Cash & Stock (cash in hand, today's expenses, inventory valuation). Each row is a link. Net profit leaves the dashboard; it is a period figure that belongs in Reports.
- `DashboardMetrics.today_net_profit` is removed (so no field is left unused), which also drops the P&L query from the dashboard path.

**DD-62 — Cash in hand is all-time with an opening cash (2026-11-25, v1.3 FR-33).**
- `cash_in_hand = shop_settings.opening_cash + cash_position(FIRST_DAY, today).net`. The single drawer formula stays `cash_position`; only the range and the opening term change. Schema **v10** adds `opening_cash REAL NOT NULL DEFAULT 0 CHECK (opening_cash >= 0)`.
- Cash in hand has no edit path. Opening cash is a setup value in Settings.
- *Why:* the owner counts the physical drawer, which carries over day to day. A daily total that drops to 0 at midnight did not match it.

**DD-63 — Bill profit is computed in the backend, on request (2026-11-25, v1.3 FR-34).**
- **Bill profit** = `total_amount − Σ allocation cost` − linked-return refund value + restocked return cost − replacement issue cost (scrap units keep their cost as lost). It is computed in SQL per bill only when `include_profit` is true, so the default history query is unchanged.
- **Line profit** = `line_total − discount share (bill_math::net_unit_price) − net allocation cost`. Charges and return effects are shown as reconciling rows.
- The toggle is session state in the UI (not persisted). Every app start begins hidden, because customers can see the counter screen.

**DD-64 — Party opening balance as an opening bill (2026-11-25, v1.3 FR-35).**
- **Decision:** an opening balance is a line-less bill on the party's side (`OB-` code, own `code_sequences` prefix), marked by a new opening-bill flag on `sale_transactions` / `purchase_transactions` (schema **v11**, `ADD COLUMN` with no rebuild) and a partial unique index (one per party). The purchase-side source stays `PURCHASE`, so the settlement path accepts it unchanged.
- Every sales / purchase aggregate, list, picker, trend, profit and print path excludes flagged bills. Dues, ledger and settlement include them. A test enumerates the excluding queries.
- Created / updated / removed inside `create_party` / `update_party` (one `BEGIN IMMEDIATE`). The amount cannot go below the settled amount. Bill edit and delete (FR-20) reject opening bills.
- *Why:* reusing the bill shape gives settlement, the oldest-first fill, ledger rows and dues for free. A bare number on the party would need a second dues model.

**DD-65 — Service components are a relation table; service lines draw through the existing allocations (2026-11-25, v1.4 FR-36/FR-37).**
- `service_components (service_item_id, component_item_id, quantity_per_unit)` is the bill of materials: one row per pair, unique, no self-reference. Item type rules (service → physical only) live in the item service. Schema **v12** also adds `items.service_cost` and `sale_items.service_cost_total`.
- A service sale line writes ordinary `sale_stock_allocations` rows against the components' lots. COGS, bill profit, edit/delete reversal and lot re-cost then work unchanged, because they already go through allocations. Code that assumed `lot.item_id = line.item_id` is reviewed (lot draw preview, item history, return reversal).
- *Why:* a JSON list on the item can't be indexed or joined ("which services use this paper?", "Can make N"). A separate service-allocation table would duplicate the FIFO audit.

**DD-66 — Service cost is a floor over the real FIFO cost (2026-11-25, v1.4 FR-37).**
- Line cost = `C + max(0, service_cost_total − C)`, where C is the net FIFO cost of the components drawn. Only the **target** (`service_cost_total = round2(service_cost × qty)`) is stored. Misc. cost is derived in SQL, so a lot re-cost or a partial reversal can't leave a stale stored difference.
- Misc. cost is shown as its own P&L line under COGS, never folded into FIFO COGS, so inventory figures still reconcile to lots.
- *Why (owner, 2026-11-25):* "use per item FIFO purchase price, but if I define the service cost more than the sum of unit prices, add that difference as a miscellaneous cost". The floor means the service is never costed below what it really took from stock.

**DD-67 — Services are not returnable (2026-11-25, v1.4 FR-37, owner decision).**
- Return pickers omit service lines and the return service rejects them (also as replacement issues). Corrections go through bill edit/delete.
- *Why:* consumed components (printed paper) can't be restocked, and a refund-only path would need a new cost rule for no real use.

**DD-68 — Item analytics are computed on request (2026-11-25, v1.4 FR-38/FR-39).**
- Item profit and inventory speed are single aggregate queries over existing tables and indexes. No per-day analytics table.
- Inventory speed counts units out from `sale_stock_allocations` joined to the lot's item. One path then covers direct sales and service consumption, net of returned units.
- Class: rank items with units out > 0; top 20% Fast, bottom 20% Slow (`ceil(0.2 × n)`, at least 1, top wins when n is small), the rest Normal; units out 0 with stock > 0 is Dead.
- *Why:* at shop scale (thousands of items, tens of thousands of lines a year) the indexed aggregate runs in milliseconds. A cache would have to be kept correct through every FR-20 edit, delete and return, which is exactly the kind of drift the project avoids.

**DD-69 — Dashboard: cash first, lookup with a fixed search field (2026-11-25, v1.4 FR-40).**
- Cash & Stock is a full-width row above Today and Dues. Low stock moves into the right column beside them. Inventory valuation links to Inventory.
- The item lookup's suggestions render inline in their own scroll area below a fixed search field. They no longer render as an absolute dropdown inside a scrolling modal body, which made the body (and the field) scroll.
- The trend becomes an SVG line chart (time on x, ₹ on y from 0, one axis). It is hand-built because the app has no chart library, and two series don't justify adding one. Colours: categorical slots 1–2 (blue / orange), selected per theme and validated for CVD. Light orange is under 3:1 contrast, so the tooltip and a table view are required.

**DD-70 — One import pattern for every importable entity (2026-11-25, v1.4 FR-41).**
- Each kind (items, service components, parties, expenses) has a template writer, a pure parser/validator returning the existing `ImportValidationReport`, and an execute step in one `BEGIN IMMEDIATE` transaction through the normal service code (FR-35 opening bills, FTS sync, code sequences). The parsing core (CSV/XLSX via `csv` / `calamine`, header matching, date and number parsing) is shared.
- No import of sale / purchase history: replaying FIFO over imported bills is a migration project, not an import.

**DD-72 — Settlement history is a tab on Credit & Dues (2026-11-25, v1.4 FR-42).**
- A third tab, not a new sidebar page: settlements belong to the khata, and the user already goes there to settle. One new read command, `get_settlement_history`, pages in SQL over `idx_settlements_date`. Edit and delete call the existing `update_settlement` / `delete_settlement` (FR-26), so no rule is duplicated (DD-56).
- Only `SETTLEMENT` rows are listed, because those are the only rows editable on their own. Other ledger kinds would appear as rows the user can't act on.
- *Why (owner, 2026-11-25):* edit/delete existed but were "nested inside the view section". Settlement history across all parties is a daily check.

**DD-73 — Bill print as one items area with charges at its foot (2026-11-25, v1.4 FR-43).**
- The print is one bordered items area: line rows, a flexible spacer, then charge/discount rows and the Total row. On a single page the spacer pushes the charges to the bottom of the area. A multi-page bill drops the spacer, repeats the `<thead>`, and lets charges follow the last item. CSS print only (`@page A4`, `break-inside: avoid` on rows); no PDF library.
- The print reads only the stored bill (lines, pack snapshots, charges, totals); it computes nothing (DD-56).
- *Why (owner, 2026-11-25):* the layout of a familiar shop bill book, with the party at top left and the date at top right.

**DD-71 — Entity review outcomes (2026-11-25, v1.4).**
- **Doc drift fixed in the HLD:** the ERD now shows `items/parties.updated_at`, return name snapshots, `is_opening_balance`, `opening_cash`, `backup_directory` and `CODE_SEQUENCE`. `balance_due` is documented with `RETURN_CREDIT`. The stale "cash in hand = today" query row is corrected.
- **Real redundancy removed:** `idx_sale_items_item (item_id)` duplicates the prefix of `idx_sale_items_item_date`; v12 drops it.
- **Deliberate caches kept** (each has one writer and is recomputed on every change): `current_stock`, `is_low_stock`, bill `items_subtotal / charges_total / discounts_total`, `paid_amount / balance_due / payment_status`, line `transaction_date`, return `total_amount`.
- **Kept as is:** purchase headers carry two markers (`source`, `is_opening_balance`). An opening bill is always `source = PURCHASE`. A rebuild to merge them is not worth the migration risk. Polymorphic references (`payment_settlements.transaction_id`, `bill_charges.transaction_id`, `stock_outflow_allocations.source_id`, `return_transactions.original_transaction_id`) stay without SQL FKs, guarded by the services.
- **Cash in hand is not stored** (see HLD §6.2 "Derived, not stored").
- **Recommended later, not scheduled:** a read-only "Verify data" check that recomputes every cache above and reports orphans of the polymorphic references; integer paise (DD-55).

**DD-74 — Soft delete reuses `is_active`; no archive table, no migration (2026-11-25, v1.5 FR-45 / FR-46).**
- `items.is_active` and `parties.is_active` (INTEGER 0/1 with a CHECK, since v1) are the delete flag. SQLite has no boolean type, so 0/1 is the standard form, and read paths already filter on it (DD-17).
- A separate "deleted items" table was rejected. `sale_items`, `purchase_items`, `return_items`, `stock_adjustments`, `service_components`, bill headers and `payment_settlements` reference these rows by id (FK). Moving rows out would break the FKs, and every history query would need a second join.
- "Deleted on" is `updated_at`: every write to a deleted record is rejected except restore, so `updated_at` stays the delete time. The UI says **Delete** and **Deleted items**; the record is never physically removed.

**DD-75 — Search indexes hold active rows only (2026-11-25, v1.5).**
- `item_search_fts` and `party_search_fts` rows come from `WHERE is_active = 1` sources (`fts_specs.rs`). Delete reindexes the key after commit (DD-24), which removes the name row and every bundle-barcode row. Restore reindexes it back. Every search query also joins `is_active = 1`, so a stale row is never shown.
- The startup check (`ensure_search_index` / `ensure_fts_index`) compares active count with indexed count and rebuilds on a mismatch, so a crash between commit and reindex heals itself. A rebuild ends with FTS5 `optimize` to compact segments. Index size follows the active rows only.
- The Deleted lists don't use FTS: they are small, so `name LIKE ?` over `WHERE is_active = 0` with SQL paging is enough.

**DD-76 — Item delete rules (2026-11-25, v1.5 FR-45; owner decisions).**
- Delete with stock is allowed after a confirm that shows the stock value (owner). The lots stay untouched; Inventory value excludes deleted items by joining `items.is_active = 1`, and a restore brings the value back exactly.
- A physical item used as a component of an active service can't be deleted: the service would become unsellable.
- New returns and replacements of a deleted item are rejected with "Restore the item first" (owner). Otherwise returned stock would land on a hidden item.
- Bill edits may keep existing lines of a deleted item, the same way they keep a deactivated party (`keep_party`). Without this rule, deleting an item would lock every old bill that contains it.
- Names stay unique including deleted items (`UNIQUE COLLATE NOCASE`). A clash returns `ITEM_NAME_DELETED` with the id, so the UI offers Restore instead of a raw constraint error.

**DD-77 — Party delete rules (2026-11-25, v1.5 FR-46; owner asked for parties too).**
- Deleting is blocked while any bill of the party has `balance_due > 0`: hiding a party with money owed would hide that money from the khata.
- Credit & Dues summaries drop their `is_active = 1` filter (a party with dues shows with a Deleted badge). That covers the rare case where a later bill edit leaves a due on a deleted party. Settlements on such a party are allowed.
- New returns against a deleted party's bill are rejected with "Restore the party first", the same rule as items.

**DD-78 — Lots in the item edit screen reuse FR-25 (2026-11-25, v1.5 FR-44).**
- An item has no purchase price of its own; each lot has one (FR-25 terminology). The edit screen embeds the existing editable `LotInspector` (`get_item_lots`) and `LotPriceEditModal` (`update_lot_purchase_price`), so no backend change and no rule is duplicated (DD-56).

**DD-80 — Parties with any history are only soft-deleted; unreferenced parties are removed (2026-11-25, v1.5 FR-46 amendment; owner decision).**
- Bills and returns snapshot `party_name`, but phone, the party ledger, dues summaries and settlement history read `parties` live, and `sale_transactions`, `purchase_transactions`, `return_transactions` and `payment_settlements` all reference `party_id` (4 foreign keys). So the `parties` table is the source of truth for anything with history: such a party is only ever soft-deleted (DD-74).
- A party with **zero references** (no row in those four tables, any purchase source) has no history to protect (typically a bad import row): delete removes the `parties` row and its `party_search_fts` rows (after commit). An imported opening balance blocks through the dues rule while due; setting the opening balance to 0 removes the unsettled `OB-` bill (FR-35) and the party becomes unreferenced. Delete stays one action; the backend picks the mode (DD-56).
- Restore lives only in the Deleted parties list on the Parties page (customers and vendors together). Other screens show at most a "Deleted" badge.

**DD-79 — Never-used items are removed permanently (2026-11-25, v1.5 FR-45 amendment A1; owner decision).**
- Import mistakes leave items that were never used. Delete stays one action; the backend picks the mode (DD-56). **HARD** when the item appears in no `sale_items`, `return_items` or `stock_adjustments` row, has no `purchase_items` under a non-`OPENING` header, every `OPENING` lot is untouched (`remaining_quantity = quantity`, no `sale_stock_allocations` / `stock_outflow_allocations` on it), and it is not a component in any `service_components` row. Its own bundles and its own component rows (as a service) are not "use". Otherwise **SOFT** (DD-74).
- Safe because nothing references the row after its OPENING lots, bundles and own component rows are removed (the 8 `REFERENCES items(id)` foreign keys are `item_bundles`, `purchase_items`, `sale_items`, `return_items`, `stock_adjustments` (two versions of the table) and `service_components` x2). An imported item's opening lot shares one `OPN-` header per date: only this item's lines go, and the header is dropped when it is left empty.
- A soft-deleted item that is still unused can be removed permanently from the Deleted items list; the same check decides.

**DD-82 — An edited item total sets that line's rate (2026-11-26, v1.6 FR-47; owner request). Replaces DD-81, withdrawn the same day.**
- DD-81 booked the difference between the typed total and `qty × rate` as "Item extra charge" / "Item discount" bill lines (schema v13). The owner did not want item-level adjustments: the cashier already sets the rate while selling, so a typed total should simply change the rate. G2 was reverted on main (`85006ab`), schema v13 never ships, and the owner's database was rolled back to v12 (backup taken; the two system types were unused).
- `preview_sale_line { bundle_quantity, unit_price, entered_total? }` returns `{ line_total, unit_price }`; with a total and qty > 0 the rate is `entered_total ÷ qty`, not rounded, so `round2(qty × rate)` gives the typed total back. The sale payload and the save path are unchanged (the line just carries that `unit_price`), so returns, profit, edit and reports need nothing new (DD-56: the division is the backend's).
- Rates are displayed to 2 decimals as everywhere else; a stored rate such as 333.333… is expected.

**DD-83 — Low Stock Level is edited in the item details drawer, not inline in the Item List (2026-11-26, v1.6 FR-50; owner request). Amends DD-45.**
- The always-an-input column of DD-45 made the level too easy to change by accident while moving through the list, and the level is not first-class information on that page. The column and `LowStockLevelCell` are removed.
- The details drawer shows the level read-only with an explicit Edit → Save / Cancel step. Same command (`update_item_low_stock_threshold`) and validation; no backend change. The Status badge (server flag) stays in the list.

**DD-84 — Opening quantity is corrected in place; it is not an adjustment (2026-11-26, v1.6 FR-51; owner request).**
- An item entered with opening 6 when only 1 ever existed cannot be fixed with Set stock: that posts a COUNT_CORRECTION whose allocations count as Inventory Losses. A wrong entry is not shrinkage.
- `update_opening_lot_quantity` / `preview_opening_lot_quantity` edit the OPENING lot through the same `edit_purchase_lot` a bill edit uses: the floor is the already-used quantity (`LOT_BELOW_CONSUMED`), remaining and the item stock cache follow. Quantity 0 on an unused lot removes the lot and its OPN header. Cost is kept; line total = round2(qty x cost). OPN headers stay at total 0 (DD-07: not a bill), so nothing else changes.
- No `stock_adjustments` row and no allocation, hence no P&L loss and COGS of earlier sales unchanged. Deleting a sale later gives its units back to the lot as usual. The preview is a dry run of the same body (DD-56); the dialog holds no rule.

**DD-85 — A stock adjustment is undone by removing it, not by a reversing entry (2026-11-26, v1.6 FR-52; owner request).**
- A wrong Set stock (for example a count correction made instead of fixing the opening quantity) leaves a false loss. A reversing +adjustment would keep the loss and add a second record, so the adjustment is deleted instead; past P&L, stock valuation and the dashboard recompute. Accepted: the audit trail of the undone row is lost, as with bill deletes.
- Reduction: each `stock_outflow_allocations` row (`ADJUSTMENT`, adjustment id) gives its `quantity_depleted` back to its lot (never above the lot quantity; an overflow is an error, not clamped) and to the item cache; the rows and the adjustment are deleted. Increase: the `ADJ-` lot found by the `Stock adjustment #id:` note is removed with `remove_purchase_lot` (refused when referenced) and the adjustment is deleted. Deleted item: refused (restore first).
- `get_adjustment_undo_guard` and `undo_stock_adjustment` share one body; the guard is a dropped transaction (DD-56). The dialog holds no rule.

## v1.7 decisions (2026-11-26)

Money and stock correctness after the owner's full review (HLD FR-53..FR-59, §16; owner decisions D1..D11 of 2026-11-26). Finding ids (R, E, S, L) are those of the v1.7 review. No schema migration; v13 stays unused.

**DD-86 — A vendor return takes exact lots only (2026-11-26, v1.7 FR-53, owner D9; findings R1, R2, R10). Supersedes the FIFO bullet of DD-05 and HLD Assumption 11 for vendor returns.**
- Every vendor-return line needs `purchase_item_id`; a line without it is refused with `VENDOR_LOT_REQUIRED`. The FIFO path of `remove_stock_for_vendor_return` is removed, so a vendor return can no longer draw OPENING, ADJUSTMENT or RETURN lots, other vendors' lots or other bills' lots (R1).
- Eligible lot: header `source = PURCHASE`, header `party_id` = the return's party (R2), and, when the return is linked to a bill, a lot of that bill; `remaining_quantity > 0`. Anything else is `VENDOR_LOT_NOT_ELIGIBLE`.
- The backend query `list_vendor_return_lots(item_id, party_id, purchase_id?)` applies the same filter (one shared predicate) and feeds the picker and the per-lot maximum, so the UI holds no rule (DD-56). With exactly one eligible lot the UI preselects it.
- Quantity checks are made per lot over all lines of the return (a simulated draw), so an overdraw is reported on the line that causes it (R10).

**DD-87 — Vendor-return value is net of that bill's discount (2026-11-26, v1.7 FR-53, owner D11; finding R3). Amends DD-48 "Vendor-return cost lock" (the value is no longer the plain lot cost).**
- Value of a line = qty × gross lot cost × that bill's own **net factor** = `net_unit_price(1, items_subtotal, discounts_total)` = `(items_subtotal − discounts_total) ÷ items_subtotal`, from the stored header of the lot's bill. Discounts only: charges (`bill_charges` rows with effect `CHARGE`) are excluded and stay a purchase expense (HLD Assumption 18). This is the rule customer returns already use (FR-19).
- The stock leaves at gross cost (`cost_amount` = round2(qty × lot cost)); `RETURN_ITEM.total_amount` = round2(qty × lot cost × factor). The difference is a **"Purchase discount reversed"** P&L line on the return date, subtracted from Other Income, so the discount is counted once (R3 counted it twice: once as income, once inside a phantom `REFUND_IN`).
- Example: ₹1000 bill with a ₹100 discount, all 10 units returned → credit ₹900, no cash, ₹100 discount income reversed.
- The vendor-return cost lock of DD-48 stays (`VENDOR_RETURNED_COST_CHANGE`): the value was settled at the old cost.

**DD-88 — Party change on a bill edit needs an explicit settlement move (2026-11-26, v1.7 FR-54, owner D6; findings L1, R7, E7). Amends DD-48 / HLD FR-20 "re-pointed" settlements.**
- Return credit (unlinked-return `RETURN_CREDIT` rows) still blocks a party change (`PARTY_CHANGE_WITH_CREDITS`).
- `SETTLEMENT` rows move with the bill only when the payload carries `confirm_move_settlements = true`, set by the UI after its confirm dialog. Without it the backend refuses with `BillLocked SETTLEMENTS_ON_BILL` naming the count and amount. `BillEditGuard.settlements_to_move { count, amount }` tells the UI what the dialog says. Both parties' ledgers move in the same transaction.
- A party's `party_type` is locked once any bill, return or settlement references it (L1). A return with no linked bill takes only a party of the matching type (R7).

**DD-89 — Change-only sale edit (2026-11-26, v1.7 FR-55, owner D3; findings E1, E2, E3, E4, E5, E9). Supersedes the "full reverse-then-repost" sale bullet of DD-48, the reverse-then-repost wording of DD-41, HLD Assumption 19 (old form) and DD-09's consequence that "re-posted sale lines take the oldest lots available at edit time".**
- Lines match by `sale_item_id` (new optional payload field; `create_sale` rejects it):
  - Unchanged: no write.
  - Rate only: update the price fields (`unit_price`, `total_amount`).
  - Quantity down: release from the newest allocation first via `reverse_sale_allocations` (restock = true, `returned_quantity` untouched; the allocation shrinks or goes).
  - Quantity up: FIFO-draw only the extra.
  - Item or pack changed, or a new line: release in full, draw fresh.
  - Removed: release in full.
- Service lines scale their stored `service_cost_total` per unit and their stored component ratios (allocations per component ÷ line quantity), never the live service definition.
- The UPFRONT row is updated in place (amount, date), never deleted and re-created; it is deleted only when the new upfront is 0, and created only when there was none.
- Charges keep their stored rows (name, effect) unless the user changed that row (E9).
- The purchase edit keeps `item_name` and `pack_label` on kept lots whose item didn't change (E2).
- The edit form sends `sale_item_id`, loads each line's item and bundle by id (`get_item`, `bundle_id`), and gates Save on `dirty` (E3, E4). So a notes-only save leaves allocations, costs, names and the UPFRONT row byte-identical.
- DD-09 itself (FIFO = entry order) stands; only the extra quantity of an edited line takes the oldest lots at edit time.

**DD-90 — Deleted references are frozen (2026-11-26, v1.7 FR-54 / FR-55, owner D10; findings E5, E6). Amends DD-76 ("Bill edits may keep existing lines of a deleted item": only unchanged) and DD-77 ("Settlements on such a party are allowed": no longer).**
- History is kept untouched: an unchanged line or bill that references a deleted item or party stays as it is.
- Any write that uses one fails with the new `AppError::DeletedReference { entity: ITEM | PARTY, id, name, message }` ("'X' is deleted. Restore it first."): a changed or added line of a deleted item (including a deleted service's components, consumed through a kept service line), a raised due on a deleted party, a settlement (create, edit, delete), a return, or a lot reprice that changes its bill.
- The UI shows the message with a **Restore** button (`restore_item` / `restore_party`) and then re-runs the preview; the user continues in the same form. The party half ships in H2, the item half and the shared UI in H3, the settlement / return / reprice checks in H7.

**DD-91 — Returns post in the return period (2026-11-26, v1.7 FR-56, owner D2; findings R4, R5, R6, R8). Amends DD-13 / HLD §7 Period COGS (net of returns by sale date) and DD-68 item profit dating.**
- COGS by sale date is gross (`quantity_depleted × unit_cost`). The returned cost (`RETURN_ITEM.cost_amount`) is credited on the return date: linked lines (restockable or scrap; a scrap line's cost then moves to Inventory Losses on the same date, so it is counted once) and unlinked restockable lines (R4: cost = qty × the new `RETURN` lot's cost). Closed months never change.
- Item analytics use the same rule: sales by bill date, returns (linked and unlinked, by `ri.item_id`) by return date, so Σ item profit = P&L gross profit for a range.
- Bill profit subtracts its linked return **line** values, not the return header, and counts linked lines only, the same as item profit (R6).
- The return header total = Σ rounded lines (R6). The refund cap is per sale line on the total: Σ refunded over all returns of that line ≤ its net total (`round2(line total × net factor)`) + `MONEY_EPS`, not a per-unit tolerance (R8).

**DD-92 — Found stock is never a gain (2026-11-26, v1.7 FR-57, owner D5; findings S3, S4, S7, S8, S11). Amends DD-08 and DD-40.**
- A positive adjustment allows only `UNRECORDED_STOCK` and `COUNT_CORRECTION` (found); `DAMAGED`, `EXPIRED` and `THEFT_LOST` are reduction-only (S3). No P&L gain is ever booked; the stock is realised when it sells.
- `preview_stock_adjustment` / `preview_set_item_stock` return `undoable_writeoffs` (earlier reductions of the item whose G7 `get_adjustment_undo_guard` says `can_undo`), so the "Undo it instead" hint comes from the backend.
- A typed cost of 0 is honoured (S4: the UI no longer turns 0 into null). The default cost is the latest `PURCHASE` or `OPENING` lot, then the latest lot of any source (S8), so one mistyped adjustment cost does not carry forward.
- The Lot Inspector orders lots by id and labels it "FIFO order (entry order)" (S7). The reprice modal states that past COGS and losses using the lot will change (S11).

**DD-93 — Deleting an item with stock: Just delete or Book as loss (2026-11-26, v1.7 FR-58, owner D4; finding S5). Amends DD-76 "Delete with stock is allowed".**
- `delete_item { item_id, write_off }`. `write_off = false` (the UI default, "Just delete") is today's soft delete: lots untouched, no P&L entry, restore brings the stock back.
- `write_off = true` posts, in the same transaction and before `set_active(false)`, one reduction adjustment for the whole remaining stock through the normal negative-adjustment path (FIFO outflow allocations, Inventory Losses on today's local date), notes "Written off on item delete". Reason: `THEFT_LOST`. A new `WRITE_OFF` reason was considered, but `stock_adjustments.reason` has a `CHECK (reason IN (...))` since schema v5, so adding one needs a table rebuild, and v1.7 has no migration.
- `preview_item_delete` returns `write_off { quantity, base_unit, loss }` (null without stock). A hard delete (DD-79) never has stock, so it is unaffected. Undoing the write-off (G7) requires restoring the item first.

**DD-94 — Date order (2026-11-26, v1.7 FR-59, owner D7; finding E10). Extends DD-58.**
- Checks run inside the `*_in_tx` bodies, so previews show them; error `ValidationError` with code `DATE_ORDER` naming the limiting date.
- Return date ≥ linked bill date. Settlement date ≥ bill date (single, batch, edit). Bill edit date ≤ earliest `SETTLEMENT` and earliest linked return (UPFRONT moves with the bill). Opening-balance As of date ≤ earliest settlement of the `OB-` bill.
- Not covered: stock adjustments (S10, DD-97).

**DD-95 — The lot floor counts restocked customer-return units as consumed (2026-11-26, v1.7 FR-59; findings S1, S2). Amends DD-48 "The quantity floor is `consumed = quantity − remaining_quantity`".**
- Floor = Σ sale allocations `quantity_depleted` on the lot (returned units included) + Σ outflow allocations. A restocked unit is back in `remaining` but still tied to its allocation, so shrinking below it would break the later return delete.
- `reverse_sale_bill_stock` and `RestoreOutflow` no longer clamp: a reversal that would take a lot above its quantity (or below 0) fails with `STOCK_INVARIANT`, and the stock cache is credited with exactly what the lot received.

**DD-96 — Lot reprice payment rule is independent of how the bill was paid (2026-11-26, v1.7 FR-59, owner D8; finding E8). Supersedes the DD-51 payment rule "A bill paid entirely upfront (balance 0 and applied 0) stays PAID".**
- A bill with `balance_due = 0` before the reprice stays fully paid: `upfront = new_total − applied`. This holds for upfront-only and upfront-plus-settlements bills alike. `open < 0` is still `TOTAL_BELOW_APPLIED`.
- A part-paid or unpaid bill keeps its upfront (capped at the new open) and takes the change in its balance due. `lot_price_upfront` in `bill_math.rs` is the one place.

**DD-97 — Rounding and cleanup (2026-11-26, v1.7 FR-59; findings S6, S9, R9, L2, L4, L5, L6, L7). Amends DD-15.**
- One `round2` semantics: Rust is the reference and `money.ts` is aligned to it, so the two never differ at half a paisa. Stored money is rounded (`create_source_lots` total, vendor-return `total_amount`).
- The per-line refund cap is on the total (DD-91). Totals and quantities must be finite and below a cap, so a line total can't overflow to infinity.
- A search reindex after commit logs and retries, and never fails the save (the data is committed).
- The base unit is locked once the item has stock or any bill line.
- A party import with a blank As of date keeps the existing date. The opening lot of a new item uses `today_local()` and `check_entry_date`.
- Set stock snaps a residue below 1e-6 to 0, and low stock uses `current_stock < threshold − EPS` (S9). `evaluate_return_guard` treats any error of the dry run as blocking delete (R9).
- UI rule moves (DD-56): typed cost 0, the per-pack price of `LotPriceEditModal`, `itemForm` / `BundleCreationModal` validation, the `ReturnFormVendor` lot filter and maximum, the `openBills.ts` FIFO sort, dues sums and the `NewPurchasePage` cost per base unit all come from the backend.
- **Left as they are:** S10, a back-dated negative adjustment is valued at today's lots (the preview shows the cost; date order does not cover adjustments). Previews take the write lock (`BEGIN IMMEDIATE` then rollback): a performance note only, no wrong data.
