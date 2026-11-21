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

**DD-12 — Stock adjustment payload.** Adds `adjustment_date` and an optional `unit_cost` (see DD-08). The reasons are `DAMAGED | EXPIRED | THEFT_LOST | COUNT_CORRECTION`.

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
- **Deferred to v2:** every export, parties import, opening dues, VedBill migration, and restore from backup.
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
  - a backup is taken before migrating;
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
