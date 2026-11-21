# NFR-01 + NFR-04 Review: Embedded DB, DB path, typed forward-only migrations, and the SCHEMA DELTA

Reviewer scope: `migrations.rs`, `connection.rs`, `sqlite_uow.rs`, `main.rs`, `app_state.rs`, enum storage, schema delta vs aligned HLD §6.1/§6.2 + LLD §3.1/§5.3 + DD-01/05/06/07/11/12/20/22/23/25/28.
Read-only review; no builds run.

## 1. Scope traced
- `src-tauri/src/data/sqlite/migrations.rs` (217 lines), `connection.rs`, `sqlite_uow.rs`, `mod.rs` (enum_to_sql/enum_col), `repositories/*.rs` (column lists), `main.rs`, `app_state.rs`, `tauri.conf.json`
- `src-tauri/src/core/value_objects/*.rs`, `core/entities/*.rs`, `core/error.rs`; `src/types/index.ts`
- `tests/sqlite_smoke_test.rs`, `tests/common/mod.rs`, `tests/common/fixtures.rs`, `tests/migration_service_test.rs` (VedBill, unrelated to schema migration)
- Docs: DESIGN_DECISIONS.md (all), HLD NFR-01/NFR-04/§6/§8.2, LLD §3.1/§5.3/§10.2, `install.txt`, PROJECT.md/CHECKPOINT.md
- Cross-refs: FR-11 F11-09, NFR-03 F-02, FR-10, FR-08 F-02, FR-13 F-03

## 2. Acceptance and DD checklist

| Clause | Status | Evidence |
|---|---|---|
| NFR-01 embedded SQLite, no server | ✅ | `Cargo.toml` rusqlite `bundled`; `connection.rs:6-14` file-backed r2d2 pool |
| `core/` platform agnostic, no tauri/Win32 | ✅ | `rg tauri\|windows\|cfg(` over `src/core src/data src/services` finds nothing (only a `#[cfg(test)]`). `core/error.rs:47` imports `rusqlite::Error` (acceptable, not Win32/tauri). Only `main.rs:2,10` and `api/*` touch tauri. |
| Pragmas on every connection: FK ON, WAL, synchronous NORMAL, busy_timeout 5000 | ✅ (order caveat) | `connection.rs:7-12` via `with_init`, so every pooled connection gets them. See N-09 (ordering). |
| Pool | ⚠️ | `connection.rs:14` max_size 4, default min_idle (=max) so 4 connections open at startup; fine for desktop. |
| BEGIN IMMEDIATE once per mutating service call (LLD §10.2) | ✅ | `sqlite_uow.rs:26`; but also used for every read-only call (N-08) |
| Rollback on drop / commit failure | ✅ | `sqlite_uow.rs:62-67` (drop), `:73-79` (commit fail rolls back), `:81-85` |
| Numbered, forward-only, recorded in `schema_version` table, one txn each | ❌/⚠️ | `migrations.rs:201-217`: single hardcoded `if version < 1`; uses `PRAGMA user_version`, no `schema_version` table; the only txn is v1. |
| `ALTER TABLE ADD COLUMN`/new tables only, no JSON blobs | ✅ (today) / ❌ (design needs rebuilds) | No JSON/blob columns anywhere in `SCHEMA_V1`. The aligned design needs table rebuilds (dropping `bundle_id` FK, nullable `transaction_id`), which conflicts with NFR-04 wording "ADD COLUMN only"; see §5. |
| Enum columns TEXT with CHECK, SCREAMING_SNAKE | ❌ | No CHECKs (`migrations.rs`); no `rename_all` on any enum (`rg rename_all src` = 0 hits); stored values PascalCase (`mod.rs:16-17,39`). |
| DD-20 `shop.db` + `BOOK_KEEPER_DB_PATH` | ❌ | `main.rs:12` hardcodes `book_keeper.db`; no `std::env::var` anywhere in `src` (F11-09). |
| DD-22 atomicity | ⚠️ | UoW is correct; individual services' use is out of scope here (see FR-xx reports). |
| DD-01/05/06/07/11/12/23/25/28 schema | ❌ | See §5 Schema Delta (none are present). |
| Schema/migration/FK tests | ❌ | N-14 |

## 3. Findings

| ID | Severity | Kind | file:line | What's wrong | Concrete failure scenario | Change needed | Overlaps |
|---|---|---|---|---|---|---|---|
| N-01 | Critical | design-gap | `migrations.rs:4-199` | The schema is the pre-alignment v1. Missing: `payment_settlements.kind/return_id` and nullable `transaction_id`; `purchase_transactions.source/supplier_invoice_ref`; `sale_stock_allocations.returned_quantity`; `return_items.line_type/cost_amount`; table `stock_outflow_allocations` (instead of `adjustment_stock_allocations`); `pack_label/pack_quantity` on lines and no `bundle_id`; `stock_adjustments.unit_cost`; all CHECKs; item name UNIQUE; unique barcodes. Full per-table list in §5. | Cannot persist a REFUND_OUT/REFUND_IN or unlinked refund (transaction_id NOT NULL, `migrations.rs:160`); cannot record repeated partial returns (no returned_quantity, DD-05); cannot distinguish ADJ/INIT/IMP lots from real purchases so vendor payables and purchase reports are polluted (DD-07). Every fix-phase FR depends on this. | Apply §5 delta as one schema theme before service work. | DD-01/05/06/07/11/12/25/28; FR-01/02/03/06/07/08/09; NFR-03 F-02 |
| N-02 | High | design-gap/bug | `migrations.rs:201-217` | Migration runner is not a framework: one hardcoded step, `version` read **before** `BEGIN IMMEDIATE` (race between two app instances/tests), no step list, no `schema_version` row, `foreign_keys` is ON from `with_init` (`connection.rs:11`) and `PRAGMA foreign_keys` is a no-op inside a txn, so future table rebuilds (see §5) will cascade/fail the FK check on a pooled connection. `execute_batch("PRAGMA user_version = 1;")` ok but not recorded in a table as LLD §5.3 requires. | Add v2 as `if version < 2` and ship a rebuild of `sale_items`: with FK ON, `DROP TABLE sale_items` fails (`sale_stock_allocations`/`return_items` reference it) or, worse, silently succeeds when the child table is empty and breaks later. Two Tauri instances starting together both see version 0/1 and both run steps (second fails on `ALTER ADD COLUMN duplicate column`). | Ordered `MIGRATIONS: &[(u32,&str name,&str sql)]` list; for each pending step: open conn with FK OFF for rebuild steps, `BEGIN IMMEDIATE`, re-read version inside txn, run step, `PRAGMA foreign_key_check`, insert `schema_version(version,name,applied_at)`, set user_version, `COMMIT`; re-enable FK. Return `MigrationError` with step number. | LLD §5.3, NFR-04 |
| N-03 | High | design-gap | `migrations.rs:159-167`, `core/entities/settlement.rs:9` | `payment_settlements.transaction_id INTEGER NOT NULL` (and Rust `i64`) contradicts DD-01 (nullable for walk-in/unlinked refunds, lump-sum). No `kind`, no `return_id`; `transaction_type` is free TEXT with no CHECK. Making a column nullable requires a table rebuild in SQLite (no `ALTER COLUMN`). | Walk-in return refund (`REFUND_OUT`, no bill) has nowhere to store transaction_id: insert fails with NOT NULL constraint, or a placeholder `0` is written which then breaks the future `FOREIGN KEY` and ledger joins. | Rebuild `payment_settlements` per §5.11; entity -> `Option<i64>`; add CHECKs. | DD-01, DD-03, DD-04; FR-06, FR-07 |
| N-04 | High | design-gap | `migrations.rs:76,106,107,36` | `bundle_id INTEGER REFERENCES item_bundles(id)` on `purchase_items` and `sale_items` (DD-28 forbids). SQLite `ALTER TABLE DROP COLUMN` **fails** on a column that carries a FK constraint, so a rebuild is required for both tables; `bundle_quantity` must be renamed `pack_quantity`; `pack_label` backfilled from `item_bundles.bundle_name`. | With the FK in place and FK enforcement ON, `delete_bundle` (DD-28 "always allowed") fails with FOREIGN KEY constraint for any bundle ever used on a bill; editing multiplier retroactively changes the meaning of history. | Rebuild `sale_items`, `purchase_items` per §5.5/§5.8. | DD-28, FR-13 F-xx |
| N-05 | High | design-gap/contract | `core/value_objects/*.rs` (no `rename_all`), `data/sqlite/mod.rs:16-17,39-45,53-58`, `src/types/index.ts:11-29` | All enums serialize PascalCase and are stored as such; `SettlementType` has 5 legacy variants, `AdjustmentReason` 8 legacy variants, `ReturnType` has 4 (duplicated `Customer/CustomerReturn`). Missing enums entirely: `PaymentKind`, `TransactionType` (stored as raw `String` "SALE"/"PURCHASE", `settlement.rs:8`), `LotSource`, `OutflowSourceType`, `ReturnLineType`. CHECK constraints (LLD §5.3) can only be added after values are rewritten. | If `#[serde(rename_all="SCREAMING_SNAKE_CASE")]` is added without a data rewrite, every existing `payment_status='Paid'` row fails `enum_from_sql` (`mod.rs:48`) -> `DatabaseError("invalid enum value 'Paid'")` and the sales history page errors. Conversely adding CHECK(`IN ('PAID',...)`) first rejects the current `enum_to_sql` output "Paid". | Add `#[serde(rename_all = "SCREAMING_SNAKE_CASE")]` to all enums, delete legacy variants, add the five new enums, regenerate TS unions in `src/types/index.ts` and mock; rewrite stored values (table in §4) in the same migration step that adds CHECKs. | DD-23; FR-08 F-02, FR-10 |
| N-06 | Medium | bug | `data/sqlite/mod.rs:39-45` | `enum_to_sql` swallows serialization failure and non-string outputs: `Err(_) => String::new()`, non-string -> JSON text. An empty string would be written into an enum column silently. | Future enum with a data-carrying variant (or serde error) writes `''` into `payment_status`; today harmless, after CHECK constraints it surfaces as an opaque constraint error far from the cause. | Return `Result` or `panic!`/`debug_assert`; add a unit test that every enum round-trips to the SCREAMING_SNAKE literal used in the CHECK. | DD-23 |
| N-07 | Medium | design-gap | `main.rs:10-12`, `tauri.conf.json:5` | Default location/name: `app_data_dir()` is derived from identifier `com.bookkeeper.app` (Windows `%APPDATA%\com.bookkeeper.app`, Linux `~/.local/share/com.bookkeeper.app`), but HLD §8.2:629, `install.txt:104` and UI spec promise `%APPDATA%\book_keeper\shop.db` / `~/.local/share/book_keeper/shop.db`. No env override, no `create_dir_all` for an override's parent, no fallback when app_data_dir fails. Resolution is inline in `main.rs`, not testable. | User on WSL sets `BOOK_KEEPER_DB_PATH=/mnt/c/Users/x/AppData/Roaming/book_keeper/shop.db` (install.txt:106): ignored, app writes to another file; or path set but directory missing -> pool creation `unable to open database file`. Header shows `shop.db Connected` (`Header.tsx:124`) while the file is `book_keeper.db`. | Add `resolve_db_path(env: Option<OsString>, app_data: &Path) -> PathBuf` in `app_state.rs` (or a tiny `data/sqlite/db_path.rs`): env var (non-empty) else `app_data/shop.db`; `create_dir_all(parent)`; decide default dir (either change `identifier`/subdir to `book_keeper` or update docs). Unit-test with env injected (no `set_var`). | DD-20, FR-11 F11-09, HLD §8.2 |
| N-08 | Medium | bug (perf/robustness) | `sqlite_uow.rs:26`, `report_service_impl.rs:36,125,143` | Every `begin_transaction()` is `BEGIN IMMEDIATE`, including pure-read report/list services. In WAL mode readers should never block; here every dashboard/report/search-history call takes the single write lock and queues behind any writer (and behind other readers). | Dashboard polling (or a Reports query) while a long items import holds the write lock: the read waits up to `busy_timeout` 5 s then returns `SQLITE_BUSY` ("database is locked") instead of reading the WAL snapshot; with pool size 4, 4 pending reads block a fifth checkout. | Add `begin_read()`/`ReadContext` using `BEGIN` (deferred) for read-only services, or keep one UoW but a `read_only` flag; keep IMMEDIATE for writes (LLD §10.2 only mandates it for mutating methods). | NFR-02, LLD §10.2 |
| N-09 | Low | bug | `connection.rs:9-11` | `PRAGMA journal_mode=WAL` runs before `busy_timeout`. `r2d2` builds `min_idle = max_size = 4` connections concurrently at startup; the WAL switch needs an exclusive lock and with timeout still 0 can return `SQLITE_BUSY` immediately. | Startup with 4 concurrent init closures against a fresh file: one fails `database is locked`, `Pool::build` returns `pool creation failed`, app refuses to start intermittently (rare, more likely on slow disk/AV scanning). | Set `busy_timeout` first, then WAL. Also `PRAGMA foreign_keys` check: assert `PRAGMA foreign_keys` returns 1 in a test. | LLD §5.3 |
| N-10 | Medium | design-gap | `main.rs:12`, `app_state.rs:51`, `migrations.rs:201-217` | No guard rails around migration: (a) no pre-migration file backup of an existing DB, (b) no handling of `user_version` > latest (DB from a newer app version opens silently and old code writes into an unknown schema), (c) migration failure propagates via `?` in the Tauri `setup` closure -> app exits with a raw error string and no dialog, (d) no `PRAGMA foreign_key_check`/integrity check after rebuild steps. | After a failed v3 step on a shopkeeper's real DB the app will not start, with no safe copy to roll back to. Downgrading the exe re-opens a v4 DB with v3 code and corrupts invariants. | Before applying any pending step to a non-empty DB: copy `shop.db` -> `shop.db.bak-v{N}` (use `rusqlite::backup`, backup is already a dependency); refuse (`MigrationError`) when `user_version > LATEST`; surface setup errors through a dialog. | NFR-04 |
| N-11 | Medium | design-gap | `migrations.rs:14-27` etc. | Type/constraint drift vs HLD §6.2: `items.name` has no UNIQUE (HLD: NOT NULL, UNIQUE); `items.base_unit` no DEFAULT 'PCS'; `created_at` NOT NULL with no `DEFAULT CURRENT_TIMESTAMP` (all tables); `items.updated_at` and `parties.updated_at` exist in code but not in HLD (harmless; document it); `purchase_transactions.paid_amount/balance_due` have no DEFAULT 0.0; `payment_status` no CHECK; `party_type` no CHECK; `shop_settings` has no `CHECK(id=1)`; `item_bundles` has no `units_per_bundle > 1` CHECK; no `ON DELETE` clauses (none required). Money/qty `REAL`, dates `TEXT`, booleans `INTEGER` otherwise match HLD. | Two items named "Lux Soap" can be created (`item_service_impl` only checks barcode); search typeahead then returns two identical rows and import-by-name matches arbitrarily. | Add `CREATE UNIQUE INDEX idx_items_name_unique ON items(name)` after a duplicate-name pre-check (or case-insensitive `COLLATE NOCASE` decision), CHECKs per §5, defaults per HLD. Decide and document `updated_at`. | NFR-03 F-02, FR-13, FR-11 |
| N-12 | Medium | quality | `repositories/*.rs` (`COLS`/`TX_COLS`/`ITEM_COLS` + `VALUES (?1,...)` + `enum_col(r, N)`) | Every repository uses positional column lists and numeric indexes (e.g. `sale_repo.rs:11,21,98`; `return_repo.rs:10-11,18-39`; `settlement_repo.rs:9`). Adding columns or rebuilding tables in a different order breaks mapping silently (wrong `r.get(5)`), not at compile time. | Rebuilding `purchase_items` with `pack_label, pack_quantity` placed after `item_id` shifts every index; `unit_cost` is read from the `quantity` slot with no error because both are REAL. | Keep rebuilt column order identical to the entity/COLS order, or switch to named access (`r.get("payment_status")`), and add a repo round-trip test per table. | all repo reviews |
| N-13 | Low | design-gap | `migrations.rs:2,12` | `CREATE TABLE IF NOT EXISTS` inside a version-gated batch is fine for v1, but any dev DB that already exists at `user_version=1` in the old shape would be treated as "up to date" if v1 is rewritten in place (N-15). | Developer's local `book_keeper.db` stays at version 1 with the old shape; app starts, then first `INSERT ... kind` fails "no such column". | Detect legacy shape (see §6 recommendation). | NFR-04 |
| N-14 | High | test-gap | `tests/sqlite_smoke_test.rs`, `tests/common/*` | No test of: user_version/`schema_version` content, idempotent re-run of `run_migrations`, pragmas on pooled connections (FK=1, WAL, busy_timeout), FK violation rejected (`INSERT sale_items` with a missing item), every CHECK, enum literal round-trip, upgrade path from a frozen older schema, `BOOK_KEEPER_DB_PATH` resolution. `fixtures.rs:257-295` `ensure_*` create ghost parent rows to *satisfy* FKs, so FK enforcement is only implicitly exercised. `migration_service_test.rs` is VedBill import only. | A refactor that drops `foreign_keys=ON` from `with_init` (or runs migration on a non-init connection) passes all 59 tests. | Add `tests/schema_migration_test.rs` (IDs in §7). | LLD §11 |
| N-15 | Medium | design-gap | `main.rs:12` | Rename `book_keeper.db` -> `shop.db` orphans any dev data (shop settings, items). No import/rename path. | Developer upgrades, launches, sees an empty shop. | Pre-release: document "delete old file". Else on first start, if `shop.db` absent and `book_keeper.db` present in the same dir, move it (one-off) before opening. | FR-11 F11-09 |
| N-16 | Low | quality | `migrations.rs:12` | Seed `INSERT OR IGNORE INTO shop_settings (id, shop_name) VALUES (1,'My Shop')` lives in the DDL string; `setting_repo.rs:42` upserts id=1. No `CHECK (id = 1)`. | A `create` with id=2 gives two profile rows; `get` ambiguity. | `CHECK (id = 1)`. | FR-12 |

Severity counts: Critical 1, High 5 (N-02, N-03, N-04, N-05, N-14), Medium 7 (N-06, N-07, N-08, N-10, N-11, N-12, N-15), Low 3 (N-09, N-13, N-16).

Note the strings in N-07 `install.txt:104` path vs `identifier`: HLD says `book_keeper`; one of docs/identifier must change, flagged for decision.

## 4. Enum <-> TEXT storage

Rust: all value objects `#[derive(Serialize, Deserialize)]` with NO `rename_all` (`core/value_objects/*.rs`); storage goes through serde JSON (`mod.rs:39,48,53`), so the DB literal equals the IPC literal, and the TS unions (`src/types/index.ts:11-29`) mirror PascalCase.

| Table.column | Rust enum (current variants) | Currently stored | Target (LLD §3.1 / DD-23) | Rewrite rule for existing rows |
|---|---|---|---|---|
| parties.party_type | PartyType {Customer, Vendor} | `Customer`/`Vendor` | `CUSTOMER`/`VENDOR` | `UPPER(party_type)` |
| sale_transactions.payment_status, purchase_transactions.payment_status | PaymentStatus {Paid, Partial, Unpaid} | `Paid`/`Partial`/`Unpaid` | `PAID`/`PARTIAL`/`UNPAID` | `UPPER()` |
| return_transactions.return_type | ReturnType {Customer, Vendor, CustomerReturn, VendorReturn} | any of 4 | `CUSTOMER`/`VENDOR` | `CustomerReturn`/`Customer` -> `CUSTOMER`; `VendorReturn`/`Vendor` -> `VENDOR` |
| return_transactions.settlement_type | SettlementType {CashRefund, ReduceDue, DefectiveReplacement, SpecificBill, FifoDues} | those 5 | `REFUND`/`REPLACEMENT` | CashRefund, ReduceDue, SpecificBill, FifoDues -> `REFUND`; DefectiveReplacement -> `REPLACEMENT` (the dues-vs-cash distinction moves to ledger `kind`) |
| return_items.condition | StockCondition {Restockable, DamagedScrap} | `Restockable`/`DamagedScrap` | `RESTOCKABLE`/`DAMAGED_SCRAP` | CASE map |
| stock_adjustments.reason | AdjustmentReason {AuditCorrection, DamageWriteOff, Expiry, Theft, FoundStock, InitialSetup, Damaged, Expired} | 8 legacy | `DAMAGED`/`EXPIRED`/`THEFT_LOST`/`COUNT_CORRECTION` | AuditCorrection, FoundStock, InitialSetup -> `COUNT_CORRECTION`; DamageWriteOff, Damaged -> `DAMAGED`; Expiry, Expired -> `EXPIRED`; Theft -> `THEFT_LOST` (lossy, acceptable) |
| payment_settlements.transaction_type | none (raw `String`) | `SALE`/`PURCHASE` (already uppercase) | `TransactionType` enum `SALE`/`PURCHASE` | none, add CHECK |
| payment_settlements.kind | missing | n/a | `PaymentKind` | backfill, §5.11 |
| purchase_transactions.source | missing | n/a | `LotSource` | backfill, §5.4 |
| return_items.line_type | missing | n/a | `ReturnLineType` | all existing = `RETURNED` |
| stock_outflow_allocations.source_type | missing | n/a | `OutflowSourceType` | existing adjustment rows -> `ADJUSTMENT` |
| payment_mode (3 tables) | `Option<String>`, free text | NULL/free | nullable free TEXT (v1 NULL) | none |
| base_unit | free String | e.g. `pcs` | free TEXT (HLD examples uppercase) | optional `UPPER()`; decide (smoke test uses `"pcs"`) |

## 5. Schema Delta (primary input for the fix phase)

Conventions: "In place" = ADD COLUMN/RENAME COLUMN/CREATE INDEX/CREATE TABLE/DROP TABLE; "Rebuild" = create `_new`, `INSERT ... SELECT`, drop old, rename (FK pragma OFF, run `PRAGMA foreign_key_check`). SQLite bundled with rusqlite 0.31 is 3.45, so RENAME COLUMN and DROP COLUMN (only for non-FK, non-indexed, non-CHECK columns) are available. Adding a column with a CHECK via `ALTER ADD COLUMN ... CHECK` is allowed but cannot add table-level CHECKs to existing columns (needs rebuild).

Column order below is the proposed order for the rebuilt/new definitions (keep entity field order identical, see N-12).

### 5.1 shop_settings
| Current | Target | Change | SQLite | Backfill |
|---|---|---|---|---|
| `id PK AUTOINCREMENT, shop_name NOT NULL, phone, address, tax_id` | same, plus `CHECK (id = 1)` | ADD CHECK | Rebuild (or skip, enforce in repo) | none; keep seed row |

### 5.2 items
| Current | Target | Change | SQLite | Backfill |
|---|---|---|---|---|
| `name TEXT NOT NULL` | `name TEXT NOT NULL UNIQUE` | ADD UNIQUE | `CREATE UNIQUE INDEX idx_items_name_unique ON items(name)` in place | pre-check duplicates; if present, suffix `name || ' (' || id || ')'` |
| `barcode TEXT` + non-unique `idx_items_barcode` | unique among non-null: `CREATE UNIQUE INDEX ... ON items(barcode) WHERE barcode IS NOT NULL` | ADD UNIQUE (DROP+CREATE INDEX) | In place | dedupe: NULL out later duplicates, log them. Items-vs-bundle barcode collision cannot be an index; enforce in service (DD-24 forbids triggers for FTS only, but keep app-level). |
| `base_unit TEXT NOT NULL` | `... DEFAULT 'PCS'` | default | needs rebuild (cosmetic, skip; services always supply) | n/a |
| `created_at NOT NULL` | `... DEFAULT CURRENT_TIMESTAMP` | default | skip (code always supplies RFC3339; note CURRENT_TIMESTAMP format differs) | n/a |
| `updated_at` (not in HLD) | keep, document | n/a | n/a | n/a |
| `current_stock REAL NOT NULL DEFAULT 0` | add `CHECK (current_stock >= -1e-9)` optional; `is_low_stock IN (0,1)`, `is_active IN (0,1)` | ADD CHECK | Rebuild (batch with other rebuilds, or defer) | none |
Indexes `idx_items_name`, `idx_items_category` keep (name index is subsumed by the UNIQUE index: DROP `idx_items_name`).

### 5.3 item_bundles
| Current | Target | Change | SQLite | Backfill |
|---|---|---|---|---|
| `units_per_bundle REAL NOT NULL` | add `CHECK (units_per_bundle > 1)` | ADD CHECK | Rebuild | existing rows with <=1 must be fixed or the INSERT..SELECT fails; pre-check and abort with listing |
| `barcode` non-unique index | unique where not null | ADD UNIQUE | In place (partial unique index) | dedupe as items |
| `item_id REFERENCES items(id)` | unchanged; bills no longer reference bundles (DD-28), so delete is unconstrained | n/a | n/a | n/a |

### 5.4 purchase_transactions
| Current | Target | Change | SQLite | Backfill |
|---|---|---|---|---|
| (none) | `source TEXT NOT NULL DEFAULT 'PURCHASE' CHECK (source IN ('PURCHASE','ADJUSTMENT','OPENING','RETURN'))` | ADD COLUMN | In place (`ADD COLUMN` with CHECK + constant default is allowed) | by `transaction_code` prefix: `INIT-%` -> `OPENING`; `PUR-OPEN-%` -> `OPENING`; `IMP-%` -> `OPENING` (DD-16: import opening stock is OPENING); `ADJ-%` -> `ADJUSTMENT`; everything else `PURCHASE`. Also normalise these non-PURCHASE headers to `total_amount=0, party_id=NULL, payment_status='PAID', paid_amount=0, balance_due=0` (DD-07) and re-prefix `INIT-/IMP-/PUR-OPEN-` to `OPN-n` sequences. Code sources: `item_service_impl.rs:92` (INIT-), `import_export_service_impl.rs:251` (IMP-), `migration_service_impl.rs:199` (PUR-OPEN-, deferred), `inventory_service_impl.rs:178` (ADJ-). Note: INIT-/IMP- headers currently may carry a non-zero total and `UNPAID/PAID`; check, because vendor payables sum `balance_due` today. |
| (none) | `supplier_invoice_ref TEXT` | ADD COLUMN | In place | where `notes LIKE 'Supplier Invoice%'` (check actual folding in `purchase_service_impl`) move to column and strip from notes; else NULL |
| `payment_status TEXT NOT NULL` | `CHECK (payment_status IN ('PAID','PARTIAL','UNPAID'))`; values upper-cased | ADD CHECK + enum rewrite | Rebuild (CHECK); rewrite UPDATE can run first in place | `UPPER()` |
| `paid_amount/balance_due NOT NULL` | `DEFAULT 0.0` | default | Rebuild (optional) | none |
| DD-01 semantics | header created with paid=0, balance=total; paid/balance are ledger-derived | DATA BACKFILL | n/a | For existing rows: insert a `UPFRONT` ledger row = `paid_amount` for every header with `paid_amount>0` and no ledger rows (the old code wrote upfront via `paid_amount` and possibly one settlement row; check `payment_service_impl` for double counting before inserting), then verify invariant `paid = Σ(UPFRONT+SETTLEMENT)` |
| index | add `idx_purchase_tx_source_date ON (source, transaction_date)`; keep `idx_purchase_tx_party`; `idx_purchase_tx_date` can be dropped | ADD INDEX | In place | n/a |

### 5.5 purchase_items
| Current | Target | Change | SQLite | Backfill |
|---|---|---|---|---|
| `bundle_id INTEGER REFERENCES item_bundles(id)` | removed | DROP COLUMN (FK-bearing) | **Rebuild** (DROP COLUMN refuses FK columns) | n/a |
| `bundle_quantity REAL NOT NULL` | `pack_quantity REAL NOT NULL` | RENAME | rename in place (`RENAME COLUMN`) or fold into rebuild | value kept (when bundle_id NULL, `bundle_quantity` should equal `quantity`; verify) |
| (none) | `pack_label TEXT` | ADD COLUMN | in place | `(SELECT bundle_name FROM item_bundles b WHERE b.id = bundle_id)` before dropping `bundle_id`; NULL if no bundle |
| `remaining_quantity REAL NOT NULL` | add `CHECK (remaining_quantity >= 0 AND remaining_quantity <= quantity)` | ADD CHECK | Rebuild (same one) | existing violations (e.g. float drift above quantity after returns) must be clamped before copy; abort if negative |
| index | keep `idx_purchase_items_item(item_id, remaining_quantity)`, `idx_purchase_items_tx`; recreate after rebuild | n/a | n/a | n/a |

### 5.6 sale_transactions
| Current | Target | Change | SQLite | Backfill |
|---|---|---|---|---|
| `payment_status NOT NULL` | CHECK IN ('PAID','PARTIAL','UNPAID'), upper-case | ADD CHECK + enum rewrite | Rebuild for CHECK | `UPPER()` |
| `paid_amount/balance_due` | `DEFAULT 0.0` | default | optional | none |
| DD-01 | ledger-derived | DATA BACKFILL | n/a | same UPFRONT backfill as 5.4 for `paid_amount>0` rows without ledger rows; reconcile with existing `payment_settlements` rows (those are SETTLEMENT kind, §5.11) |
| indexes | keep date and party | n/a | n/a | n/a |

### 5.7 (index of sale_items is in 5.8)

### 5.8 sale_items
| Current | Target | Change | SQLite | Backfill |
|---|---|---|---|---|
| `bundle_id` FK | removed | DROP COLUMN | **Rebuild** | n/a |
| `bundle_quantity` | `pack_quantity` | RENAME | in place or in rebuild | as 5.5 |
| (none) | `pack_label TEXT` | ADD COLUMN | in place | from `item_bundles.bundle_name` via `bundle_id` |
| rest unchanged (`quantity`, `unit_price`, `total_amount`) | n/a | n/a | n/a | n/a |
Rebuild note: `sale_stock_allocations.sale_item_id` and `return_items.sale_item_id` reference this table, so the rebuild must run with FK OFF and preserve ids (`INSERT ... SELECT id,...`), then `foreign_key_check`.

### 5.9 sale_stock_allocations
| Current | Target | Change | SQLite | Backfill |
|---|---|---|---|---|
| (none) | `returned_quantity REAL NOT NULL DEFAULT 0 CHECK (returned_quantity >= 0 AND returned_quantity <= quantity_depleted)` | ADD COLUMN (with CHECK referencing another column is allowed in ADD COLUMN? Column CHECKs may reference other columns; safe, but test on 3.45) | In place | **Not derivable exactly.** Existing customer returns restored `purchase_items.remaining_quantity` without logging per-allocation. Backfill rule: 0 for all rows; then recompute from `return_items` (sale_item_id set, condition RESTOCKABLE or DAMAGED_SCRAP) by walking allocations in reverse id order up to `return_items.quantity`, consistent with DD-05. Pre-release this is moot (see §6). |
| indexes | keep both; add composite only if needed | n/a | n/a | n/a |

### 5.10 return_transactions / return_items
| Table | Current | Target | Change | SQLite | Backfill |
|---|---|---|---|---|---|
| return_transactions | `return_type`, `settlement_type` TEXT NOT NULL, `original_transaction_id INTEGER` (no FK) | CHECKs: `return_type IN ('CUSTOMER','VENDOR')`, `settlement_type IN ('REFUND','REPLACEMENT')`, plus `CHECK (NOT (return_type='VENDOR' AND settlement_type='REPLACEMENT'))` | ADD CHECK + enum rewrite | Rebuild | CASE map from §4; `original_transaction_id` has no FK by design (polymorphic sale/purchase), keep and document |
| return_transactions | code `return_code` UNIQUE, prefix | HLD says `RET-`; DD-07 adds `RTL-` only for RETURN lot headers | none | n/a | n/a |
| return_items | (none) | `line_type TEXT NOT NULL DEFAULT 'RETURNED' CHECK (line_type IN ('RETURNED','REPLACEMENT_ISSUED'))` | ADD COLUMN | In place | all existing = `RETURNED` |
| return_items | (none) | `cost_amount REAL NOT NULL DEFAULT 0` | ADD COLUMN | In place | existing customer RESTOCKABLE/DAMAGED lines: compute `quantity * unit_cost` of the allocations (`sale_stock_allocations` for that `sale_item_id`, reverse-id walk); if not derivable, 0 and document (affects Inventory Losses, DD-13) |
| return_items | `condition TEXT NOT NULL` | `CHECK (condition IN ('RESTOCKABLE','DAMAGED_SCRAP'))` but `REPLACEMENT_ISSUED` lines and vendor lines need a value: make `condition` NOT NULL with default `'RESTOCKABLE'` or nullable for issued lines (decide; HLD says "for RETURNED customer lines") | ADD CHECK | Rebuild (also NOT NULL change if made nullable) | upper-case rewrite |
| return_items | `sale_item_id`, `purchase_item_id` FKs exist | add indexes `return_item(sale_item_id)`, `return_item(purchase_item_id)` | ADD INDEX | In place | n/a |
| return_items | `unit_price` | `unit_price` is 0 for issued lines (HLD) | none | n/a | n/a |

### 5.11 payment_settlements (ledger)
| Current | Target | Change | SQLite | Backfill |
|---|---|---|---|---|
| `transaction_id INTEGER NOT NULL` | `transaction_id INTEGER` nullable; with `CHECK (kind IN ('REFUND_OUT','REFUND_IN') OR transaction_id IS NOT NULL)` | DROP NOT NULL + ADD CHECK | **Rebuild** | copy as is |
| (none) | `kind TEXT NOT NULL CHECK (kind IN ('UPFRONT','SETTLEMENT','RETURN_CREDIT','REFUND_OUT','REFUND_IN'))` | ADD COLUMN | In place possible only with constant DEFAULT, so do it in the rebuild | existing rows -> `SETTLEMENT` (old code only wrote `settle_bill_payment` rows and, per FR-06/FR-07 reports, possibly return refunds); verify with `payment_service_impl` which paths wrote rows. Rows that were upfront payments written as settlements (if any) -> `UPFRONT`. |
| (none) | `return_id INTEGER REFERENCES return_transactions(id)` | ADD COLUMN | in place (FK column w/ NULL default is allowed) | NULL |
| `transaction_type TEXT NOT NULL` | `CHECK (transaction_type IN ('SALE','PURCHASE'))` | ADD CHECK | rebuild | none |
| `amount_paid REAL NOT NULL` | `CHECK (amount_paid > 0)` | ADD CHECK | rebuild | existing zero/negative rows (old refunds may have been stored negative) must be reclassified: negative -> `REFUND_*` kind with `ABS()` |
| `transaction_id` no FK (polymorphic) | keep polymorphic, no FK, index `(transaction_type, transaction_id)` | none | none | none |
| indexes `idx_settlements_party/tx/date` | keep; add `(return_id)` optional | n/a | n/a | n/a |

### 5.12 stock_adjustments
| Current | Target | Change | SQLite | Backfill |
|---|---|---|---|---|
| (none) | `unit_cost REAL` (positive adjustments only) | ADD COLUMN | In place | NULL (or lot cost from the lot row if derivable; not needed) |
| `quantity REAL NOT NULL` | `CHECK (quantity <> 0)` | ADD CHECK | Rebuild | existing zero rows (if any) must be removed or the copy fails |
| `reason TEXT NOT NULL` | `CHECK (reason IN ('DAMAGED','EXPIRED','THEFT_LOST','COUNT_CORRECTION'))` | ADD CHECK + enum rewrite | Rebuild | map in §4 |

### 5.13 adjustment_stock_allocations -> stock_outflow_allocations (DD-11)
| Current | Target | Change | SQLite | Backfill |
|---|---|---|---|---|
| `adjustment_stock_allocations(id, stock_adjustment_id FK, purchase_item_id FK, quantity_depleted, unit_cost)` | `stock_outflow_allocations(id, source_type TEXT NOT NULL CHECK IN ('ADJUSTMENT','REPLACEMENT','VENDOR_RETURN'), source_id INTEGER NOT NULL, purchase_item_id INTEGER NOT NULL REFERENCES purchase_items(id), quantity_depleted REAL NOT NULL, unit_cost REAL NOT NULL)` + index `(source_type, source_id)` + index `(purchase_item_id)` | NEW TABLE + DATA BACKFILL + DROP TABLE | In place (`CREATE`, `INSERT SELECT`, `DROP`) | `INSERT INTO stock_outflow_allocations(source_type, source_id, purchase_item_id, quantity_depleted, unit_cost) SELECT 'ADJUSTMENT', stock_adjustment_id, purchase_item_id, quantity_depleted, unit_cost FROM adjustment_stock_allocations ORDER BY id;` then DROP old table and its two indexes. `source_id` is polymorphic (no FK). Vendor returns today deplete lots without any allocation rows (N/A, nothing to backfill; flag in FR-07). |

### 5.14 parties
| Current | Target | Change | SQLite | Backfill |
|---|---|---|---|---|
| `party_type TEXT NOT NULL` | `CHECK (party_type IN ('CUSTOMER','VENDOR'))` | ADD CHECK + rewrite | Rebuild | `UPPER()` |
| `updated_at` | keep (document) | n/a | n/a | n/a |

### 5.15 item_search_fts (virtual)
No schema change. DD-24/FR-14: bundle-barcode rows are extra rows in the same `barcode` column (`item_id UNINDEXED`), reindexed by app code. Do not recreate it in a rebuild step; if `items` is rebuilt, FTS is unaffected (no FK).

### 5.16 schema_version (NEW)
`CREATE TABLE schema_version (version INTEGER PRIMARY KEY, name TEXT NOT NULL, applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP);` plus keep `PRAGMA user_version` as a fast guard (both are written in the same transaction). In place.

### 5.17 Summary matrix (what needs a rebuild)
Rebuild required: `purchase_items`, `sale_items` (drop FK column), `payment_settlements` (nullable column), and, only to add CHECKs on existing columns, `purchase_transactions`, `sale_transactions`, `return_transactions`, `return_items`, `stock_adjustments`, `item_bundles`, `parties`, `items`, `shop_settings`. In place: `source`, `supplier_invoice_ref`, `pack_label`, `returned_quantity`, `line_type`, `cost_amount`, `unit_cost`, `return_id`, new tables, all indexes, enum value rewrites (UPDATE).
Because nearly every table is touched by CHECK additions, an in-place incremental path means rebuilding essentially the whole schema anyway.

## 6. Migration plan and schema strategy

### Is there a real user DB?
No evidence of one: PROJECT.md is titled "Production Readiness", CHECKPOINT.md lists M1-M6 as done only against test DBs ("59 backend tests on real SQLite"), `git` history shows no release tag or installer, and the DB file name itself is about to change (DD-20: `book_keeper.db` -> `shop.db`), which already orphans any developer database. All 14 existing tables would need rebuilds or enum rewrites, and several backfills (returned_quantity, cost_amount, ledger kind, upfront rows) are lossy or guesses.

### Trade-off
- Clean v1 rewrite (recommended while pre-release): one authoritative `SCHEMA_V1` DDL equal to the aligned HLD (with all CHECKs, uniques, indexes, `schema_version` table), no data migration code, no lossy backfills, simplest to test. Cost: any existing dev DB is unusable; must be detected and refused with a clear message (or deleted).
- Incremental v2..v5: preserves the (probably empty) dev DB and proves the migration framework, but costs five rebuild steps with guessed backfills, more tests, and ends with the same schema.

### Recommendation (3 lines)
1. Rewrite `SCHEMA_V1` in place to the aligned design now (§5 target definitions), with the new migration runner (N-02) and `schema_version`; the DB file is renamed `shop.db` so old files are never opened by accident.
2. Make `run_migrations` detect a legacy dev DB (`user_version = 1` and `payment_settlements` lacks `kind`) and fail with `MigrationError("legacy development database; delete or back up and recreate")` rather than limp on (N-13).
3. Build the framework and a frozen-fixture upgrade test now so that the first post-release change is a real `v2` (ADD COLUMN or rebuild) using the same runner; if the team learns a real user DB exists, use the plan below instead.

### Fallback incremental plan (only if a real v1 DB must be preserved)
Each step: pre-migration backup copy (N-10), FK OFF, `BEGIN IMMEDIATE`, re-check version, apply, `foreign_key_check`, record `schema_version`, COMMIT.
1. **v2 "ledger and lot source"** (money + lots): add `schema_version`; `purchase_transactions.source/supplier_invoice_ref` + backfill by code prefix + normalise non-PURCHASE headers; rebuild `payment_settlements` (nullable `transaction_id`, `kind`, `return_id`, CHECKs); backfill `kind`; insert UPFRONT rows for existing headers; verify ledger invariants.
2. **v3 "bills decoupled from bundles"**: rebuild `purchase_items` and `sale_items` (`pack_label`, `pack_quantity`, drop `bundle_id`); `foreign_key_check`.
3. **v4 "returns and outflow"**: `sale_stock_allocations.returned_quantity`, `return_items.line_type/cost_amount`, new `stock_outflow_allocations` + backfill + drop old, `stock_adjustments.unit_cost`.
4. **v5 "enums and constraints"**: rewrite all enum values (§4), then rebuild header/line tables to add CHECKs, `CHECK (id=1)`, units CHECK; add unique name/barcode indexes (after dedupe), the §5.3 index list.
Order is forced by dependencies: ledger/source before service changes; enum rewrite before CHECKs; Rust serde rename must ship in the same release as v5 (N-05).

Whichever path: ship the serde `SCREAMING_SNAKE_CASE`, TS unions, mock data and repo column lists in the same commit as the schema.

## 7. Test coverage (what exists, what to add)

Exists: `sqlite_smoke_test.rs` (2 tests, builds app state against a temp file, i.e. implicit migration run), `tests/common/mod.rs` `TestDb` (fresh DB + `run_migrations` per test), FTS tests, VedBill `migration_service_test.rs` (unrelated). `fixtures.rs:257-295` seeds parent rows so FK-ON inserts succeed.

Missing (add `tests/schema_migration_test.rs`; suggested IDs, no LLD TC exists for NFR-01/04):
- TC-SCH-001 fresh DB: `user_version == LATEST`, `schema_version` has one row per step with names, all expected tables/indexes exist (`sqlite_master`).
- TC-SCH-002 `run_migrations` twice is a no-op (no error, no duplicate `schema_version` rows, seed row count 1).
- TC-SCH-003 pragmas on every pooled connection: get 4 connections, each `PRAGMA foreign_keys=1`, `journal_mode=wal`, `synchronous=1`, `busy_timeout=5000`.
- TC-SCH-004 FK enforcement: insert `sale_items` with missing `item_id` fails; deleting a lot referenced by an allocation fails.
- TC-SCH-005 one test per CHECK (amount_paid>0, kind/transaction_id rule, remaining<=quantity, returned<=depleted, units_per_bundle>1, quantity<>0, each enum IN-list, id=1).
- TC-SCH-006 UNIQUE: duplicate item name, duplicate item barcode, duplicate bundle barcode rejected; multiple NULL barcodes allowed.
- TC-SCH-007 enum round-trip: for every enum variant `enum_to_sql` equals the CHECK literal (guards N-05/N-06) and `enum_from_sql` parses it.
- TC-SCH-008 upgrade path (only if incremental): frozen old-v1 SQL fixture (`tests/fixtures/schema_v1_legacy.sql`) populated with representative rows (Paid sale, INIT-/ADJ-/IMP- lots, adjustment allocations, bundle lines) -> `run_migrations` -> assert enum rewrites, `source` backfill, `pack_label`, allocation copy, invariants, `foreign_key_check` empty.
- TC-SCH-009 failure rollback: inject a failing step; assert `user_version` and tables unchanged and the error is `MigrationError`.
- TC-SCH-010 downgrade guard: DB with `user_version = LATEST+1` -> `MigrationError`.
- TC-SCH-011 legacy-shape detection (clean-rewrite path).
- TC-SCH-012 `UnitOfWork` drop without commit rolls back; commit failure rolls back; reads do not block while a writer is open (after N-08).
- TC-PATH-001..003 `resolve_db_path`: env set, env empty, default `shop.db`; parent directory created; relative path rejected or canonicalised.

## 8. Schema and contract delta summary for this requirement
- New columns: `purchase_transactions.source`, `.supplier_invoice_ref`; `purchase_items`/`sale_items.pack_label` (+ `pack_quantity` rename); `sale_stock_allocations.returned_quantity`; `return_items.line_type`, `.cost_amount`; `payment_settlements.kind`, `.return_id` (+ nullable `transaction_id`); `stock_adjustments.unit_cost`.
- New tables: `schema_version`, `stock_outflow_allocations` (replaces `adjustment_stock_allocations`).
- Removed: `purchase_items.bundle_id`, `sale_items.bundle_id`, `adjustment_stock_allocations`.
- New/changed constraints: all CHECKs of LLD §5.3, unique item name, unique barcodes (items and bundles), `CHECK (id=1)` on shop_settings, §5.3 index list.
- Rust: `rename_all = "SCREAMING_SNAKE_CASE"` on all enums; new enums `PaymentKind`, `TransactionType`, `LotSource`, `OutflowSourceType`, `ReturnLineType`; trimmed `SettlementType`, `ReturnType`, `AdjustmentReason`; entity fields for every new column; `PaymentSettlement.transaction_id: Option<i64>`; named `resolve_db_path`.
- TS/mocks: regenerate the union types in `src/types/index.ts:11-29` and mock seed data (`src/api/mock/seedData.ts`, `mockClient.ts`) with the SCREAMING_SNAKE literals.
- IPC: none specific; startup error surfacing for migration failures.

## 9. Cross-requirement issues spotted
- FR-06/FR-07: settlement and refund paths currently write no ledger `kind`; ledger backfill assumptions (5.11) need those reviews' conclusions (which code paths wrote `payment_settlements` rows, whether `paid_amount` is also written directly, DD-01 double count risk).
- FR-07/FR-08: vendor returns deplete lots without any allocation rows today; `stock_outflow_allocations` `VENDOR_RETURN` has no historical data to backfill.
- FR-11: backup service names/overwrites files and uses its own connections; consider using it for the pre-migration backup (N-10) after F11-10 is fixed.
- FR-12: code sequences per source (`OPN-`, `RTL-`) and legacy `INIT-/IMP-/PUR-OPEN-` prefixes; sequence derivation must be consistent with the source column.
- FR-13: barcode uniqueness across items and bundles cannot be done with one index; service-level check required.
- UI/UX: Header shows `shop.db` regardless of the real file name (`Header.tsx:124`); show the resolved path from a backend command instead.
