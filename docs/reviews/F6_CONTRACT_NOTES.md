# Backend contract changes the frontend must adopt in F6

This file collects the changes from F2–F5. TypeScript types in `src/types/*` and `src/api/ipc.ts` still reflect the old shapes unless noted.

## Errors (F2, DD-26)
- Every command rejects with `AppError { type, details }`. Read the message with `errorMessage(e)` from `src/api/errors.ts`.

## Codes (F2)
- Clients no longer send `transactionCode` / `returnCode`. Remove the deprecated optional fields from the TS payloads, and the mock's use of them.
- `get_next_*_code` returns a display-only preview.

## Sales & purchases (F3) — verify with `git show 436305a -- src-tauri/src/core/dtos`
- `CreatePurchasePayload` gains `payment_status` and `supplier_invoice_ref`.
- The status/paid cross-check applies to both sales and purchases; a party is required for credit.
- The sale response includes `change_amount`. The purchase response is `{ purchase_id, transaction_code }`.
- Settlement returns `SettlementResult`. Purchases can be settled per bill or by lump sum.
- `get_party_ledger` is real.
- The stock adjustment payload gains `adjustment_date` and `unit_cost`. The result is `{ adjustment_id, loss_value }`.
- `get_item_lots(item_id)` is available.

## Returns (F4)
- `create_customer_return` / `create_vendor_return` → `ReturnResult { return_id, return_code, credited_amount, cash_refunded }`.
- `CustomerReturnPayload.replacement_items: [{ item_id, quantity }]`, and per-line `unit_cost_override: number | null`.
- `VendorReturnItemPayload.unit_cost` is removed; the value comes from the lot cost. `settlement_type` must be `REFUND`.
- `list_returns({ filter: { return_type?, start_date?, end_date?, page?, limit? } })` → `PaginatedList<ReturnTransaction>`.
- `ReturnDetailView` gains `allocations` and `ledger`.

## Items, bundles, search (F4)
- `create_bundle` → `ItemBundle`. New `update_bundle(bundle_id, payload: UpdateBundlePayload)` → `ItemBundle`. New `delete_bundle(bundle_id)`.
- `ItemSearchResult.matched_bundle_id: number | null`. Default search limit is 8.
- MRP must be `null`, never 0. Duplicate names and barcodes are rejected with `ValidationError` (field `name` / `barcode`).

## Import & backup (F4)
- `validate_items_import(file_path)` → `ImportValidationReport { valid_count, rows_with_errors[{row, field, message}], duplicates[{row, name, existing_item_id}] }`.
- `execute_items_import(file_path, conflict_strategy: "SKIP"|"OVERWRITE"|"ADD_STOCK")` → `ImportSummary { created, updated, skipped, stock_lots_created }`.
- `import_items_from_excel` is removed. Replace the TEMP Inventory control.
- `backup_database(destination_path)` takes an existing directory and returns the created file path.

## Pagination shape (current)
- `PaginatedList { items, total_count, page, limit, total_pages }`. F5/T8 may normalise this further; check the F5 notes appended below.

## F5 reports
- `DashboardMetrics { today_sales_amount, today_sales_count, today_purchase_amount, today_purchase_count, today_net_profit, total_stock_valuation, customer_receivables, vendor_payables, live_cash_in_drawer }`. Removed: `today_sales`, `today_purchases`.
- `PnLReport { gross_sales, sales_returns, net_sales, cogs, gross_profit, inventory_losses, net_profit, total_purchases }`. Removed: `from_date`, `to_date`, `total_discounts`, `returns_refunded`.
- `StockValuationReport { total_valuation, active_lots_count, categories_breakdown: [{ category, items_count, valuation }] }`. Removed: `total_items_count`, `categories`. Items with no category are grouped under "Uncategorized".
- `CashFlowReport { sales_cash_inflow, customer_settlements_inflow, vendor_refunds_inflow, purchase_cash_outflow, vendor_settlements_outflow, refunds_cash_outflow, net_cash_balance }`. Removed: `total_inflow`, `total_outflow`, `net_cash_flow`, `closing_drawer_balance`, `from_date`, `to_date`. Compute totals on the client if needed.
- `LowStockItem { id, name, category, base_unit, current_stock, default_sale_price, mrp }`. Removed: `item_id`. The list is no longer capped at 50. Order is `current_stock ASC, name`.
- `TrendDataPoint { date, sales_amount, purchase_amount }`. The `period` argument is `"7d"` (7 daily points), `"30d"` (30 daily) or `"1y"` (12 monthly points, `date` = `YYYY-MM-01`). It is zero-filled and oldest first. Any other value returns a `ValidationError` on `period`.
- New `get_recent_transactions({ limit })`, with `limit` from 1 to 100 (otherwise `ValidationError`), returns `RecentTransaction[] { kind: "SALE"|"PURCHASE"|"RETURN", id, code, date, created_at, party_name, amount, payment_status: "PAID"|"PARTIAL"|"UNPAID"|null }`, newest first.
- `get_pnl_report` and `get_cash_flow_report` return a `ValidationError` for a bad date or `from_date > to_date`.

## F5 lists (T8) — supersedes "Pagination shape (current)" above
- **One shape:** `Paginated<T> { items, total, page, limit }`. `total_count` is renamed `total`; `total_pages` is gone (UI: `ceil(total / limit)`). Applies to sales history, purchases history, catalog, returns, adjustments.
- **Validation (rejected, never clamped):** `page >= 1` (default 1); `1 <= limit <= 200` (default 50). Violations raise `ValidationError` with field `page` / `limit`. Defaults live in the services; commands pass `undefined` straight through.
- **Dates:** `start_date` / `end_date` are zero-padded `YYYY-MM-DD`, inclusive, `start <= end`, else `ValidationError` (field `start_date` / `end_date`). The legacy `from_date` / `to_date` aliases are removed.
- **Order:** newest first (`transaction_date DESC, id DESC`); returns by `return_date`, adjustments by `adjustment_date`; catalog by name (case-insensitive); parties by name.
- `get_sales_history({ filter })` -> `Paginated<SaleHistoryRow>`; `get_purchases_history({ filter })` -> `Paginated<PurchaseHistoryRow>`.
  - `filter = { start_date?, end_date?, payment_status?: "ALL"|"PAID"|"PARTIAL"|"UNPAID", party_id?, search_query?, page?, limit? }` (same for both).
  - `search_query` matches the transaction code or party name (purchases: also `supplier_invoice_ref`), case-insensitive for ASCII; `%` and `_` are literal; blank means no filter.
  - Purchases list `source = PURCHASE` only.
  - `SaleHistoryRow { id, transaction_code, transaction_date, created_at, party_id, party_name, line_count, total_amount, paid_amount, balance_due, payment_status }`. `party_name` is the DD-30 header snapshot.
  - `PurchaseHistoryRow` = the same fields plus `supplier_invoice_ref`.
- `get_sale_details` -> `SaleDetailView`: `items[]` gain `returned_quantity` (base units). Each line still has `sale_item { item_name, pack_label, pack_quantity, quantity, unit_price, ... }`, `item_name`, `bundle_name` (= `pack_label`) and `allocations[]` (FIFO cost). `settlements[]` = the bill's ledger rows.
- `get_purchase_details` -> `PurchaseDetailView`: `purchase.supplier_invoice_ref`; `items[].purchase_item { unit_cost, remaining_quantity, pack_label, pack_quantity, ... }`; `settlements[]`.
- `get_items_catalog({ category?, low_stock_only?, search?, page?, limit? })` (flat args) -> `Paginated<ItemSearchResult>`.
  - Rows are `{ item_id, name, category, barcode, base_unit, mrp, default_sale_price, current_stock, is_low_stock, bundles[], matched_bundle_id }`.
  - `bundles[]` are `{ bundle_id, bundle_name, units_per_bundle, bundle_price, barcode }`.
  - `search` goes through the FTS indexer; `category` and `low_stock_only` still apply.
  - `category` matching is case-insensitive. The old result was a bare `Item[]` (field `id`); it is now `item_id`.
- New `list_categories()` -> `string[]` (distinct, active items, sorted). Replaces the hard-coded category list.
- `search_items_fts({ query, limit? })` is unchanged for the client (the default limit 8 now lives in the service).
- `list_parties({ party_type?, active_only? })` and `search_parties({ query, party_type? })` -> `PartyListRow[]`.
  - `PartyListRow` = the `Party` fields flat, plus `outstanding_balance` and `transaction_count`.
  - Customers: sale side. Vendors: PURCHASE-source purchases.
  - Parties stay **unpaginated** (pickers need the full directory). `active_only` defaults to true; search matches name or phone.
  - The separate dues call for the directory is no longer needed.
- `get_party_dues_summary({ party_type })` -> `PartyDueSummary[]` (unpaginated; only parties with open bills).
  - Fields are `{ party_id, party_name, phone, party_type, unsettled_bills_count, earliest_bill_date, total_invoiced, total_paid, balance_due }`.
  - Totals are over the open bills. `earliest_bill_date` is the MIN of `transaction_date`.
  - Renamed: `total_due` -> `balance_due`, `oldest_bill_date` -> `earliest_bill_date`, `unpaid_bills_count` -> `unsettled_bills_count`.
- `list_stock_adjustments({ item_id?, page?, limit? })` -> `Paginated<StockAdjustment>` (still bare entities; item name not joined).
- `list_returns({ filter })` -> `Paginated<ReturnTransaction>`. The limit cap is now 200, rejecting instead of clamping; dates are validated.
