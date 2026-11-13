"""
Generates high-definition, standalone SVGs for Book Keeper HLD diagrams.
"""
import os

DOCS_IMG_DIR = r"c:\custom_apps\book_keeper\docs\images"
os.makedirs(DOCS_IMG_DIR, exist_ok=True)

def generate_system_arch_svg():
    svg = """<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1100 850" width="100%" height="100%" style="background:#0f172a; font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,Helvetica,Arial,sans-serif;">
  <defs>
    <linearGradient id="uiGrad" x1="0%" y1="0%" x2="100%" y2="100%">
      <stop offset="0%" stop-color="#1e293b"/>
      <stop offset="100%" stop-color="#0f172a"/>
    </linearGradient>
    <linearGradient id="coreGrad" x1="0%" y1="0%" x2="100%" y2="100%">
      <stop offset="0%" stop-color="#064e3b"/>
      <stop offset="100%" stop-color="#022c22"/>
    </linearGradient>
    <linearGradient id="dbGrad" x1="0%" y1="0%" x2="100%" y2="100%">
      <stop offset="0%" stop-color="#1e1b4b"/>
      <stop offset="100%" stop-color="#0f172a"/>
    </linearGradient>
    <filter id="shadow" x="-5%" y="-5%" width="110%" height="110%">
      <feDropShadow dx="0" dy="4" stdDeviation="6" flood-color="#000" flood-opacity="0.4"/>
    </filter>
    <marker id="arrow" viewBox="0 0 10 10" refX="6" refY="5" markerWidth="6" markerHeight="6" orient="auto-start-reverse">
      <path d="M 0 1 L 8 5 L 0 9 z" fill="#38bdf8"/>
    </marker>
    <marker id="arrowGreen" viewBox="0 0 10 10" refX="6" refY="5" markerWidth="6" markerHeight="6" orient="auto-start-reverse">
      <path d="M 0 1 L 8 5 L 0 9 z" fill="#34d399"/>
    </marker>
  </defs>

  <!-- Title -->
  <text x="550" y="45" text-anchor="middle" fill="#f8fafc" font-size="24" font-weight="700" letter-spacing="0.5">Book Keeper — High-Level System Architecture</text>
  <text x="550" y="70" text-anchor="middle" fill="#94a3b8" font-size="14">Offline-First Windows Desktop Architecture (SQLite Local Engine)</text>

  <!-- ==================== UI LAYER ==================== -->
  <rect x="50" y="100" width="1000" height="190" rx="12" fill="url(#uiGrad)" stroke="#334155" stroke-width="1.5" filter="url(#shadow)"/>
  <rect x="70" y="115" width="230" height="26" rx="6" fill="#38bdf8" fill-opacity="0.15"/>
  <text x="80" y="133" fill="#38bdf8" font-size="13" font-weight="700">DESKTOP USER INTERFACE (WINDOWS)</text>

  <!-- UI Cards -->
  <!-- Card 1: Dashboard -->
  <g transform="translate(70, 155)">
    <rect width="170" height="115" rx="8" fill="#1e293b" stroke="#475569" stroke-width="1"/>
    <rect width="170" height="28" rx="8" fill="#0284c7" fill-opacity="0.25"/>
    <text x="12" y="19" fill="#38bdf8" font-size="13" font-weight="600">Home Dashboard</text>
    <text x="12" y="48" fill="#cbd5e1" font-size="11">• Low Stock Alerts (&lt; 5)</text>
    <text x="12" y="66" fill="#cbd5e1" font-size="11">• Stock Valuation (₹)</text>
    <text x="12" y="84" fill="#cbd5e1" font-size="11">• Dues (Receivables/Payables)</text>
    <text x="12" y="102" fill="#cbd5e1" font-size="11">• Daily Sales &amp; Profit</text>
  </g>

  <!-- Card 2: Sales Module -->
  <g transform="translate(260, 155)">
    <rect width="170" height="115" rx="8" fill="#1e293b" stroke="#475569" stroke-width="1"/>
    <rect width="170" height="28" rx="8" fill="#059669" fill-opacity="0.25"/>
    <text x="12" y="19" fill="#34d399" font-size="13" font-weight="600">Sales Billing</text>
    <text x="12" y="48" fill="#cbd5e1" font-size="11">• Multi-Item Cart</text>
    <text x="12" y="66" fill="#cbd5e1" font-size="11">• Cash vs Credit Sale</text>
    <text x="12" y="84" fill="#cbd5e1" font-size="11">• Walk-in or Customer</text>
    <text x="12" y="102" fill="#cbd5e1" font-size="11">• Pre-Sale Stock Check</text>
  </g>

  <!-- Card 3: Purchase Module -->
  <g transform="translate(450, 155)">
    <rect width="170" height="115" rx="8" fill="#1e293b" stroke="#475569" stroke-width="1"/>
    <rect width="170" height="28" rx="8" fill="#d97706" fill-opacity="0.25"/>
    <text x="12" y="19" fill="#fbbf24" font-size="13" font-weight="600">Purchase Entry</text>
    <text x="12" y="48" fill="#cbd5e1" font-size="11">• Multi-Item Stock In</text>
    <text x="12" y="66" fill="#cbd5e1" font-size="11">• Unit Cost &amp; Batches</text>
    <text x="12" y="84" fill="#cbd5e1" font-size="11">• Cash vs Credit Purchase</text>
    <text x="12" y="102" fill="#cbd5e1" font-size="11">• Vendor Payables</text>
  </g>

  <!-- Card 4: Settlements & Dues -->
  <g transform="translate(640, 155)">
    <rect width="170" height="115" rx="8" fill="#1e293b" stroke="#475569" stroke-width="1"/>
    <rect width="170" height="28" rx="8" fill="#8b5cf6" fill-opacity="0.25"/>
    <text x="12" y="19" fill="#c084fc" font-size="13" font-weight="600">Dues &amp; Settlements</text>
    <text x="12" y="48" fill="#cbd5e1" font-size="11">• Customer Receivables</text>
    <text x="12" y="66" fill="#cbd5e1" font-size="11">• Vendor Payables</text>
    <text x="12" y="84" fill="#cbd5e1" font-size="11">• 1-Click Settlement</text>
    <text x="12" y="102" fill="#cbd5e1" font-size="11">• Accurate Cash Flow</text>
  </g>

  <!-- Card 5: Returns & Reports -->
  <g transform="translate(830, 155)">
    <rect width="190" height="115" rx="8" fill="#1e293b" stroke="#475569" stroke-width="1"/>
    <rect width="190" height="28" rx="8" fill="#ec4899" fill-opacity="0.25"/>
    <text x="12" y="19" fill="#f472b6" font-size="13" font-weight="600">Returns &amp; Reports</text>
    <text x="12" y="48" fill="#cbd5e1" font-size="11">• Returns &amp; Item Swaps</text>
    <text x="12" y="66" fill="#cbd5e1" font-size="11">• Stock Damage/Audit</text>
    <text x="12" y="84" fill="#cbd5e1" font-size="11">• Day/Wk/Mo/Yr Reports</text>
    <text x="12" y="102" fill="#cbd5e1" font-size="11">• CSV / Excel Export</text>
  </g>

  <!-- Connectors UI -> Core Engine -->
  <path d="M 345 270 L 345 340" stroke="#38bdf8" stroke-width="2" fill="none" marker-end="url(#arrow)"/>
  <path d="M 535 270 L 535 340" stroke="#38bdf8" stroke-width="2" fill="none" marker-end="url(#arrow)"/>
  <path d="M 725 270 L 725 340" stroke="#38bdf8" stroke-width="2" fill="none" marker-end="url(#arrow)"/>

  <!-- ==================== CORE ENGINE LAYER ==================== -->
  <rect x="50" y="345" width="1000" height="175" rx="12" fill="url(#coreGrad)" stroke="#059669" stroke-width="1.5" filter="url(#shadow)"/>
  <rect x="70" y="360" width="240" height="26" rx="6" fill="#34d399" fill-opacity="0.15"/>
  <text x="80" y="378" fill="#34d399" font-size="13" font-weight="700">CORE LOGIC &amp; SERVICES ENGINE</text>

  <!-- Core Component 1: FIFO Engine -->
  <g transform="translate(70, 400)">
    <rect width="215" height="100" rx="8" fill="#064e3b" stroke="#10b981" stroke-width="1"/>
    <text x="14" y="24" fill="#6ee7b7" font-size="13" font-weight="600">FIFO Depletion Engine</text>
    <text x="14" y="48" fill="#a7f3d0" font-size="11">• Inward Batch Allocation</text>
    <text x="14" y="66" fill="#a7f3d0" font-size="11">• Depletes Oldest Lots First</text>
    <text x="14" y="84" fill="#a7f3d0" font-size="11">• Precise COGS Computation</text>
  </g>

  <!-- Core Component 2: Stock Validator -->
  <g transform="translate(310, 400)">
    <rect width="215" height="100" rx="8" fill="#064e3b" stroke="#10b981" stroke-width="1"/>
    <text x="14" y="24" fill="#6ee7b7" font-size="13" font-weight="600">Stock &amp; Audit Validator</text>
    <text x="14" y="48" fill="#a7f3d0" font-size="11">• Zero Negative Stock Guard</text>
    <text x="14" y="66" fill="#a7f3d0" font-size="11">• Auto Low-Stock Flagging (&lt;5)</text>
    <text x="14" y="84" fill="#a7f3d0" font-size="11">• Physical Count Adjustments</text>
  </g>

  <!-- Core Component 3: Ledger & Settlements -->
  <g transform="translate(550, 400)">
    <rect width="220" height="100" rx="8" fill="#064e3b" stroke="#10b981" stroke-width="1"/>
    <text x="14" y="24" fill="#6ee7b7" font-size="13" font-weight="600">Accounting &amp; Dues</text>
    <text x="14" y="48" fill="#a7f3d0" font-size="11">• Customer Due Tracking</text>
    <text x="14" y="66" fill="#a7f3d0" font-size="11">• Supplier Payable Tracking</text>
    <text x="14" y="84" fill="#a7f3d0" font-size="11">• Day-of-Payment Cash Logs</text>
  </g>

  <!-- Core Component 4: Reporting & Backup -->
  <g transform="translate(795, 400)">
    <rect width="230" height="100" rx="8" fill="#064e3b" stroke="#10b981" stroke-width="1"/>
    <text x="14" y="24" fill="#6ee7b7" font-size="13" font-weight="600">Backup &amp; Report Exporter</text>
    <text x="14" y="48" fill="#a7f3d0" font-size="11">• Total Stock Valuation (₹)</text>
    <text x="14" y="66" fill="#a7f3d0" font-size="11">• Profit &amp; Period Aggregates</text>
    <text x="14" y="84" fill="#a7f3d0" font-size="11">• CSV/Excel &amp; SQLite Backup</text>
  </g>

  <!-- Connectors Core -> DB -->
  <path d="M 177 500 L 177 565" stroke="#34d399" stroke-width="2" fill="none" marker-end="url(#arrowGreen)"/>
  <path d="M 417 500 L 417 565" stroke="#34d399" stroke-width="2" fill="none" marker-end="url(#arrowGreen)"/>
  <path d="M 660 500 L 660 565" stroke="#34d399" stroke-width="2" fill="none" marker-end="url(#arrowGreen)"/>
  <path d="M 910 500 L 910 565" stroke="#34d399" stroke-width="2" fill="none" marker-end="url(#arrowGreen)"/>

  <!-- ==================== DATABASE LAYER ==================== -->
  <rect x="50" y="570" width="1000" height="240" rx="12" fill="url(#dbGrad)" stroke="#4f46e5" stroke-width="1.5" filter="url(#shadow)"/>
  <rect x="70" y="585" width="310" height="26" rx="6" fill="#818cf8" fill-opacity="0.15"/>
  <text x="80" y="603" fill="#818cf8" font-size="13" font-weight="700">EMBEDDED STORAGE (SQLITE: shop.db)</text>

  <!-- DB Pills / Grid -->
  <!-- Col 1 -->
  <g transform="translate(70, 625)">
    <rect width="215" height="50" rx="6" fill="#1e1b4b" stroke="#6366f1" stroke-width="1"/>
    <text x="12" y="20" fill="#a5b4fc" font-size="12" font-weight="700">ITEM</text>
    <text x="12" y="38" fill="#cbd5e1" font-size="10.5">name, category, current_stock, is_low_stock</text>

    <rect y="60" width="215" height="50" rx="6" fill="#1e1b4b" stroke="#6366f1" stroke-width="1"/>
    <text x="12" y="80" fill="#a5b4fc" font-size="12" font-weight="700">PARTY</text>
    <text x="12" y="98" fill="#cbd5e1" font-size="10.5">name, phone, party_type, notes</text>

    <rect y="120" width="215" height="50" rx="6" fill="#1e1b4b" stroke="#6366f1" stroke-width="1"/>
    <text x="12" y="140" fill="#a5b4fc" font-size="12" font-weight="700">SHOP_SETTING</text>
    <text x="12" y="158" fill="#cbd5e1" font-size="10.5">shop_name, phone, address, tax_id</text>
  </g>

  <!-- Col 2 -->
  <g transform="translate(310, 625)">
    <rect width="215" height="50" rx="6" fill="#1e1b4b" stroke="#6366f1" stroke-width="1"/>
    <text x="12" y="20" fill="#a5b4fc" font-size="12" font-weight="700">PURCHASE_TRANSACTION</text>
    <text x="12" y="38" fill="#cbd5e1" font-size="10.5">code, date, party_id, payment_mode/status</text>

    <rect y="60" width="215" height="50" rx="6" fill="#1e1b4b" stroke="#6366f1" stroke-width="1"/>
    <text x="12" y="80" fill="#a5b4fc" font-size="12" font-weight="700">PURCHASE_ITEM (FIFO Lot)</text>
    <text x="12" y="98" fill="#cbd5e1" font-size="10.5">item_id, unit_cost, remaining_quantity</text>

    <rect y="120" width="215" height="50" rx="6" fill="#1e1b4b" stroke="#6366f1" stroke-width="1"/>
    <text x="12" y="140" fill="#a5b4fc" font-size="12" font-weight="700">STOCK_ADJUSTMENT</text>
    <text x="12" y="158" fill="#cbd5e1" font-size="10.5">item_id, quantity (+/-), reason</text>
  </g>

  <!-- Col 3 -->
  <g transform="translate(550, 625)">
    <rect width="220" height="50" rx="6" fill="#1e1b4b" stroke="#6366f1" stroke-width="1"/>
    <text x="12" y="20" fill="#a5b4fc" font-size="12" font-weight="700">SALE_TRANSACTION</text>
    <text x="12" y="38" fill="#cbd5e1" font-size="10.5">code, date, party_id, payment_mode/status</text>

    <rect y="60" width="220" height="50" rx="6" fill="#1e1b4b" stroke="#6366f1" stroke-width="1"/>
    <text x="12" y="80" fill="#a5b4fc" font-size="12" font-weight="700">SALE_ITEM</text>
    <text x="12" y="98" fill="#cbd5e1" font-size="10.5">sale_transaction_id, item_id, unit_price</text>

    <rect y="120" width="220" height="50" rx="6" fill="#1e1b4b" stroke="#6366f1" stroke-width="1"/>
    <text x="12" y="140" fill="#a5b4fc" font-size="12" font-weight="700">SALE_STOCK_ALLOCATION</text>
    <text x="12" y="158" fill="#cbd5e1" font-size="10.5">sale_item_id, purchase_item_id, cost</text>
  </g>

  <!-- Col 4 -->
  <g transform="translate(795, 625)">
    <rect width="230" height="50" rx="6" fill="#1e1b4b" stroke="#6366f1" stroke-width="1"/>
    <text x="12" y="20" fill="#a5b4fc" font-size="12" font-weight="700">PAYMENT_SETTLEMENT</text>
    <text x="12" y="38" fill="#cbd5e1" font-size="10.5">settlement_date, type, transaction_id</text>

    <rect y="60" width="230" height="50" rx="6" fill="#1e1b4b" stroke="#6366f1" stroke-width="1"/>
    <text x="12" y="80" fill="#a5b4fc" font-size="12" font-weight="700">RETURN_TRANSACTION</text>
    <text x="12" y="98" fill="#cbd5e1" font-size="10.5">return_type, settlement_type, total</text>

    <rect y="120" width="230" height="50" rx="6" fill="#1e1b4b" stroke="#6366f1" stroke-width="1"/>
    <text x="12" y="140" fill="#a5b4fc" font-size="12" font-weight="700">RETURN_ITEM</text>
    <text x="12" y="158" fill="#cbd5e1" font-size="10.5">item_id, condition (RESTOCK / SCRAP)</text>
  </g>
</svg>"""
    with open(os.path.join(DOCS_IMG_DIR, "system_architecture.svg"), "w", encoding="utf-8") as f:
        f.write(svg)
    print("Generated system_architecture.svg successfully.")

def generate_erd_svg():
    svg = """<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1200 980" width="100%" height="100%" style="background:#0f172a; font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,Helvetica,Arial,sans-serif;">
  <defs>
    <filter id="boxShadow" x="-5%" y="-5%" width="110%" height="110%">
      <feDropShadow dx="0" dy="3" stdDeviation="5" flood-color="#000" flood-opacity="0.45"/>
    </filter>
  </defs>

  <!-- Title -->
  <text x="600" y="40" text-anchor="middle" fill="#f8fafc" font-size="24" font-weight="700">Book Keeper — Entity-Relationship (ER) Diagram</text>
  <text x="600" y="65" text-anchor="middle" fill="#94a3b8" font-size="14">Embedded Relational Schema (SQLite 3)</text>

  <!-- Connecting Lines -->
  <!-- PARTY to PURCHASE_TRANSACTION -->
  <path d="M 330 220 L 410 220" stroke="#64748b" stroke-width="2" stroke-dasharray="4,4"/>
  <!-- PARTY to SALE_TRANSACTION -->
  <path d="M 330 260 L 790 220" stroke="#64748b" stroke-width="2" stroke-dasharray="4,4"/>
  <!-- PURCHASE_TRANSACTION to PURCHASE_ITEM -->
  <path d="M 530 350 L 530 400" stroke="#38bdf8" stroke-width="2.5"/>
  <!-- SALE_TRANSACTION to SALE_ITEM -->
  <path d="M 910 350 L 910 400" stroke="#38bdf8" stroke-width="2.5"/>
  <!-- ITEM to PURCHASE_ITEM -->
  <path d="M 290 530 L 410 530" stroke="#34d399" stroke-width="2"/>
  <!-- ITEM to SALE_ITEM -->
  <path d="M 290 560 L 790 530" stroke="#34d399" stroke-width="2"/>
  <!-- PURCHASE_ITEM & SALE_ITEM to SALE_STOCK_ALLOCATION -->
  <path d="M 530 635 L 670 700" stroke="#f59e0b" stroke-width="2.5"/>
  <path d="M 910 635 L 750 700" stroke="#f59e0b" stroke-width="2.5"/>
  <!-- RETURN_TRANSACTION to RETURN_ITEM -->
  <path d="M 290 850 L 410 850" stroke="#ec4899" stroke-width="2"/>

  <!-- ================= ENTITY BOXES ================= -->

  <!-- 1. PARTY -->
  <g transform="translate(60, 140)" filter="url(#boxShadow)">
    <rect width="270" height="175" rx="8" fill="#1e293b" stroke="#475569" stroke-width="1.5"/>
    <rect width="270" height="32" rx="8" fill="#3b82f6"/>
    <text x="15" y="21" fill="#fff" font-size="13" font-weight="700">PARTY (Customers &amp; Vendors)</text>
    <text x="15" y="55" fill="#f8fafc" font-size="12"><tspan fill="#38bdf8" font-weight="600">PK</tspan>  id : INTEGER</text>
    <text x="15" y="80" fill="#f8fafc" font-size="12">     name : TEXT NOT NULL</text>
    <text x="15" y="105" fill="#f8fafc" font-size="12">     phone : TEXT</text>
    <text x="15" y="130" fill="#f8fafc" font-size="12">     party_type : CUSTOMER | VENDOR</text>
    <text x="15" y="155" fill="#f8fafc" font-size="12">     notes : TEXT</text>
  </g>

  <!-- 2. PURCHASE_TRANSACTION -->
  <g transform="translate(410, 120)" filter="url(#boxShadow)">
    <rect width="260" height="230" rx="8" fill="#1e293b" stroke="#475569" stroke-width="1.5"/>
    <rect width="260" height="32" rx="8" fill="#0284c7"/>
    <text x="15" y="21" fill="#fff" font-size="13" font-weight="700">PURCHASE_TRANSACTION</text>
    <text x="15" y="55" fill="#f8fafc" font-size="12"><tspan fill="#38bdf8" font-weight="600">PK</tspan>  id : INTEGER</text>
    <text x="15" y="80" fill="#f8fafc" font-size="12">     transaction_code : TEXT UNIQUE</text>
    <text x="15" y="105" fill="#f8fafc" font-size="12">     transaction_date : TEXT (ISO)</text>
    <text x="15" y="130" fill="#f8fafc" font-size="12"><tspan fill="#a855f7" font-weight="600">FK</tspan>  party_id : INTEGER (NULLABLE)</text>
    <text x="15" y="155" fill="#f8fafc" font-size="12">     payment_mode : CASH | CREDIT</text>
    <text x="15" y="180" fill="#f8fafc" font-size="12">     payment_status : PAID | UNPAID</text>
    <text x="15" y="205" fill="#f8fafc" font-size="12">     total_amount : REAL (₹)</text>
  </g>

  <!-- 3. SALE_TRANSACTION -->
  <g transform="translate(790, 120)" filter="url(#boxShadow)">
    <rect width="260" height="230" rx="8" fill="#1e293b" stroke="#475569" stroke-width="1.5"/>
    <rect width="260" height="32" rx="8" fill="#059669"/>
    <text x="15" y="21" fill="#fff" font-size="13" font-weight="700">SALE_TRANSACTION</text>
    <text x="15" y="55" fill="#f8fafc" font-size="12"><tspan fill="#38bdf8" font-weight="600">PK</tspan>  id : INTEGER</text>
    <text x="15" y="80" fill="#f8fafc" font-size="12">     transaction_code : TEXT UNIQUE</text>
    <text x="15" y="105" fill="#f8fafc" font-size="12">     transaction_date : TEXT (ISO)</text>
    <text x="15" y="130" fill="#f8fafc" font-size="12"><tspan fill="#a855f7" font-weight="600">FK</tspan>  party_id : INTEGER (NULLABLE)</text>
    <text x="15" y="155" fill="#f8fafc" font-size="12">     payment_mode : CASH | CREDIT</text>
    <text x="15" y="180" fill="#f8fafc" font-size="12">     payment_status : PAID | UNPAID</text>
    <text x="15" y="205" fill="#f8fafc" font-size="12">     total_amount : REAL (₹)</text>
  </g>

  <!-- 4. ITEM -->
  <g transform="translate(40, 420)" filter="url(#boxShadow)">
    <rect width="250" height="235" rx="8" fill="#1e293b" stroke="#475569" stroke-width="1.5"/>
    <rect width="250" height="32" rx="8" fill="#10b981"/>
    <text x="15" y="21" fill="#fff" font-size="13" font-weight="700">ITEM (Inventory Master)</text>
    <text x="15" y="55" fill="#f8fafc" font-size="12"><tspan fill="#38bdf8" font-weight="600">PK</tspan>  id : INTEGER</text>
    <text x="15" y="80" fill="#f8fafc" font-size="12">     name : TEXT UNIQUE NOT NULL</text>
    <text x="15" y="105" fill="#f8fafc" font-size="12">     category : TEXT</text>
    <text x="15" y="130" fill="#f8fafc" font-size="12">     barcode : TEXT (INDEXED)</text>
    <text x="15" y="155" fill="#f8fafc" font-size="12">     unit : TEXT DEFAULT 'PCS'</text>
    <text x="15" y="180" fill="#f8fafc" font-size="12">     default_sale_price : REAL (₹)</text>
    <text x="15" y="205" fill="#f8fafc" font-size="12">     current_stock : REAL</text>
    <text x="15" y="225" fill="#f8fafc" font-size="12">     is_low_stock : BOOLEAN (Stock &lt; 5)</text>
  </g>

  <!-- 5. PURCHASE_ITEM -->
  <g transform="translate(410, 400)" filter="url(#boxShadow)">
    <rect width="260" height="235" rx="8" fill="#1e293b" stroke="#475569" stroke-width="1.5"/>
    <rect width="260" height="32" rx="8" fill="#0284c7"/>
    <text x="15" y="21" fill="#fff" font-size="13" font-weight="700">PURCHASE_ITEM (FIFO Lots)</text>
    <text x="15" y="55" fill="#f8fafc" font-size="12"><tspan fill="#38bdf8" font-weight="600">PK</tspan>  id : INTEGER</text>
    <text x="15" y="80" fill="#f8fafc" font-size="12"><tspan fill="#a855f7" font-weight="600">FK</tspan>  purchase_transaction_id : INT</text>
    <text x="15" y="105" fill="#f8fafc" font-size="12"><tspan fill="#a855f7" font-weight="600">FK</tspan>  item_id : INTEGER</text>
    <text x="15" y="130" fill="#f8fafc" font-size="12">     quantity : REAL</text>
    <text x="15" y="155" fill="#f8fafc" font-size="12">     unit_cost : REAL (₹)</text>
    <text x="15" y="180" fill="#f8fafc" font-size="12">     total_amount : REAL (₹)</text>
    <text x="15" y="205" fill="#f59e0b" font-weight="600" font-size="12">     remaining_quantity : REAL</text>
  </g>

  <!-- 6. SALE_ITEM -->
  <g transform="translate(790, 400)" filter="url(#boxShadow)">
    <rect width="260" height="210" rx="8" fill="#1e293b" stroke="#475569" stroke-width="1.5"/>
    <rect width="260" height="32" rx="8" fill="#059669"/>
    <text x="15" y="21" fill="#fff" font-size="13" font-weight="700">SALE_ITEM (Outward Lines)</text>
    <text x="15" y="55" fill="#f8fafc" font-size="12"><tspan fill="#38bdf8" font-weight="600">PK</tspan>  id : INTEGER</text>
    <text x="15" y="80" fill="#f8fafc" font-size="12"><tspan fill="#a855f7" font-weight="600">FK</tspan>  sale_transaction_id : INT</text>
    <text x="15" y="105" fill="#f8fafc" font-size="12"><tspan fill="#a855f7" font-weight="600">FK</tspan>  item_id : INTEGER</text>
    <text x="15" y="130" fill="#f8fafc" font-size="12">     quantity : REAL</text>
    <text x="15" y="155" fill="#f8fafc" font-size="12">     unit_price : REAL (₹)</text>
    <text x="15" y="180" fill="#f8fafc" font-size="12">     total_amount : REAL (₹)</text>
  </g>

  <!-- 7. SALE_STOCK_ALLOCATION -->
  <g transform="translate(580, 680)" filter="url(#boxShadow)">
    <rect width="290" height="160" rx="8" fill="#1e293b" stroke="#f59e0b" stroke-width="1.5"/>
    <rect width="290" height="32" rx="8" fill="#d97706"/>
    <text x="15" y="21" fill="#fff" font-size="13" font-weight="700">SALE_STOCK_ALLOCATION (COGS)</text>
    <text x="15" y="55" fill="#f8fafc" font-size="12"><tspan fill="#38bdf8" font-weight="600">PK</tspan>  id : INTEGER</text>
    <text x="15" y="80" fill="#f8fafc" font-size="12"><tspan fill="#a855f7" font-weight="600">FK</tspan>  sale_item_id : INTEGER</text>
    <text x="15" y="105" fill="#f8fafc" font-size="12"><tspan fill="#a855f7" font-weight="600">FK</tspan>  purchase_item_id : INTEGER</text>
    <text x="15" y="130" fill="#f8fafc" font-size="12">     quantity_depleted : REAL</text>
    <text x="15" y="150" fill="#f8fafc" font-size="12">     unit_cost : REAL (₹ batch cost)</text>
  </g>

  <!-- 8. PAYMENT_SETTLEMENT -->
  <g transform="translate(900, 680)" filter="url(#boxShadow)">
    <rect width="260" height="185" rx="8" fill="#1e293b" stroke="#475569" stroke-width="1.5"/>
    <rect width="260" height="32" rx="8" fill="#8b5cf6"/>
    <text x="15" y="21" fill="#fff" font-size="13" font-weight="700">PAYMENT_SETTLEMENT</text>
    <text x="15" y="55" fill="#f8fafc" font-size="12"><tspan fill="#38bdf8" font-weight="600">PK</tspan>  id : INTEGER</text>
    <text x="15" y="80" fill="#f8fafc" font-size="12">     settlement_date : TEXT (ISO)</text>
    <text x="15" y="105" fill="#f8fafc" font-size="12"><tspan fill="#a855f7" font-weight="600">FK</tspan>  party_id : INTEGER</text>
    <text x="15" y="130" fill="#f8fafc" font-size="12">     transaction_type : TEXT</text>
    <text x="15" y="150" fill="#f8fafc" font-size="12">     amount_paid : REAL (₹)</text>
    <text x="15" y="170" fill="#f8fafc" font-size="12">     payment_mode : CASH | BANK</text>
  </g>

  <!-- 9. RETURN_TRANSACTION -->
  <g transform="translate(40, 720)" filter="url(#boxShadow)">
    <rect width="250" height="200" rx="8" fill="#1e293b" stroke="#475569" stroke-width="1.5"/>
    <rect width="250" height="32" rx="8" fill="#db2777"/>
    <text x="15" y="21" fill="#fff" font-size="13" font-weight="700">RETURN_TRANSACTION</text>
    <text x="15" y="55" fill="#f8fafc" font-size="12"><tspan fill="#38bdf8" font-weight="600">PK</tspan>  id : INTEGER</text>
    <text x="15" y="80" fill="#f8fafc" font-size="12">     return_code : TEXT UNIQUE</text>
    <text x="15" y="105" fill="#f8fafc" font-size="12">     return_type : CUSTOMER | VENDOR</text>
    <text x="15" y="130" fill="#f8fafc" font-size="12">     settlement_type : REFUND | DUE</text>
    <text x="15" y="155" fill="#f8fafc" font-size="12">     total_amount : REAL (₹)</text>
    <text x="15" y="180" fill="#f8fafc" font-size="12">     reason : DEFECTIVE | DAMAGED</text>
  </g>

  <!-- 10. RETURN_ITEM -->
  <g transform="translate(320, 720)" filter="url(#boxShadow)">
    <rect width="230" height="175" rx="8" fill="#1e293b" stroke="#475569" stroke-width="1.5"/>
    <rect width="230" height="32" rx="8" fill="#db2777"/>
    <text x="15" y="21" fill="#fff" font-size="13" font-weight="700">RETURN_ITEM</text>
    <text x="15" y="55" fill="#f8fafc" font-size="12"><tspan fill="#38bdf8" font-weight="600">PK</tspan>  id : INTEGER</text>
    <text x="15" y="80" fill="#f8fafc" font-size="12"><tspan fill="#a855f7" font-weight="600">FK</tspan>  return_transaction_id : INT</text>
    <text x="15" y="105" fill="#f8fafc" font-size="12"><tspan fill="#a855f7" font-weight="600">FK</tspan>  item_id : INTEGER</text>
    <text x="15" y="130" fill="#f8fafc" font-size="12">     quantity : REAL</text>
    <text x="15" y="155" fill="#f8fafc" font-size="12">     condition : RESTOCK | SCRAP</text>
  </g>
</svg>"""
    with open(os.path.join(DOCS_IMG_DIR, "entity_relationship_diagram.svg"), "w", encoding="utf-8") as f:
        f.write(svg)
    print("Generated entity_relationship_diagram.svg successfully.")

if __name__ == "__main__":
    generate_system_arch_svg()
    generate_erd_svg()
