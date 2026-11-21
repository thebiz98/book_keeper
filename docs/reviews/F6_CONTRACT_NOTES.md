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
