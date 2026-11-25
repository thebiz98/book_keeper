# UI/UX Specification Document: Book Keeper
**System:** Offline-First Retail Accounting & Inventory Management Desktop Application  
**Target Environment:** Windows Desktop (Tauri v2 + React + Tailwind CSS)  
**Parent Document:** [High-Level Design (HLD)](book_keeper_HLD.md) | [Design Decisions](DESIGN_DECISIONS.md)  
**Status:** Approved Page-Wise UI/UX Baseline (rev. 2026-11-21 alignment)  

---

## 1. Design Philosophy & Ergonomics

1. **Heads-Down Keyboard-First Billing:** The retail counter workflow is designed to operate completely without a mouse. Predictable `Enter`/`Tab` key sequences jump from Item Search to Quantity to Unit/Bundle to Rate to Cart, looping back instantly for the next scan/type.
2. **High-Density, Clutter-Free Layout:** All business-critical numbers (current stock, line totals, balance dues, cash drawer position) are prominently visible without navigating through nested menus.
3. **Semantic Status Colors:**
   - **Emerald / Green:** Paid status, stock healthy, positive cash inflow, change due.
   - **Amber / Yellow:** Low stock alert (`< 5 units`), partial payment status.
   - **Rose / Red:** Unpaid credit due, negative stock block, cash outflow.
4. **Payment Methods Note:** Per design baseline, payment mode selection (UPI, Card, Bank) is deferred in the UI to minimize counter friction; the database retains the column as `NULLABLE` for future version extensibility.
5. **Sticky column headers:** on every list the header row stays visible while rows scroll (light and dark: opaque header).

---

## 2. Global Shell & Navigation Framework

The application maintains a persistent desktop shell across all views.

```
+------------------------------------------------------------------------------------------------------+
| [Icon] Book Keeper  |  Shop: [Gupta Stationery]  |  Today: [17 Nov 2026]  | Cash Drawer: ₹4,250      |
+-------------------+----------------------------------------------------------------------------------+
| NAVIGATION        | MAIN WORKSPACE CONTENT AREA                                                      |
| [F1] Dashboard    |                                                                                  |
| [F2] New Sale     |                                                                                  |
| [F3] New Purchase |                                                                                  |
| ----------------- |                                                                                  |
| Items / Inventory |                                                                                  |
| Sales History     |                                                                                  |
| Purchase History  |                                                                                  |
| Credit & Dues     |                                                                                  |
| Returns & Adjust  |                                                                                  |
| Parties (Cust/Ven)|                                                                                  |
| Reports           |                                                                                  |
| Settings          |                                                                                  |
+-------------------+----------------------------------------------------------------------------------+
| Hotkeys: F1 Home | F2 Sale | F3 Purchase | F4/Ctrl+F Item | F5 Dues | F6 Inventory | Ctrl+B Backup    |
+------------------------------------------------------------------------------------------------------+
```

### 2.1. Top Header Bar
- **Shop Title Pill:** Displays active shop name from `SHOP_SETTING`.
- **System Date & Time:** Live system date.
- **Cash Drawer Live Pill:** Today's net cash from the payment ledger:
  $$\text{Cash Drawer} = (\text{Sale Upfront} + \text{Customer Settlements} + \text{Vendor Refunds In}) - (\text{Purchase Upfront} + \text{Vendor Settlements} + \text{Customer Refunds Out})$$
  Return credits against dues are not cash and are excluded. Refreshes after every money-moving action.
- **Global Item Search Popover (`F4` or `Ctrl + F`):** Quick item stock and price lookup accessible anywhere without navigating away.
- **Local DB State:** Indicator displaying local SQLite health (`shop.db Connected`).

### 2.2. Left Navigation Bar
- Collapsible sidebar with quick key shortcuts:
  - `[F1]` Dashboard / Home
  - `[F2]` New Sale / Billing (Primary action styling)
  - `[F3]` New Purchase / Inward
  - Items & Bundles (`F6`)
  - Sales History
  - Purchase History
  - Credit & Dues (Khata)
  - Returns & Stock Adjustments
  - Parties Directory (Customers & Suppliers)
  - Financial Reports
  - Settings & Local Backup

---

## 3. Page-by-Page Specifications

### Page 1: Dashboard / Home Cockpit (`F1`)
* **Purpose:** Daily command center for stock health, cash register balancing, and credit dues.
* **Summary Cards (v1.3, FR-32, FR-33):** three grouped cards replace the KPI ribbon (side by side on wide screens, stacked on narrow). Each row is a link: `label + caption ... amount`. There is no profit figure on this page (it is in Reports and Sales History).
  - **Today:** Sales (amount, "n bills") -> Sales History; Purchases (amount, "n batches") -> Purchase History.
  - **Dues:** Receivables ("customers owe you"), Payables ("you owe vendors") -> Credit & Dues.
  - **Cash & Stock:** **Cash in hand** (largest, coloured; running all-time total including opening cash, never resets), Today's expenses -> Expenses, Inventory valuation (sum of remaining lot qty x unit cost) -> Reports.
  - Wireframe: `[ TODAY: Sales / Purchases ]  [ DUES: Receivables / Payables ]  [ CASH & STOCK: CASH IN HAND / Expenses / Valuation ]`
* **Action Header Bar:**
  - `[+ New Sale (F2)]`
  - `[+ New Purchase (F3)]`
  - `[Settle Due (F5)]`
  - `[Stock Adjustment]`
* **Main Left/Center Pane:**
  - *(2026-11-25: the Sales vs Purchases trend moved to Financial Reports as a collapsed, optional panel; it is not on the dashboard.)*
  - **Recent Transactions Feed:** Table of last 10 sales, purchases and returns (`get_recent_transactions`): Time, Code, Party, Amount, Status (`PAID`/`PARTIAL`/`UNPAID`; none for returns), Quick Action (open details).
* **Right Contextual Pane — Low Stock Monitor (`current_stock < 5`):**
  - Instant list of items where aggregated stock is strictly under 5 base units.
  - Row columns: Item Name, Category, Stock Level.
  - Row Action: `[Inward (F3)]` pre-fills item into a new purchase order.

---

### Page 2: New Sale / Counter Billing (`F2`)
* **Purpose:** High-speed retail checkout with 100% keyboard navigation and strict negative-stock prevention.
* **Header Controls (v1.3, FR-30):**
  - Left: Customer Selector: Typeahead matching Name/Phone or defaults to "Walk-in Customer". Displays previous outstanding balance if customer selected.
  - Right: Auto-generated Invoice Code (`SAL-XXXX`) and Date Picker (defaults to current date, editable for delayed entries).
  - Wireframe: `[ Customer ................ ]            [Next Invoice SAL-0042] [Date]`
* **Rapid Item Entry Row (Sequential Focus Cycle):**
  1. **Item Search Input (FTS5):** Type 2-3 characters or scan barcode (item or bundle barcode). Press `Enter` to select top item and auto-populate base price and optional MRP. Pressing `Enter` with the field **empty** completes the bill (opens the Payment dialog).
  2. **Available Stock & Price Indicator:** Real-time stock and pricing display (e.g., `Available: 28 PCS | Rate: ₹15.00 | MRP: ₹20.00`), net of quantity of this item already in the cart. Displays bold error if stock is 0.
  3. **Quantity Input:** Defaults to `1`. Press `Enter` to accept or type custom quantity.
  4. **Packaging / Unit Selector:** Defaults to base unit (or the last unit used for this item); arrow keys switch to a bundle (`Box of 10`, `Dozen`). Bundle barcode scans pre-select that bundle.
     - The last option of the dropdown is **"+ New bundle..."** (only when an item is selected); it, or `Alt + B`, opens the quick modal to add a bundle on the fly. Cancel keeps the previous unit. There is no separate Bundle button (v1.3, FR-29).
  5. **Unit Rate (₹):** Pre-populated with default base or bundle price; editable.
  6. **Add Trigger (`Enter`):** Appends line item to Cart Table (blocked with the inline Negative Stock Alert if cart total for the item would exceed stock) and resets cursor to Search Input.
* **Cart Line Items Table:**
  - Columns: `#`, `Item Name`, `Packaging / Unit` (e.g. `2 × Box of 10`), `Quantity (Base Units)`, `MRP (₹)`, `Rate (₹)`, `Line Total (₹)`, `Remove [Del]`.
* **Checkout Footer (v1.3, FR-30):**
  - Left: Items / Total Units and the Remarks / Notes field.
  - Right, right-aligned stack directly above the actions: Items Subtotal (₹); when charges exist Charges (+), Discounts (−) and Grand Total. The payable figure is the largest.
  - Wireframe: `[Items: 3 | Units: 12 | Notes ....]      Items Subtotal 1,200.00 / Charges (+) / Discounts (-) / GRAND TOTAL  then  [Clear] [Complete Sale]`
  - Primary Action: `[Complete Sale (Enter on empty search / Ctrl + Enter)]` — opens the Payment dialog (Page 3). Nothing is saved until the dialog is confirmed.
  - Discard Action: `[Cancel Bill (Esc)]` with confirmation when the cart is not empty.

---

### Page 3: Payment & Sale Summary Dialog (Modal Overlay)
* **Purpose:** Collect payment before committing, then confirm the result (FR-15).
* **Step A — Payment (before commit):**
  - Invoice Code (provisional) & Customer Name (or Walk-in).
  - **Grand Total (₹):** Extra-large bold text.
  - Payment Status: `PAID` (default) / `PARTIAL` / `UNPAID`. `PARTIAL` and `UNPAID` require a customer (`Alt + C` creates one inline).
  - `Cash Received (₹)` (auto-focused; prefilled with total for PAID; hidden for UNPAID).
  - Live output: `Change to Return (₹)` for PAID (green), `Balance Due (₹)` for PARTIAL (amber). Invalid combinations shown in red and block confirm.
  - `Enter` → commits the sale (FIFO depletion, ledger). On error (e.g. stock changed), message shown, stays on Step A. `Esc` → back to cart, nothing saved.
* **Step B — Result (after commit):**
  - Final bill code, total, paid, balance due, change to return.
  - `Enter` or `Space` dismisses, resets billing counter to blank state, and focuses Item Search for next customer.

---

### Page 4: New Purchase / Inward Batch Entry (`F3`)
* **Purpose:** Capture incoming inventory shipments, record supplier purchase invoices, and create new FIFO lots.
* **Header Controls:**
  - Auto-generated Purchase Code (`PUR-XXXX`).
  - Inward Date.
  - Supplier / Vendor Selector (Search or `+ Add Vendor` / `Alt + V`).
  - Supplier Invoice Reference / DC Number (stored on the purchase).
* **Inward Line Item Entry Row (v1.3, FR-30: one line; fixed one-line labels above equal-height fields; helper text in a reserved line below):**
  - Item Search / Selection (placeholder "Search item by name or barcode...").
  - Packaging / Unit Selector (`Base Unit` or `Bundle`).
  - Inward Quantity.
  - Inward Unit Cost (₹ per bundle or ₹ per base unit; system auto-normalizes cost per base unit for FIFO storage).
  - Multiplier Preview: E.g., `4 Boxes × 25 = 100 PCS added to stock`.
* **Layout mirrors New Sale (2026-11-25):** header has the Vendor selector on the left and the supplier invoice #, the `Next Inward` code pill and the date on the right. The entry row runs Item, Quantity, Unit, Purchase Price, Add (picking an item focuses Quantity, as on New Sale).
* **Inward Batches Table** (same column order as the sale cart):
  - Columns: `#`, `Item Name`, `Packaging`, `Quantity` (base units beneath for a bundle), `Price / base unit` (where the cart shows MRP), `Purchase Price` (where the cart shows Rate), `Line Total`, `Action`.
* **Footer & Settlement** (same layout as New Sale): left = Lines / Total Base Units + Notes; right = the shared totals stack (Subtotal; with charges Charges / Discounts / Grand Total), the payment row, any message, then `[Clear (Esc)]` and the main action.
  - Payment Status: `PAID` (paid = total), `PARTIAL` (0 < paid < total), `UNPAID` (paid = 0). `PARTIAL`/`UNPAID` require a vendor. Paid can never exceed total.
  - `Paid Amount (₹)` and `Balance Due (₹)` (Vendor Payable).
  - A refused save (e.g. unpaid with no vendor: "Pick a vendor to record an unpaid or part-paid purchase") shows its message right above the buttons; a click is never silent.
  - Action: `[Record Inward Stock (Ctrl + Enter)]` — creates `PURCHASE_ITEM` FIFO lots with `remaining_quantity = quantity`, records the upfront payment in the ledger, updates cached item stock, and refreshes low-stock flag.

---

### Page 5: Inventory & Item Master (`F6`)
* **Purpose:** Complete product catalog management, packaging tiers, and stock monitoring.
* **Filter & Search Bar:**
  - FTS5 search (Name, Barcode, Category).
  - Category Filter Dropdown.
  - Quick Toggle: `[x] Show Only Low Stock (< 5 units)`.
  - Action Buttons: `[+ Add Item]`, `[Import Items (CSV / Excel)]` — opens the import dialog: file pick → dry-run report (valid rows, row errors, duplicates) → conflict strategy (`Skip` / `Overwrite details` / `Add stock`) → Confirm → summary. *(Export: deferred to v2.)*
* **Items Table:**
  - Columns: `Barcode`, `Item Name`, `Category`, `Base Unit`, `Current Stock`, `Stock Status Badge`, `MRP (₹)`, `Default Sale Price (₹)`, `Bundles Configured`, `Actions` (Edit · Adjust · **Set stock**, PHYSICAL items only).
* Header row is fixed; the table fills the page height and scrolls inside itself; the pager stays visible below.
* **Item Details / Edit Drawer:**
  - General Info: Name, Category, Barcode, Base Unit, MRP (₹, optional), Default Selling Price.
  - Read-only line *Type: Physical (stocked item)*. No type selector in v1.1; service items are designed later (FR-21).
  - **Packaging Bundles Sub-table:**
    - List of bundles (e.g. "Dozen" = 12 PCS, "Box" = 50 PCS).
    - `[+ Add Bundle]` opens the bundle modal with **dynamic suggestions from existing bundles** (no hard-coded presets) + custom names, Multiplier (> 1), Bundle MRP, Bundle Sale Price, Outer Barcode.
    - Edit (any field, including multiplier) / Delete per bundle at any time. Bundles are only an entry shortcut; past bills keep their own record of units, price and the pack label used.
  - Stock on hand: N - `[Set stock...]` opens the Set Stock dialog (unrecorded stock without a purchase; FR-08 rev.). Other stock changes come from purchases, sales, returns, adjustments. No archive/delete of items in v1.
* **Batch Lot Inspector (Row Expand):**
  - Unrecorded stock shows as an `Adjustment` lot at the entered cost.
  - Lists the item's lots (`get_item_lots`): Lot Date, Code, Source (`Purchase` / `Adjustment` / `Opening` / `Return`), Vendor, Original Qty, Remaining Qty, Unit Cost; active lots first.

---

### Page 6: Credit & Dues Management (Khata Ledger)
* **Purpose:** Manage customer receivables and vendor payables with date-accurate settlement tracking (FR-06).
* **Top Level Tabs:**
  - **Tab 1: Customer Receivables** (Money owed to the shop).
  - **Tab 2: Vendor Payables** (Money owed by the shop).
* Header row is fixed; the table fills the page height and scrolls inside itself; the pager stays visible below.
* **Dues Table:**
  - Columns: `Party Name`, `Phone`, `Unsettled Bills Count`, `Earliest Bill Date`, `Total Invoiced (₹)`, `Total Paid (₹)`, `Net Due Balance (₹)`, `Actions`.
* **Party Row Expand — Open Bills:** each unpaid/partial bill of the party (code, date, total, paid, balance) with its own `[Settle]` button (single-bill mode).
* **Settlement Action Modal (`Receive on account` / `Pay on account` — lump sum):**
  - Displays list of unpaid/partial bills for the selected party, oldest first, with a live preview of how the amount will be split.
  - Form Fields:
    - `Settlement Date` (defaults to today).
    - `Settlement Amount (₹)` (> 0, ≤ party's total outstanding).
    - `Notes / Receipt Reference`.
  - Behavior: Allocates settlement amount against oldest unpaid bills (FIFO bill settlement). Decrements `balance_due` and updates status to `PAID` when fully settled.
  - Cash Register Impact: Immediately logs cash inflow (customer) or outflow (vendor) in the payment ledger to balance the day's cash drawer.
  - The same modal opened from Sales/Purchase History settles **that bill only** (amount ≤ its balance).
* **Row action `[View Ledger]`:** party's bills, payments, credits and refunds in date order (`get_party_ledger`).

---

### Page 7: Sales History Ledger
* **Purpose:** Audit log of all past customer invoices.
* **Filters:** Date Range, Payment Status (`All`, `PAID`, `PARTIAL`, `UNPAID`), Search (invoice code or customer name). Server-side pagination.
* Header row is fixed; the table fills the page height and scrolls inside itself; the pager stays visible below.
* **Table Columns:** `Invoice Code`, `Date & Time`, `Customer`, `Items` (line count), `Total Amount (₹)`, `Paid (₹)`, `Balance Due (₹)`, `Status Badge`, `Actions`.
* **Actions:**
  - `[View Invoice / Line Items]` — lines with returned qty, FIFO cost breakdown, ledger rows.
  - `[Record Settlement]` (active if balance due > 0; single-bill mode)
  - `[Initiate Return / Replacement]` (opens Page 9 Customer tab pre-linked to this bill)
* **Edit / delete (FR-20, DD-41; replaces NFR-03):** the bill drawer has `[Edit]` and `[Delete]`. Buttons are disabled with the blocking reasons listed (from `get_bill_edit_guard`) when the bill has linked returns, return credits (delete) and so on. `[Edit]` opens Page 2 in edit mode; `[Delete]` opens the delete confirmation (§4). The drawer header shows "Edited <date>" for an edited bill.

---

### Page 8: Purchase History Ledger
* **Purpose:** Audit log of all inward supplier shipments and lot tracking.
* **Filters:** Date Range, Payment Status, Search (purchase code or vendor name). Shows supplier purchases only (no ADJ/OPN/RTL lots).
* Header row is fixed; the table fills the page height and scrolls inside itself; the pager stays visible below.
* **Table Columns:** `Purchase Code`, `Date`, `Vendor`, `Supplier Bill Ref`, `Total (₹)`, `Paid (₹)`, `Balance Due (₹)`, `Status Badge`, `Actions`.
* **Actions:**
  - `[View Batch Lines & Remaining FIFO Quantities]`
  - `[Record Vendor Payment]` (active if balance due > 0; single-bill mode)
  - `[Initiate Vendor Return]` (opens Page 9 Vendor tab pre-linked to this bill)
  - `[Edit]` / `[Delete]` in the drawer (as Page 7; edit opens Page 4 in edit mode)

---

### Page 9: Returns, Replacements & Physical Adjustments
* **Purpose:** Formal handling of returns, swaps, and inventory count corrections without breaking transaction immutability.
* **Tab 1: Customer Sales Return & Swap:**
  - Mode: **With bill** (invoice search → bill's lines with sold / already returned / returnable / rate) or **No receipt** (item search, rate, optional customer).
  - Returned Quantity (≤ returnable) & Condition:
    - `RESTOCKABLE` — Item re-enters active inventory (original lot when linked; a new return lot at latest cost when unlinked — cost asked if the item never had stock).
    - `DAMAGED_SCRAP` — Item is written off to scrap.
  - Settlement:
    - `REFUND` — with a bill: value reduces **that bill's** balance due only; any remainder is cash handed back. No receipt: value reduces the selected customer's oldest dues, remainder cash (walk-in: all cash). Preview shows "₹X credited · ₹Y cash".
    - `REPLACEMENT` / Exchange (with bill only) — returned units keep their condition (`RESTOCKABLE` for a colour/size swap, `DAMAGED_SCRAP` for a defective unit); then add any item(s) and quantities to issue from stock. No money moves; a value-difference warning is shown (do a Refund + new sale if the difference must be paid).
* **Tab 2: Vendor Purchase Return:**
  - Bill / vendor selection, then lot selection (supplier lots only, remaining qty shown) or FIFO; return quantity ≤ remaining.
  - Value = lot cost × qty. Reduces the bill's (or vendor's oldest) payable first; any remainder is cash received from the supplier.
* **Tab 3: Standalone Stock Adjustments:**
  - Direct item search (Name/Barcode) without requiring invoice reference.
  - Fields: `Adjustment Date`, `Quantity` (+ or − base units, non-zero), `Reason` (`DAMAGED`, `EXPIRED`, `THEFT_LOST`, `COUNT_CORRECTION`, `UNRECORDED_STOCK` (increase only; adds qty x cost to stock value; never a purchase or income)), `Unit Cost` (positive only; prefilled from latest lot, required if none), `Notes`.
  - Recalculates cached stock and low-stock flag immediately; shows write-off value for negative adjustments.
* **History sub-lists:** recent returns (`list_returns`) and adjustments (`list_stock_adjustments`).

---

### Page 10: Financial Reports & Analytics (`FR-09`)
* **Purpose:** Periodic accounting and valuation reports.
* **Filter Bar:** Quick buttons (`Today`, `This Week`, `This Month`, `This Year`, `Custom Range`).
* **Report Modules:**
  1. **Profit & Loss Report:**
     - Gross Sales (₹)
     - Less Sales Returns (₹) → **Net Sales**
     - Less FIFO COGS, net of returned units (₹) → **Gross Profit**
     - Less Inventory Losses (write-offs, scrap returns) (₹) — COGS above includes units issued in replacements
     - **= Net Profit (₹)**
     - Also shown: Total Purchases (net of vendor returns).
  2. **FIFO Inventory Valuation Report:**
     - Active lots count, items and valuation by category, and **Total Stock Valuation (₹)** based on actual lot costs (all lot sources).
  3. **Cash Flow Register:**
     - Inflows: Sale upfront cash, Customer settlements, Vendor refunds received.
     - Outflows: Purchase upfront cash, Vendor settlements, Customer refunds paid.
     - **Net Cash Balance.**
  4. **Sales vs Purchases trend (optional, 2026-11-25):** collapsed by default under a `Sales vs Purchases trend` toggle; fetched only when opened. Bars per day (per month for 1Y) with *7D / 30D / 1Y*; independent of the date filter.
* *Export controls: deferred to v2.*

---

### Page 11: Parties Directory (Customers & Suppliers)
* **Purpose:** Unified directory for customer credit records and supplier contact details.
* **Tabs:** `Customers` and `Suppliers`.
* Header row is fixed; the table fills the page height and scrolls inside itself; the pager stays visible below.
* **Table Columns:** `Party Name`, `Phone`, `Address / Notes`, `Total Transactions`, `Current Outstanding Balance (₹)`, `Actions`.
* **Actions:** `[+ Add Party]`, `[Edit Details]`, `[View Transaction History]` (party ledger). No archive/delete in v1.
* Add/Edit Party dialog: Enter in any field saves. Validation errors show inside the dialog.

---

### Page 12: Settings, Shop Profile & Local Backup
* **Purpose:** System configuration, shop header profile, and local data persistence.
* **Shop Profile:**
  - Shop Name, Phone, Address, Tax ID / GSTIN / PAN (optional).
  - Persisted in `SHOP_SETTING` for headers and future invoice printing.
* **Local Database Backup & Snapshot:**
  - Status display: Current SQLite file location (`<app data>/shop.db`, or `BOOK_KEEPER_DB_PATH`).
  - Last backup timestamp (this session).
  - **`[One-Click Backup to USB / Folder]` Button (`Ctrl + B`):** folder picker; saves a timestamped copy `book_keeper_backup_YYYYMMDD_HHMM.db` (never overwrites).
  - Database Integrity Check utility.
* *Data migration (VedBill), parties import, exports and restore: deferred to v2.*
* **Keyboard Hotkeys Cheatsheet:** Reference table of all desktop keyboard shortcuts (rendered from the single hotkey registry).

---

## 4. Modal Dialogs & Inline Overlays

1. **Inline Packaging Bundle Modal (`Alt + B`):**
   - Opened directly from the unit dropdown during billing or purchase (also used in Item Master).
   - **Dynamic Self-Learning Suggestions (Zero Hardcoding):** As the user types in the `Bundle Name` field, the combobox auto-suggests existing packaging name + size pairings loaded dynamically from `ITEM_BUNDLE` (e.g. `Box (10 units)`, `Box (20 units)`, `Dozen (12 units)`, `Carton (100 units)`) ordered by usage frequency.
   - Selecting a suggestion instantly fills both the Name and Multiplier, leaving cursor focus on Bundle Price.
   - If defining a brand new package (e.g. "Strip" of 10), typing it and saving automatically adds it to the system's dynamic suggestion index for all future items without code changes.
   - Fields: Bundle Name, Multiplier relative to base unit (`units_per_bundle`), Bundle MRP (₹, optional), Default Bundle Price (₹), Outer Barcode (optional).
   - Commits immediately to `ITEM_BUNDLE` and selects it in the active line.
   - Visible **Cancel** and **Create Bundle** buttons. Enter saves (Ctrl+Enter too), Esc cancels.
2. **Inline Party Creation Modal (`Alt + C` / `Alt + V`):**
   - Quick creation of a Customer or Vendor without leaving the cart: Name (required), Phone (optional), and visible **Save** (primary) and **Cancel** buttons. Enter saves, Esc cancels. Save is disabled until a name is typed.
3. **Negative Stock Alert:**
   - Inline blocking alert if the cart's total base units for an item would exceed available stock.
4. **Set Stock On Hand dialog:**
   - Opened from the Item List row action "Set stock" or the Item drawer. Modes: Set counted stock (default) / Add quantity.
   - Fields: counted (or added) quantity, Date, Unit cost (only when stock increases; prefilled from the latest lot, required if none), Notes.
   - Live preview: increase shows "Unrecorded stock, adds X to stock value"; decrease shows "Count correction, estimated write-off Y"; equal shows "No change" and disables Save.
   - Visible **Cancel** and **Save stock** buttons; Ctrl+Enter saves, Esc cancels. A result view shows the new stock and value change.
5. **Quick-create rule (FR-22):** every quick-create form shows Save + Cancel in addition to Enter/Esc.

---

## 5. Traceability Matrix: HLD Requirements to UI Pages

| Requirement Code | Description | Corresponding UI Page |
|---|---|---|
| **FR-01** | Inward Stock & Purchase Management | **Page 4** (New Purchase) & **Page 8** (Purchase History) |
| **FR-02** | Outward Sales Management | **Page 2** (New Sale) & **Page 7** (Sales History) |
| **FR-03** | Strict FIFO Stock Depletion | **Page 2** (Sale commit), **Page 5** (Lot Inspector), **Page 10** (COGS) |
| **FR-04** | Negative Stock Prevention | **Page 2** (Item Entry validation) |
| **FR-05** | Real-Time Low Stock Monitoring (< 5) | **Page 1** (Right Pane) & **Page 5** (Filter) |
| **FR-06** | Credit & Dues Management | **Page 6** (Khata Ledger & Settlement Modal) |
| **FR-07** | Returns & Replacements | **Page 9** (Customer Return, Swap, Vendor Return) |
| **FR-08** | Standalone Stock Adjustments | **Page 9** (Adjustment Tab), **Page 5** (Set stock row action & item drawer), **Page 1** (Quick action) |
| **FR-09** | Financial Aggregations & Reports | **Page 10** (Reports) & **Page 1** (KPI Cards) |
| **FR-10** | Lists & Operational Views | **Page 1, 2, 4, 5, 6, 7, 8, 9, 10, 11** (sticky headers via shared Table) |
| **FR-11** | Items Import & Local Backup | **Page 5** (Import dialog) & **Page 12** (Backup, Integrity) |
| **FR-12** | Invoicing Foundation | **Page 7** (Sales History) & **Page 12** (Shop Profile) |
| **FR-13** | Multi-UOM Bundles & Inline Creation | **Page 2 & 4** (`Alt + B`), **Page 5** (Item Master) |
| **FR-14** | High-Speed FTS5 Typeahead | **Page 2 & 4** (Search row), Global Bar (`F4`) |
| **FR-15** | Keyboard Checkout & Payment Dialog | **Page 2** (Enter flow) & **Page 3** (Payment / Summary Dialog) |
| **FR-16** | Optional MRP | **Page 2** (indicator & cart), **Page 5** (item & bundle fields) |
| **FR-17** | Soft-Deletion | *Deferred to v2 — no UI in v1* |
| **FR-21** | Item type (entity only) | Page 5 (read-only type line) |
| **FR-22** | Explicit Save/Cancel on quick-create | Page 2/4 (Alt+C/V, Alt+B), Page 11, §4 |
| **NFR-06** | Keyboard Ergonomics | Global shell hotkey registry, **Page 2/3/4** |

### v1.1 Inventory (Page 5) live search and Low Stock Level (v4)
* Search is live (about 150 ms after typing stops, no Search button, Enter runs immediately, page resets to 1). Toggle: "Show Only Low Stock (below each item's level)".
* Columns: expander, Item Name, Category, Base Unit, Current Stock, Status, Low Stock Level (inline), MRP, Sale Price, Bundles, Actions. Barcode column removed (still searchable, still in the drawer).
* Inline cell: always an input; focus selects text; Enter saves and moves to next row's cell; blur saves; Esc reverts; empty/negative/non-numeric shows an inline error and is not saved; unchanged = no save; Status badge updates from the server response; with "Low Stock only" on, a row that stops being low stays until refresh.
* Drawer: "Low Stock Level (base units)", default 5, help "Marked Low Stock when stock falls below this. 0 = never".
* Dashboard low-stock widget shows `stock / level unit`. Pages 7, 8, 11, party selector and bill picker use live search (see FR-14 rev.). Traceability: FR-05 rev., FR-14 rev.


---

## Page 13: Store Expenses (`F7`) - FR-18 (v1.1)
Sidebar item `[F7] Expenses` after Inventory; HotkeyBar `F7 Expenses`; header cash drawer formula appends `- Today's Expenses`.
Layout: left "New Expense" form (Code preview EXP-xxxx, Date, Type typeahead with `[+ Create "..."]`, Amount, Notes, `[Save Expense (Enter)] [Clear (Esc)]`), right Expenses list (date range, type filter, live search, Code/Date/Type/Notes/Amount/Actions, period total, pager).
Keyboard: F7 focuses Type; Enter selects or opens inline create (Save/Cancel buttons; Enter saves, Esc cancels); focus then moves to Amount; Enter to Notes; Enter on Notes or Ctrl+Enter saves; after save toast "EXP-xxxx saved", form resets (date kept), focus returns to Type, list and header cash refresh.
Edit loads the row into the form ("Editing EXP-xxxx", `Update` / `Cancel edit`). Delete: danger ConfirmDialog. Default list range: current month.
Dashboard: KPI tile "Today's Expenses" (`kpi-expenses`), cash sub-text "After today's expenses", action `[+ Expense (F7)]`. Reports: P&L "Less: Operating Expenses" with per-type breakdown before Net Profit; Cash Flow outflow line "Store expenses" after "Customer refunds paid". Traceability: FR-18 -> Page 13, Page 1, Page 10.


## Bill correction (FR-20, `feat/bill-edit-delete`)
* **Page 2 (Sale) edit mode.** Amber banner "Editing SAL-1001 — saving replaces the original bill" with `[Cancel edit]`. The invoice code shows the original code. Date, customer, notes, lines and charges are prefilled. The cart allows the bill's own original quantities on top of current stock. `Esc` / Cancel edit asks "Discard changes?" and returns to Sales History.
* **Page 3 (Payment dialog) in edit mode.** Title and confirm button read "Update bill". Shows "Already received later: ₹applied" and "Due now: ₹open". Cash Received is prefilled with the old upfront amount. The result screen reads "Bill updated". If the date changed, the dialog notes that the cash moves to the new day; if the customer changed it warns "Later payments ₹X will move to <new customer>".
* **Page 4 (Purchase) edit mode.** Same banner. Each line shows "Used: N" when consumed and its quantity cannot go below N ("At least N already used"). Item and Remove are disabled on referenced lines with a tooltip. The cost field of a vendor-returned line is read-only with a tooltip.
* **Pages 6, 7, 8.** A party-ledger row opens the bill drawer. Drawer actions `[Edit]`, `[Delete]` with reasons when blocked.
* **Page 9.** A return row opens the Return Details drawer with `[Delete return]` (typed confirmation of the return code). Disabled with reasons when a newer return exists on the same bill, restocked units were consumed, or the return's lot was used.
* **§4 delete confirmation.** Title "Delete SAL-1001?". Body: line count, total, "₹X cash received for this bill will be removed from cash records" (purchase: "paid ... removed from cash out"), and "Settlements on other bills are unaffected". Typed field: type the bill code. Buttons: Cancel / Delete (danger).
* **Traceability.** FR-20 -> Pages 2, 3, 4, 6, 7, 8, 9.


## Dashboard and lookup layout (FR-40, DD-69, v1.4)
```
+-------------------------------------------------------------+
| Cash & Stock:  Cash in hand | Today's expenses | Inv. value  |
+--------------------------------------------+----------------+
| Today (sales, purchases) | Dues (rcv, pay) | Low stock      |
+--------------------------------------------+ (stock/level,  |
| Recent transactions                        |  Inward)       |
+--------------------------------------------+----------------+
```
Item lookup (F4): a larger dialog (`lg`); the search field stays at the top, the selected item card sits under it, and only the results list scrolls. Reports: the Sales vs Purchases trend is a two-line chart (blue Sales, orange Purchases) with a legend, hover/keyboard tooltip and a Table toggle.