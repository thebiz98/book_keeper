# Phase 1 — HLD ↔ LLD Alignment Review

> **Status: resolved (2026-11-21).** Every D-item below, plus D-25 (returns weren't reflected in P&L), was decided with the user and recorded in [`../DESIGN_DECISIONS.md`](../DESIGN_DECISIONS.md) (DD-01…DD-27). The HLD, LLD, UI LLD and UI/UX spec were all updated to match. This file is kept as the history of *why* those changes were made.

*Date: 2026-11-21. Scope: `book_keeper_HLD.md` (authority) vs `book_keeper_LLD.md` (backend), `book_keeper_UI_LLD.md`, `book_keeper_UI_UX_Specification.md`, `book_keeper_data_import_export_and_migration.md`.*

Severity: **S1** = money/stock correctness risk if code follows the doc · **S2** = requirement not specified or contradictory · **S3** = naming/cosmetic drift.

## 1. Requirement coverage matrix

| Req | Backend LLD | UI LLD / UI-UX | Verdict |
|---|---|---|---|
| FR-01 Purchases | §6.8, TC-PUR-* | Page 4, 8 | Covered. Overpayment (`paid > total`) unhandled → negative `balance_due` (D-07). |
| FR-02 Sales | §6.7, TC-SALE-* | Page 2, 3, 7 | Covered. Payload `payment_status` ignored in favour of derived status (D-08). |
| FR-03 FIFO | §6.2, TC-FIFO-* | Page 5 lot inspector | Covered. FIFO order = `id ASC`, not `transaction_date` (D-10). No IPC for lot inspector (D-19). |
| FR-04 Negative stock | §6.2 step 2, §6.7 step 3 | RapidBillingRow, Page 2 | Covered. Multi-line same-item cart: per-line validation vs aggregate (D-11). |
| FR-05 Low stock | §6.3 cache logic, TC-INV-006 | Page 1, 5 | Covered. |
| FR-06 Dues | §6.4, §6.5, TC-PAY/DUES | Page 6, SettlementModal | Covered but **upfront payment double-count risk** (D-01). UI only does FIFO settle; per-bill settle from history pages not specified in UI LLD (D-15). |
| FR-07 Returns | §6.9, TC-FIFO-004/005 | Page 9 | **Weakest area.** Vendor CashRefund modelled as `record_payment` (D-02), replacement charges COGS to original sale (D-03), no returned-qty ≤ sold-qty guard (D-04), repeated-return restoration not specified (D-05). |
| FR-08 Adjustments | §6.3, TC-INV-004/005 | Page 9 Tab 3 | Covered. Synthetic ADJ purchase pollutes purchase lists/counts (D-06); payload has no `adjustment_date` (D-12); fallback cost = sale price (D-13). |
| FR-09 Reports | §6.10 (trait only), TC-REP-* | Page 10 | **Thin**: no algorithm for cash flow, week/period handling, or how refunds are identified in `PAYMENT_SETTLEMENT` (D-01, D-14). |
| FR-10 Lists | §5.2 list_* repos | Page 5–8, 11 | Filters partially: `search_query` in HLD filter, not in repo signature (D-16). |
| FR-11 Import/Export/Backup | §6.13, §6.14 (signatures only) | Page 12, Migration doc | **Partially specified.** Migration doc IPC (`validate_import_file`, `execute_import`, `export_data`, templates, parties import, restore) not in LLD (D-17). CSV not in LLD (Excel only). |
| FR-12 Invoicing foundation | transaction_code VO, ShopSetting | Page 12 | Covered. ADJ codes share purchase table (D-06). |
| FR-13 Bundles | §6.6, TC-SRCH-004 | BundleCreationModal | Covered. No bundle update/delete IPC though repo has them; `units_per_bundle > 1` validation only in HLD comment. |
| FR-14 FTS5 | §4, TC-SRCH-* | ItemSearchTypeahead | Covered. HLD says triggers sync FTS; LLD uses app-level indexer (D-18, acceptable). Multi-word `blue gel` not in test matrix. |
| FR-15 Keyboard checkout | — | UI LLD §4, Page 2/3 | Flow differs: HLD 3-stop (qty→unit→commit), UI adds Rate stop; HLD "Enter on empty search completes sale" absent from UI LLD (D-20). Cash-received entered *after* commit, decoupled from `paid_amount` (D-21). |
| FR-16 MRP | entity fields | Page 2, 5 | Covered (attribute only). |
| FR-17 Soft delete | `archive` repos, §6.11 guard | — | Item archive guard unspecified (stock > 0? open lots?). Party guard = "open dues", HLD = "linked transactions" (soft-archive anyway, so OK). UI pages don't list Archive actions (D-22). |
| NFR-01 No server | §8, §8.1 | — | Covered. DB file is `book_keeper.db` in code vs `shop.db` in HLD; `BOOK_KEEPER_DB_PATH` override (HLD §8.2) not in LLD (D-23). |
| NFR-02 Latency | cached stock, single dashboard query | SWR §7.2 | Covered by design; no perf test other than TC-REP-001. |
| NFR-03 Immutability | implicit (no update/delete tx APIs) | — | Not stated as a rule in LLD; no DB-level guard (triggers) specified. |
| NFR-04 Typed migrations | `migrations.rs` | — | Covered structurally; no versioning scheme described. |
| NFR-05 Pluggable strategy | §6.2, §8.1 | — | Covered. But `restore_lots_for_return` and vendor-return depletion are FIFO-specific in semantics. |
| NFR-06 Keyboard ergonomics | — | UI LLD §4.1 | Covered, with hotkey drift (D-24). |

## 2. Discrepancies & design defects

### S1 — correctness risks
- **D-01 Upfront payment double-count.** LLD §6.7/§6.8 insert the header with `paid_amount`/`balance_due` already net of upfront cash **and** call `record_payment` for that upfront amount, which (per §6.4) decrements `balance_due` again. Separately, HLD cash-drawer formula = upfront cash + settlements; if upfront cash is also a `PAYMENT_SETTLEMENT` row, it is counted twice. The LLD must pick one representation. Also no field distinguishes an upfront row from a later settlement or a refund (HLD §6.2 #11 proposes `SALE_SETTLEMENT`/`PURCHASE_SETTLEMENT`; LLD uses `SALE`/`PURCHASE`).
- **D-02 Vendor CashRefund.** LLD §6.9 records vendor cash refund via `record_payment(PURCHASE)` — that reduces the purchase's `balance_due` (wrong for an already-paid bill, and rejected as overpayment if paid) and is classified as a *vendor settlement outflow* when it is actually a cash *inflow*.
- **D-03 Defective replacement.** `deplete_stock_for_sale(original sale_item_id, …)` adds allocations to the original sale line → COGS of the original period increases, and `SALE_ITEM.quantity` no longer matches its allocations. No `RETURN_ITEM` replacement line model.
- **D-04 Return quantity guard.** No rule that returned qty ≤ sold qty − already returned, or that `item_id` matches `sale_item_id`'s item, or that return value ≤ original bill. `ReduceDue` amount vs `balance_due` unchecked; `ReduceDue` with `original_sale_id = null` undefined.
- **D-05 Repeated partial returns.** Reverse-order restoration reads allocations only; without tracking already-restored qty per allocation, a second return re-restores lots that were already restored (lot remaining can exceed its original quantity).
- **D-06 Synthetic ADJ purchase.** Positive adjustments create `PURCHASE_TRANSACTION` rows: they appear in purchase history, period purchase counts, dashboard `today_purchase_count`, vendor-return lot pickers, and consume/compete with the `PUR-` code sequence.
- **D-07 Purchase overpayment.** §6.8 step 4: `balance_due = total − paid` with no clamp/validation → negative balance; status logic then yields PARTIAL.

### S2 — gaps / contradictions
- **D-08** Sale/purchase `payment_status` in the payload is ignored (derived from `paid_amount`); payload saying PAID with `paid_amount = 0` becomes UNPAID, and the party-required check runs on the *payload* status, so a walk-in credit sale can slip through.
- **D-09** HLD enum names disagree internally and with LLD: `return_type` (CUSTOMER vs CUSTOMER_RETURN), `settlement_type` (REFUND/DUE vs CASH_REFUND/REDUCE_DUE vs DEFECTIVE_REPLACEMENT), condition (SCRAP vs DAMAGED_SCRAP), adjustment reasons (ERD lacks THEFT_LOST; UI-UX Page 9 lacks THEFT_LOST).
- **D-10** FIFO "oldest" = lot id, not purchase date. With delayed entry (HLD assumption #4) a back-dated purchase entered later is treated as newest. Acceptable but undocumented.
- **D-11** Cart with the same item on two lines: per-line `validate_stock_availability` before any depletion can pass each line individually but fail in aggregate (protected only if depletion re-checks).
- **D-12** `StockAdjustmentPayload` has no `adjustment_date`, but UI Page 9 has an Adjustment Date field.
- **D-13** Positive-adjustment fallback `unit_cost = default_sale_price` overstates stock valuation and future COGS.
- **D-14** FR-09 asks for Day/Week/Month/Year; APIs take from/to (fine) but trend API is `7d|30d|1y` while UI toggle is Today/7D/Month/Year. Cash-flow and refund outflow derivation unspecified. Loss from write-offs (adjustment allocations) isn't in any report.
- **D-15** HLD `settle_bill_payment` supports specific-bill settlement; UI Pages 7/8 have `[Record Settlement]` per bill, but UI LLD SettlementModal is FIFO only.
- **D-16** `SalesHistoryFilter.search_query` (HLD) absent from repo `list_sales` signature; UI `getSalesHistory` returns `SaleSummaryResponse` which lacks date/party/item-summary columns needed by Page 7.
- **D-17** Import/export: migration doc defines dry-run validation, conflict strategies, parties import with opening balances, CSV, templates, restore-from-snapshot; LLD only has `export_items_to_excel`, `import_items_from_excel`, `export_transactions_to_excel`. Party opening balances have no target column/mechanism at all (PARTY has no balance field).
- **D-19** No IPC for: item lot inspector (Page 5), recent transactions feed (Page 1), party transaction history (Page 11; `get_party_ledger` exists), bundle edit/delete, list returns.
- **D-20** HLD FR-15 step 4 commits on Enter at the unit selector and completes sale on Enter-in-empty-search; UI LLD inserts a Rate stop and only documents Ctrl+Enter.
- **D-21** Summary dialog collects Cash Received *after* `create_sale` committed with `paid_amount` from the footer → change calculation is cosmetic; LLD's `change_amount` from backend is then unused or inconsistent.
- **D-22** Archive UX for items/parties not specified in UI docs; archived parties' visibility in dues lists undefined.
- **D-23** `shop.db` vs `book_keeper.db`; `BOOK_KEEPER_DB_PATH` env override not designed.
- **D-24** Hotkey drift: UI-UX `Alt+S` settings, `Ctrl+F` search, `Alt+C`/`Alt+V` party creation, `Ctrl+B` backup vs UI LLD registry.

### S3 — cosmetic
- **D-18** FTS sync: HLD says SQLite triggers; LLD indexes in application code after commit (per CHECKPOINT convention). Fine, but HLD should be updated.
- HLD §9.4 `BEGIN IMMEDIATE` list omits `create_vendor_return`; LLD §10.2 too.
- `create_item` returns `i64` in backend LLD vs `ItemSearchResult` in UI LLD ApiClient.
- Doc links point at `c:/custom_apps/...` paths.

## 3. Conclusion
The LLDs cover all 17 FRs and 6 NFRs at least nominally, and the architecture (MVCS, UoW, strategy pattern, single IPC case-conversion boundary) is consistent with the HLD. They are **not** fully sound: the money-flow model (D-01, D-02, D-07, D-08) and returns (D-03–D-05) have design-level defects, and FR-11 is under-specified relative to the migration doc. Review agents must check which interpretation the code actually took for each D-item rather than re-reporting the doc issue.
