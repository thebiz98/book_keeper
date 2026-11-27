# Plan: Scan a purchase bill into a pre-filled New Purchase (Gemini-compatible endpoint)

## Context
The owner wants to upload a photo of a vendor bill (typed or handwritten) and get a pre-filled New Purchase page to check and record. Decisions (2026-11-28):
- **The app builds the whole request:** the prompt, the JSON response schema and the image.
- **The VM is a thin wrapper over the Gemini API.** It holds the owner's Google key and forwards the request.
- **Other shops can set their own endpoint and key**, including Google's own Gemini URL with their own key directly. No VM is needed then.
- **Configured in Settings, not a file.** A shop running only the exe has no editable config, because `app.config.json` is compiled into the binary.
- **Hidden for now** behind the compiled flag `billScan.enabled: false`, until the VM is ready.

### Why one request shape covers both the VM and direct Gemini
The app always sends a standard Gemini `generateContent` body. That body has:
- `contents.parts`: the prompt text plus `inline_data` holding the image as base64.
- `generationConfig.responseMimeType = "application/json"` and `responseSchema`.

The app sends the key in the `x-goog-api-key` header.
- **Direct:** endpoint = `https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-flash:generateContent`, key = the shop's Gemini key.
- **VM:** endpoint = `https://<vm>/scan`, key = an app token the VM checks. The VM swaps in the real Google key and forwards the body unchanged.

There's one code path and one parser: read `candidates[0].content.parts[0].text` and parse it as the schema JSON. The VM then has only a few lines of work, and the contract doc tells the owner exactly what it should do.

**Privacy note:** on Gemini's free tier, Google may use the data it receives to improve its products. Vendor bills hold prices and vendor details. The Settings section will say so, so a shop can choose a paid key.

## Where settings live
- **Endpoint URL, model and enabled state:** in the database, in new `shop_settings` columns. This follows the `backup_directory` precedent: migration v13, `SettingRepository` getters/setters, and `src-tauri/src/data/sqlite/repositories/setting_repo.rs`.
- **API key:** in Windows Credential Manager via the `keyring` crate. It never goes in `shop.db`, so it doesn't leak into backups or restores, and it never goes to the frontend. Settings shows "Key saved ✓" with buttons to Replace and Remove.
- **Compiled `app.config.json` keeps a feature gate only:** `billScan { enabled, timeoutSecs, maxImageMb }`. While `enabled` is false, both the Settings section and the Scan button are hidden, and the backend refuses the command.

## Backend (Rust)
1. **Config gate** in `src-tauri/src/config.rs`: a `BillScanConfig` with `#[serde(default)]`, default off. Expose `billScanEnabled` and `billScanConfigured` (true when a URL and key are both saved) on `AppConfigView` in `src-tauri/src/api/settings_controller.rs`.
2. **Migration v13** (`src-tauri/src/data/sqlite/migrations.rs`): add `ai_endpoint_url TEXT` to `shop_settings`. Add the repo methods `get/set_ai_endpoint`.
3. **Key store:** a `SecretStore` trait (`get/set/delete`). The production version uses `keyring` (service `com.bookkeeper.app`, entry `ai_api_key`); the test version is in-memory.
4. **Dependencies:**
   - `reqwest` with `default-features = false, features = ["json", "rustls-tls"]`.
   - `base64`.
   - `keyring` with Windows native support.
   - Note all three in `docs/DEPENDENCY_VULNERABILITIES.md`.
5. **Prompt and schema** (`src-tauri/src/services/impl/bill_scan/prompt.rs`): a versioned constant prompt for Indian purchase bills (handwritten or typed, ₹, Hindi/English, qty/unit/rate/amount, freight/discount lines, "return null rather than guess"), plus the `responseSchema` JSON:
   - `vendor {name, phone, gstin}`
   - `invoiceRef`, `date` (YYYY-MM-DD)
   - `lines[] {name, quantity, unit, rate, amount}`
   - `charges[] {label, amount}`
   - `total`, `warnings[]`
6. **DTOs** (`src-tauri/src/core/dtos/bill_scan_dto.rs`): `ScannedBill` (the parsed model output) and `PurchaseScanDraft`:
   - The vendor's raw name plus a matched `party_id`.
   - `invoice_ref`, `transaction_date` (null when it's a future date or doesn't parse).
   - Lines, each with the raw name, `item_id`, the top 3 candidates, `quantity`, `unit_cost`, and flags (`Unmatched`, `AmountMismatch`, `ZeroQty`).
   - Charges matched to charge types, a total-mismatch warning, and the model's warnings.
7. **Service** (`src-tauri/src/services/contracts/bill_scan_service.rs` and `services/impl/bill_scan_service_impl.rs`):
   - `scan_purchase_bill(image_path)`:
     - Checks the image type (jpg/png/webp by magic bytes) and size, then encodes it as base64.
     - Builds the `generateContent` body and calls a `BillScanTransport` trait (reqwest in production, a fake in tests).
     - Parses the response, then matches lines with the existing item FTS search and the vendor with the existing party search (vendors only, soft-deleted parties excluded).
     - Read-only: it never writes to the database.
   - `save_ai_settings(url, key?)`, `clear_ai_key()`, `test_ai_connection()`. The test sends a tiny text-only request and reports OK or the error.
   - Clear `AppError` messages for: disabled, not configured, 401/403 bad key, 429 quota, timeout, unreachable, unparseable output.
8. **Wiring:** add to `AppState` (`src-tauri/src/app_state.rs`). Commands go in a new `src-tauri/src/api/bill_scan_controller.rs` and are registered in `main.rs`.

## Frontend
1. Add `scanPurchaseBill`, `saveAiSettings`, `clearAiKey` and `testAiConnection` to `src/api/ipc.ts`, `src/types/api.ts` and the mock client.
2. `src/pages/SettingsPage.tsx` gets an "AI bill scan" section, shown only when `billScanEnabled`:
   - Fields: the endpoint URL, a password-masked key field (write-only), a Test connection button, and the privacy note.
3. Extend `src/pages/purchasePrefill.ts` with `draft?: PurchaseScanDraft`.
4. `src/pages/NewPurchasePage.tsx`:
   - The "Scan bill" button shows only when enabled. If no endpoint is saved, it points to Settings.
   - The button opens a file picker (`tauri-plugin-dialog`), shows a spinner, and then fills the vendor, invoice ref, date and matched lines through the page's existing line-adding path.
   - A "Check these" strip lists unmatched or flagged lines with their candidates, so the user can pick an item or dismiss the line.
   - Saving uses the existing `create_purchase` unchanged.

## Docs
- `docs/bill_scan_contract.md`:
  - The request and response shape.
  - What the VM must do: check the token, add the real key, forward to Gemini, and return Gemini's response unchanged.
  - A 15-line example wrapper.
- A new DESIGN_DECISIONS entry: one Gemini-compatible endpoint, the key kept in Credential Manager and out of the DB and backups, the prompt owned by the backend.
- A CHECKPOINT note.

## Phased commits (branch `feature/bill-scan`, v1.10 J1)
1. Config gate, migration v13, the endpoint repo methods, and `SecretStore`.
2. Prompt, schema, DTOs, and the service with the fake transport, plus tests.
3. The reqwest transport, settings/test-connection commands, and AppState wiring.
4. The frontend Settings section (hidden by default).
5. The Scan button, prefill and "Check these" strip on New Purchase.
6. Docs (contract, DD, CHECKPOINT).

## Verification
- `cargo test`, using the fake transport and the in-memory secret store:
  - Matched and unmatched lines, amount and total mismatch, future date nulled.
  - Bad image type or oversize, disabled refusal, not configured.
  - 401/429/timeout mapping, malformed model JSON.
  - Migration v13 on an existing v12 database.
  - One transport test against a local tokio TCP stub checks the `x-goog-api-key` header and the body shape.
- `npx vitest run`:
  - Settings section and button hidden when disabled.
  - A mock draft fills the page and the "Check these" strip.
  - Save sends the normal payload.
  - The key field never shows a saved key.
- `npx tsc --noEmit`, `cargo clippy --all-targets`.
- Manual, once the VM is ready:
  1. Set `billScan.enabled: true` and rebuild.
  2. In Settings, enter the VM URL and token, then click Test connection.
  3. Scan a typed bill and a handwritten bill.
  4. Also try Google's direct URL with a key, to confirm the same path works.
