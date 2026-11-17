# UI/UX Specification Document: Book Keeper
**System:** Offline-First Retail Accounting & Inventory Management Desktop Application  
**Target Environment:** Windows Desktop (Tauri v2 + React + Tailwind CSS)  
**Parent Document:** [High-Level Design (HLD)](file:///c:/custom_apps/book_keeper/docs/book_keeper_HLD.md)  
**Status:** Approved Page-Wise UI/UX Baseline  

---

## 1. Design Philosophy & Ergonomics

1. **Heads-Down Keyboard-First Billing:** The retail counter workflow is designed to operate completely without a mouse. Predictable `Enter`/`Tab` key sequences jump from Item Search to Quantity to Bundle Selection to Table, looping back instantly for the next scan/type.
2. **High-Density, Clutter-Free Layout:** All business-critical numbers (current stock, line totals, balance dues, cash drawer position) are prominently visible without navigating through nested menus.
3. **Semantic Status Colors:**
   - **Emerald / Green:** Paid status, stock healthy, positive cash inflow, change due.
   - **Amber / Yellow:** Low stock alert (`< 5 units`), partial payment status.
   - **Rose / Red:** Unpaid credit due, negative stock block, cash outflow.
4. **Payment Methods Note:** Per design baseline, payment mode selection (UPI, Card, Bank) is deferred in the UI to minimize counter friction; the database retains the column as `NULLABLE` for future version extensibility.

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
| Hotkeys: F2: Sale | F3: Purchase | F4: Search Item | Ctrl+B: Backup | Alt+S: Settings | Help (?)       |
+------------------------------------------------------------------------------------------------------+
```

### 2.1. Top Header Bar
- **Shop Title Pill:** Displays active shop name from `SHOP_SETTING`.
- **System Date & Time:** Live system date.
- **Cash Drawer Live Pill:** Real-time physical cash in drawer:
  $$\text{Cash Drawer} = (\text{Upfront Cash Sales} + \text{Customer Cash Settlements}) - (\text{Upfront Cash Purchases} + \text{Vendor Cash Settlements} + \text{Cash Refunds})$$
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
* **Top KPI Ribbon:**
  - **Today's Gross Sales (₹)** + Count of completed bills.
  - **Today's Purchases (₹)** + Count of inward batches.
  - **Today's Net Profit (₹):** Real-time calculation: $\text{Gross Sales Today} - \text{COGS of Sold Lots Today}$.
  - **Total Inventory Valuation (₹):** Real-time sum: $\sum (\text{Remaining Lot Qty} \times \text{Unit Cost})$.
  - **Total Customer Receivables (₹):** Sum of outstanding customer credit dues.
  - **Total Vendor Payables (₹):** Sum of outstanding supplier bills.
* **Action Header Bar:**
  - `[+ New Sale (F2)]`
  - `[+ New Purchase (F3)]`
  - `[Settle Due (F5)]`
  - `[Stock Adjustment]`
* **Main Left/Center Pane:**
  - **Sales vs Purchases Visual Trend:** Interactive graph toggleable by *Today / 7 Days / Month / Year*.
  - **Recent Transactions Feed:** Real-time table of last 10 transactions (Time, Code, Party, Amount, Status: `PAID`/`PARTIAL`/`UNPAID`, Quick Action).
* **Right Contextual Pane — Low Stock Monitor (`current_stock < 5`):**
  - Instant list of items where aggregated stock is strictly under 5 base units.
  - Row columns: Item Name, Category, Stock Level.
  - Row Action: `[Inward (F3)]` pre-fills item into a new purchase order.

---

### Page 2: New Sale / Counter Billing (`F2`)
* **Purpose:** High-speed retail checkout with 100% keyboard navigation and strict negative-stock prevention.
* **Header Controls:**
  - Auto-generated Invoice Code (`SAL-XXXX`).
  - Date Picker (defaults to current date, editable for delayed entries).
  - Customer Selector: Typeahead matching Name/Phone or defaults to "Walk-in Customer". Displays previous outstanding balance if customer selected.
* **Rapid Item Entry Row (Sequential Focus Cycle):**
  1. **Item Search Input (FTS5):** Type 2-3 characters or scan barcode. Press `Enter` to select top item and auto-populate base price and optional MRP.
  2. **Available Stock & Price Indicator:** Real-time stock and pricing display (e.g., `Available: 28 PCS | Rate: ₹15.00 | MRP: ₹20.00`). Displays bold error if stock is 0.
  3. **Quantity Input:** Defaults to `1`. Press `Enter` to accept or type custom quantity.
  4. **Packaging / Unit Selector:** Select base unit (`PCS`, `SHEET`, `KG`) or bundle (`Box of 10`, `Dozen`).
     - Inline Shortcut: `Alt + B` opens quick modal to add a new bundle definition on the fly.
  5. **Unit Rate (₹):** Pre-populated with default rate; editable.
  6. **Add Trigger (`Enter`):** Appends line item to Cart Table and resets cursor to Search Input.
* **Cart Line Items Table:**
  - Columns: `#`, `Item Name`, `Packaging / Unit`, `Quantity (Base Units)`, `MRP (₹)`, `Rate (₹)`, `Line Total (₹)`, `Remove [Del]`.
* **Checkout & Payment Footer:**
  - Totals: Total line items, total base units, Subtotal (₹).
  - Payment Status Selector: `PAID` (default), `PARTIAL`, `UNPAID`.
  - Payment Inputs:
    - If `PARTIAL`: Focuses `Paid Amount (₹)`, auto-computes `Balance Due = Subtotal - Paid Amount`. Requires customer assignment.
    - If `UNPAID`: Paid Amount = ₹0, Balance Due = Subtotal.
  - Remarks / Notes field.
  - Primary Action: `[Complete Sale (Ctrl + Enter)]` — commits FIFO depletion and opens On-Screen Sale Summary Dialog.
  - Discard Action: `[Cancel Bill (Esc)]`.

---

### Page 3: On-Screen Sale Summary Dialog (Modal Overlay)
* **Purpose:** High-visibility on-screen dialog displayed immediately upon checkout completion (FR-15).
* **Display Elements:**
  - Invoice Code & Customer Name.
  - **Grand Total (₹):** Extra-large bold text.
  - Payment Status (`PAID`, `PARTIAL`, `UNPAID`).
  - **Tendered Change Calculator:**
    - Input: `Cash Received (₹)` (auto-focused on open).
    - Output: `Change to Return (₹)` (prominent green text).
* **Keyboard Action:**
  - Pressing `Enter` or `Space` dismisses dialog, resets billing counter to blank state, and focuses Item Search for next customer.

---

### Page 4: New Purchase / Inward Batch Entry (`F3`)
* **Purpose:** Capture incoming inventory shipments, record supplier purchase invoices, and create new FIFO lots.
* **Header Controls:**
  - Auto-generated Purchase Code (`PUR-XXXX`).
  - Inward Date.
  - Supplier / Vendor Selector (Search or `+ Add Vendor`).
  - Supplier Invoice Reference / DC Number.
* **Inward Line Item Entry Row:**
  - Item Search / Selection.
  - Packaging / Unit Selector (`Base Unit` or `Bundle`).
  - Inward Quantity.
  - Inward Unit Cost (₹ per bundle or ₹ per base unit; system auto-normalizes cost per base unit for FIFO storage).
  - Multiplier Preview: E.g., `4 Boxes × 25 = 100 PCS added to stock`.
* **Inward Batches Table:**
  - Columns: `#`, `Item Name`, `Packaging`, `Inward Qty`, `Base Units`, `Unit Cost (₹)`, `Line Total (₹)`, `Actions`.
* **Footer & Settlement:**
  - Total Purchase Value (₹).
  - Payment Status: `PAID`, `PARTIAL`, `UNPAID`.
  - `Paid Amount (₹)` and `Balance Due (₹)` (Vendor Payable).
  - Action: `[Record Inward Stock (Ctrl + Enter)]` — creates `PURCHASE_ITEM` FIFO lots with `remaining_quantity = quantity`, updates cached item stock, and refreshes low-stock flag.

---

### Page 5: Inventory & Item Master (`F6`)
* **Purpose:** Complete product catalog management, packaging tiers, and stock monitoring.
* **Filter & Search Bar:**
  - FTS5 search (Name, Barcode, Category).
  - Category Filter Dropdown.
  - Quick Toggle: `[x] Show Only Low Stock (< 5 units)`.
  - Action Buttons: `[+ Add Item]`, `[Import Items (CSV)]`, `[Export Catalog (Excel)]`.
* **Items Table:**
  - Columns: `Barcode`, `Item Name`, `Category`, `Base Unit`, `Current Stock`, `Stock Status Badge`, `MRP (₹)`, `Default Sale Price (₹)`, `Bundles Configured`, `Actions`.
* **Item Details / Edit Drawer:**
  - General Info: Name, Category, Barcode, Base Unit, MRP (₹, optional), Default Selling Price.
  - **Packaging Bundles Sub-table:**
    - List of bundles (e.g. "Dozen" = 12 PCS, "Box" = 50 PCS).
    - `[+ Add Bundle]` button with standard presets (`Box`, `Dozen`, `Carton`, `Pack of 5`, `Pack of 10`) + custom names, Multiplier, Bundle MRP, Bundle Sale Price, Outer Barcode.
* **Batch Lot Inspector (Row Expand):**
  - Inspects active inward FIFO lots for the item: Lot Date, Purchase Code, Vendor, Original Inward Qty, Remaining Qty, Unit Cost.

---

### Page 6: Credit & Dues Management (Khata Ledger)
* **Purpose:** Manage customer receivables and vendor payables with date-accurate settlement tracking (FR-06).
* **Top Level Tabs:**
  - **Tab 1: Customer Receivables** (Money owed to the shop).
  - **Tab 2: Vendor Payables** (Money owed by the shop).
* **Dues Table:**
  - Columns: `Party Name`, `Phone`, `Unsettled Bills Count`, `Earliest Bill Date`, `Total Invoiced (₹)`, `Total Paid (₹)`, `Net Due Balance (₹)`, `Actions`.
* **Settlement Action Modal (`Settle Payment`):**
  - Displays list of unpaid/partial bills for the selected party.
  - Form Fields:
    - `Settlement Date` (defaults to today).
    - `Settlement Amount (₹)`.
    - `Notes / Receipt Reference`.
  - Behavior: Allocates settlement amount against oldest unpaid bills (FIFO bill settlement). Decrements `balance_due` and updates status to `PAID` when fully settled.
  - Cash Register Impact: Immediately logs cash inflow or outflow in `PAYMENT_SETTLEMENT` to balance the day's cash drawer.

---

### Page 7: Sales History Ledger
* **Purpose:** Audit log of all past customer invoices.
* **Filters:** Date Range, Payment Status (`All`, `PAID`, `PARTIAL`, `UNPAID`), Customer Search.
* **Table Columns:** `Invoice Code`, `Date & Time`, `Customer`, `Items Summary`, `Total Amount (₹)`, `Paid (₹)`, `Balance Due (₹)`, `Status Badge`, `Actions`.
* **Actions:**
  - `[View Invoice / Line Items]`
  - `[Record Settlement]` (active if balance due > 0)
  - `[Initiate Return / Replacement]`

---

### Page 8: Purchase History Ledger
* **Purpose:** Audit log of all inward supplier shipments and lot tracking.
* **Filters:** Date Range, Payment Status, Vendor Search.
* **Table Columns:** `Purchase Code`, `Date`, `Vendor`, `Supplier Bill Ref`, `Total (₹)`, `Paid (₹)`, `Balance Due (₹)`, `Status Badge`, `Actions`.
* **Actions:**
  - `[View Batch Lines & Remaining FIFO Quantities]`
  - `[Record Vendor Payment]` (active if balance due > 0)
  - `[Initiate Vendor Return]`

---

### Page 9: Returns, Replacements & Physical Adjustments
* **Purpose:** Formal handling of returns, swaps, and inventory count corrections without breaking transaction immutability.
* **Tab 1: Customer Sales Return & Swap:**
  - Invoice search / item selection.
  - Returned Quantity & Condition:
    - `RESTOCKABLE` — Item re-enters active inventory.
    - `DAMAGED / SCRAP` — Item is written off to scrap.
  - Settlement Resolution:
    - `CASH REFUND` (logs outflow from today's cash drawer).
    - `REDUCE CREDIT DUE` (credits customer's outstanding balance).
    - `DEFECTIVE REPLACEMENT` (swaps defective unit for a fresh unit of equal value; zero net monetary movement).
* **Tab 2: Vendor Purchase Return:**
  - Inward lot selection and return quantity.
  - Reduces vendor payable or logs cash refund from supplier.
* **Tab 3: Standalone Stock Adjustments:**
  - Direct item search (Name/Barcode) without requiring invoice reference.
  - Fields: `Adjustment Date`, `Quantity` (+ or - base units), `Reason` (`DAMAGED`, `EXPIRED`, `COUNT_CORRECTION`), `Notes`.
  - Recalculates cached stock and low-stock flag immediately.

---

### Page 10: Financial Reports & Analytics (`FR-09`)
* **Purpose:** Periodic accounting and valuation reports.
* **Filter Bar:** Quick buttons (`Today`, `This Week`, `This Month`, `This Year`, `Custom Range`).
* **Report Modules:**
  1. **Profit & Loss Report:**
     - Gross Sales Revenue (₹)
     - Minus FIFO Cost of Goods Sold (COGS) (₹)
     - **= Net Operating Profit (₹)**
  2. **FIFO Inventory Valuation Report:**
     - Active lots count, total units by category, and **Total Stock Valuation (₹)** based on actual inward purchase costs.
  3. **Daily Cash Flow Register:**
     - Upfront Cash Inflows (Cash Sales).
     - Customer Settlement Inflows (Credit collected).
     - Upfront Cash Outflows (Cash Purchases).
     - Vendor Settlement Outflows (Supplier dues cleared).
     - Customer Cash Refund Outflows.
     - **Net Cash Drawer Balance.**
* **Export Controls:** `[Export to Excel (.xlsx)]`, `[Export to CSV]`.

---

### Page 11: Parties Directory (Customers & Suppliers)
* **Purpose:** Unified directory for customer credit records and supplier contact details.
* **Tabs:** `Customers` and `Suppliers`.
* **Table Columns:** `Party Name`, `Phone`, `Address / Notes`, `Total Transactions`, `Current Outstanding Balance (₹)`, `Actions`.
* **Actions:** `[+ Add Party]`, `[Edit Details]`, `[View Transaction History]`.

---

### Page 12: Settings, Shop Profile & Local Backup
* **Purpose:** System configuration, shop header profile, and local data persistence.
* **Shop Profile:**
  - Shop Name, Phone, Address, Tax ID / GSTIN / PAN (optional).
  - Persisted in `SHOP_SETTING` for headers and future invoice printing.
* **Local Database Backup & Snapshot:**
  - Status display: Current SQLite file location (`%APPDATA%/book_keeper/shop.db`).
  - Last backup timestamp.
  - **`[One-Click Backup to USB / Folder]` Button:** Windows file picker to save a timestamped copy (e.g. `shop_backup_YYYYMMDD_HHMM.db`).
  - Database Integrity Check utility.
* **Data Migration:** Bulk Import/Export templates for Items and Parties.
* **Keyboard Hotkeys Cheatsheet:** Reference table of all desktop keyboard shortcuts.

---

## 4. Modal Dialogs & Inline Overlays

1. **Inline Packaging Bundle Modal (`Alt + B`):**
   - Opened directly from the unit dropdown during billing or purchase (also used in Item Master).
   - **Dynamic Self-Learning Suggestions (Zero Hardcoding):** As the user types in the `Bundle Name` field, the combobox auto-suggests existing packaging name + size pairings loaded dynamically from `ITEM_BUNDLE` (e.g. `Box (10 units)`, `Box (20 units)`, `Dozen (12 units)`, `Carton (100 units)`) ordered by usage frequency.
   - Selecting a suggestion instantly fills both the Name and Multiplier, leaving cursor focus on Bundle Price.
   - If defining a brand new package (e.g. "Strip" of 10), typing it and saving automatically adds it to the system's dynamic suggestion index for all future items without code changes.
   - Fields: Bundle Name, Multiplier relative to base unit (`units_per_bundle`), Bundle MRP (₹, optional), Default Bundle Price (₹), Outer Barcode (optional).
   - Commits immediately to `ITEM_BUNDLE` and selects it in the active line.
2. **Inline Party Creation Modal (`Alt + C` / `Alt + V`):**
   - Quick creation of Customer or Vendor on the fly without abandoning cart.
3. **Negative Stock Alert:**
   - Inline blocking alert if entered sale quantity exceeds available base units.

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
| **FR-08** | Standalone Stock Adjustments | **Page 9** (Adjustment Tab) & **Page 1** (Quick action) |
| **FR-09** | Financial Aggregations & Reports | **Page 10** (Reports) & **Page 1** (KPI Cards) |
| **FR-10** | Lists & Operational Views | **Page 5, 6, 7, 8, 11** |
| **FR-11** | Data Portability & Local Backup | **Page 12** (Settings & Local Backup) |
| **FR-12** | Invoicing Foundation | **Page 7** (Sales History) & **Page 12** (Shop Profile) |
| **FR-13** | Multi-UOM Bundles & Inline Creation | **Page 2 & 4** (`Alt + B`), **Page 5** (Item Master) |
| **FR-14** | High-Speed FTS5 Typeahead | **Page 2 & 4** (Search row), Global Bar (`F4`) |
| **FR-15** | Keyboard Checkout & Summary Modal | **Page 2** (Enter flow) & **Page 3** (Summary Dialog) |
