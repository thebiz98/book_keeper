"""
Generates clean, simple, easy-to-read SVGs that match the exact simple diagram layout from the chat.
"""
import os

DOCS_IMG_DIR = r"c:\custom_apps\book_keeper\docs\images"
os.makedirs(DOCS_IMG_DIR, exist_ok=True)

def generate_simple_system_arch_svg():
    svg = """<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 900 680" width="100%" height="100%" style="background:#ffffff; font-family:'Segoe UI',Roboto,Helvetica,Arial,sans-serif;">
  <defs>
    <marker id="arrow" viewBox="0 0 10 10" refX="6" refY="5" markerWidth="6" markerHeight="6" orient="auto-start-reverse">
      <path d="M 0 1 L 8 5 L 0 9 z" fill="#334155"/>
    </marker>
    <marker id="arrowDashed" viewBox="0 0 10 10" refX="6" refY="5" markerWidth="6" markerHeight="6" orient="auto-start-reverse">
      <path d="M 0 1 L 8 5 L 0 9 z" fill="#64748b"/>
    </marker>
  </defs>

  <!-- Title -->
  <text x="450" y="35" text-anchor="middle" fill="#0f172a" font-size="20" font-weight="700">System Architecture Flow</text>

  <!-- ================= UI SUBGRAPH ================= -->
  <rect x="40" y="60" width="820" height="150" rx="8" fill="#f8fafc" stroke="#cbd5e1" stroke-width="1.5"/>
  <text x="60" y="85" fill="#475569" font-size="14" font-weight="700">Windows Desktop UI</text>

  <!-- UI Nodes -->
  <!-- Home -->
  <rect x="60" y="105" width="180" height="85" rx="6" fill="#ffffff" stroke="#94a3b8" stroke-width="1.5"/>
  <text x="150" y="130" text-anchor="middle" fill="#0f172a" font-size="13" font-weight="600">Home Dashboard</text>
  <text x="150" y="152" text-anchor="middle" fill="#475569" font-size="11.5">Low Stock (&lt; 5)</text>
  <text x="150" y="170" text-anchor="middle" fill="#475569" font-size="11.5">Stock Value, Dues, Sales</text>

  <!-- Purchase Screen -->
  <rect x="260" y="105" width="170" height="85" rx="6" fill="#ffffff" stroke="#94a3b8" stroke-width="1.5"/>
  <text x="345" y="130" text-anchor="middle" fill="#0f172a" font-size="13" font-weight="600">Add Purchase</text>
  <text x="345" y="152" text-anchor="middle" fill="#475569" font-size="11.5">Item, Qty, Cost</text>
  <text x="345" y="170" text-anchor="middle" fill="#475569" font-size="11.5">Cash / Credit / Partial</text>

  <!-- Sale Screen -->
  <rect x="450" y="105" width="170" height="85" rx="6" fill="#ffffff" stroke="#94a3b8" stroke-width="1.5"/>
  <text x="535" y="130" text-anchor="middle" fill="#0f172a" font-size="13" font-weight="600">Add Sale</text>
  <text x="535" y="152" text-anchor="middle" fill="#475569" font-size="11.5">Item, Qty, Price</text>
  <text x="535" y="170" text-anchor="middle" fill="#475569" font-size="11.5">Cash / Credit / Partial</text>

  <!-- Reports Screen -->
  <rect x="640" y="105" width="200" height="85" rx="6" fill="#ffffff" stroke="#94a3b8" stroke-width="1.5"/>
  <text x="740" y="130" text-anchor="middle" fill="#0f172a" font-size="13" font-weight="600">Reports &amp; Dues</text>
  <text x="740" y="152" text-anchor="middle" fill="#475569" font-size="11.5">Day / Week / Month / Year</text>
  <text x="740" y="170" text-anchor="middle" fill="#475569" font-size="11.5">Customer &amp; Vendor Dues</text>

  <!-- Connector Lines UI -> Logic -->
  <path d="M 345 190 L 345 285" stroke="#334155" stroke-width="1.5" fill="none" marker-end="url(#arrow)"/>
  <path d="M 535 190 L 535 250" stroke="#334155" stroke-width="1.5" fill="none" marker-end="url(#arrow)"/>

  <!-- ================= BUSINESS LOGIC SUBGRAPH ================= -->
  <rect x="40" y="235" width="820" height="150" rx="8" fill="#f8fafc" stroke="#cbd5e1" stroke-width="1.5"/>
  <text x="60" y="260" fill="#475569" font-size="14" font-weight="700">Business Logic Layer</text>

  <!-- Stock Check -->
  <rect x="450" y="275" width="170" height="65" rx="6" fill="#ffffff" stroke="#94a3b8" stroke-width="1.5"/>
  <text x="535" y="302" text-anchor="middle" fill="#0f172a" font-size="13" font-weight="600">Stock Validator</text>
  <text x="535" y="322" text-anchor="middle" fill="#475569" font-size="11.5">Prevents negative stock</text>

  <!-- FIFO Engine -->
  <rect x="250" y="275" width="180" height="80" rx="6" fill="#ffffff" stroke="#94a3b8" stroke-width="1.5"/>
  <text x="340" y="300" text-anchor="middle" fill="#0f172a" font-size="13" font-weight="600">FIFO Engine</text>
  <text x="340" y="320" text-anchor="middle" fill="#475569" font-size="11.5">Finds oldest purchase</text>
  <text x="340" y="338" text-anchor="middle" fill="#475569" font-size="11.5">with remaining_qty &gt; 0</text>

  <!-- Metrics Aggregator -->
  <rect x="60" y="275" width="170" height="80" rx="6" fill="#ffffff" stroke="#94a3b8" stroke-width="1.5"/>
  <text x="145" y="300" text-anchor="middle" fill="#0f172a" font-size="13" font-weight="600">Metrics Calculator</text>
  <text x="145" y="320" text-anchor="middle" fill="#475569" font-size="11.5">Stock Value &amp; Profit</text>
  <text x="145" y="338" text-anchor="middle" fill="#475569" font-size="11.5">Receivables &amp; Payables</text>

  <!-- Settlements Manager -->
  <rect x="640" y="275" width="200" height="65" rx="6" fill="#ffffff" stroke="#94a3b8" stroke-width="1.5"/>
  <text x="740" y="302" text-anchor="middle" fill="#0f172a" font-size="13" font-weight="600">Settlement Manager</text>
  <text x="740" y="322" text-anchor="middle" fill="#475569" font-size="11.5">Clears Partial / Credit Dues</text>

  <!-- Internal Connectors -->
  <path d="M 450 310 L 430 310" stroke="#334155" stroke-width="1.5" fill="none" marker-end="url(#arrow)"/>

  <!-- ================= DATABASE SUBGRAPH ================= -->
  <rect x="40" y="415" width="820" height="230" rx="8" fill="#f8fafc" stroke="#cbd5e1" stroke-width="1.5"/>
  <text x="60" y="440" fill="#475569" font-size="14" font-weight="700">Local Embedded SQLite (shop.db)</text>

  <!-- Tables -->
  <!-- ITEM -->
  <rect x="60" y="460" width="130" height="60" rx="6" fill="#ffffff" stroke="#64748b" stroke-width="1.5"/>
  <text x="125" y="488" text-anchor="middle" fill="#0f172a" font-size="13" font-weight="600">ITEM</text>
  <text x="125" y="508" text-anchor="middle" fill="#475569" font-size="11">Stock &amp; Low-Stock</text>

  <!-- PARTY -->
  <rect x="60" y="535" width="130" height="60" rx="6" fill="#ffffff" stroke="#64748b" stroke-width="1.5"/>
  <text x="125" y="563" text-anchor="middle" fill="#0f172a" font-size="13" font-weight="600">PARTY</text>
  <text x="125" y="583" text-anchor="middle" fill="#475569" font-size="11">Customer &amp; Vendor</text>

  <!-- PURCHASE -->
  <rect x="220" y="460" width="180" height="75" rx="6" fill="#ffffff" stroke="#64748b" stroke-width="1.5"/>
  <text x="310" y="485" text-anchor="middle" fill="#0f172a" font-size="13" font-weight="600">PURCHASE</text>
  <text x="310" y="505" text-anchor="middle" fill="#475569" font-size="11">Transaction Header</text>
  <text x="310" y="523" text-anchor="middle" fill="#2563eb" font-size="11">&amp; FIFO Stock Lots</text>

  <!-- SALE -->
  <rect x="430" y="460" width="170" height="75" rx="6" fill="#ffffff" stroke="#64748b" stroke-width="1.5"/>
  <text x="515" y="485" text-anchor="middle" fill="#0f172a" font-size="13" font-weight="600">SALE</text>
  <text x="515" y="505" text-anchor="middle" fill="#475569" font-size="11">Transaction Header</text>
  <text x="515" y="523" text-anchor="middle" fill="#475569" font-size="11">&amp; Sale Items</text>

  <!-- DEPLETION / ALLOCATION -->
  <rect x="270" y="560" width="280" height="65" rx="6" fill="#ffffff" stroke="#64748b" stroke-width="1.5"/>
  <text x="410" y="588" text-anchor="middle" fill="#0f172a" font-size="13" font-weight="600">SALE_STOCK_ALLOCATION</text>
  <text x="410" y="608" text-anchor="middle" fill="#475569" font-size="11.5">Links Sale to Purchase Lots (COGS &amp; Profit)</text>

  <!-- SETTLEMENT -->
  <rect x="630" y="460" width="210" height="60" rx="6" fill="#ffffff" stroke="#64748b" stroke-width="1.5"/>
  <text x="735" y="488" text-anchor="middle" fill="#0f172a" font-size="13" font-weight="600">PAYMENT_SETTLEMENT</text>
  <text x="735" y="508" text-anchor="middle" fill="#475569" font-size="11">Partial &amp; Full Dues Logs</text>

  <!-- RETURNS & ADJUSTMENTS -->
  <rect x="630" y="535" width="210" height="60" rx="6" fill="#ffffff" stroke="#64748b" stroke-width="1.5"/>
  <text x="735" y="563" text-anchor="middle" fill="#0f172a" font-size="13" font-weight="600">RETURNS &amp; ADJUSTMENTS</text>
  <text x="735" y="583" text-anchor="middle" fill="#475569" font-size="11">Returns, Swaps &amp; Damage</text>

  <!-- Connectors Logic -> DB -->
  <path d="M 340 355 L 340 460" stroke="#334155" stroke-width="1.5" fill="none" marker-end="url(#arrow)"/>
  <path d="M 535 340 L 535 460" stroke="#334155" stroke-width="1.5" fill="none" marker-end="url(#arrow)"/>
  <path d="M 740 340 L 740 460" stroke="#334155" stroke-width="1.5" fill="none" marker-end="url(#arrow)"/>
  <path d="M 145 460 L 145 355" stroke="#334155" stroke-width="1.5" fill="none" marker-end="url(#arrow)"/>
  <path d="M 145 275 L 145 190" stroke="#334155" stroke-width="1.5" fill="none" marker-end="url(#arrow)"/>
</svg>"""
    with open(os.path.join(DOCS_IMG_DIR, "system_architecture.svg"), "w", encoding="utf-8") as f:
        f.write(svg)
    print("Generated simple system_architecture.svg")

def generate_simple_erd_svg():
    svg = """<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1020 920" width="100%" height="100%" style="background:#ffffff; font-family:'Segoe UI',Roboto,Helvetica,Arial,sans-serif;">
  <defs>
    <marker id="circleEnd" viewBox="0 0 10 10" refX="5" refY="5" markerWidth="6" markerHeight="6">
      <circle cx="5" cy="5" r="3" fill="#ffffff" stroke="#475569" stroke-width="1.5"/>
    </marker>
  </defs>

  <!-- Title -->
  <text x="510" y="32" text-anchor="middle" fill="#0f172a" font-size="20" font-weight="700">Entity-Relationship Diagram</text>

  <!-- Relation Lines -->
  <!-- PARTY to PURCHASE_TRANSACTION -->
  <line x1="260" y1="130" x2="350" y2="130" stroke="#64748b" stroke-width="1.5"/>
  <!-- PARTY to SALE_TRANSACTION -->
  <line x1="260" y1="170" x2="690" y2="170" stroke="#64748b" stroke-width="1.5"/>
  <!-- PURCHASE_TRANSACTION to PURCHASE_ITEM -->
  <line x1="490" y1="280" x2="490" y2="330" stroke="#64748b" stroke-width="1.5"/>
  <!-- SALE_TRANSACTION to SALE_ITEM -->
  <line x1="830" y1="280" x2="830" y2="330" stroke="#64748b" stroke-width="1.5"/>
  <!-- ITEM to PURCHASE_ITEM -->
  <line x1="240" y1="420" x2="350" y2="420" stroke="#64748b" stroke-width="1.5"/>
  <!-- ITEM to SALE_ITEM -->
  <line x1="240" y1="450" x2="690" y2="420" stroke="#64748b" stroke-width="1.5"/>
  <!-- PURCHASE_ITEM / SALE_ITEM to SALE_STOCK_ALLOCATION -->
  <line x1="490" y1="520" x2="590" y2="580" stroke="#64748b" stroke-width="1.5"/>
  <line x1="830" y1="500" x2="670" y2="580" stroke="#64748b" stroke-width="1.5"/>
  <!-- RETURN_TRANSACTION to RETURN_ITEM -->
  <line x1="270" y1="780" x2="340" y2="780" stroke="#64748b" stroke-width="1.5"/>

  <!-- ================= ENTITY BOXES ================= -->

  <!-- 1. PARTY -->
  <g transform="translate(40, 70)">
    <rect width="220" height="150" rx="4" fill="#ffffff" stroke="#334155" stroke-width="1.5"/>
    <rect width="220" height="28" fill="#f1f5f9" stroke="#334155" stroke-width="1.5"/>
    <text x="110" y="19" text-anchor="middle" fill="#0f172a" font-size="12" font-weight="700">PARTY</text>
    <text x="12" y="48" fill="#0f172a" font-size="11"><b>PK</b> id : int</text>
    <text x="12" y="68" fill="#0f172a" font-size="11">name : string</text>
    <text x="12" y="88" fill="#0f172a" font-size="11">phone : string</text>
    <text x="12" y="108" fill="#0f172a" font-size="11">party_type : CUSTOMER / VENDOR</text>
    <text x="12" y="128" fill="#0f172a" font-size="11">notes : string</text>
  </g>

  <!-- 2. PURCHASE_TRANSACTION -->
  <g transform="translate(350, 60)">
    <rect width="280" height="220" rx="4" fill="#ffffff" stroke="#334155" stroke-width="1.5"/>
    <rect width="280" height="28" fill="#f1f5f9" stroke="#334155" stroke-width="1.5"/>
    <text x="140" y="19" text-anchor="middle" fill="#0f172a" font-size="12" font-weight="700">PURCHASE_TRANSACTION</text>
    <text x="12" y="48" fill="#0f172a" font-size="11"><b>PK</b> id : int</text>
    <text x="12" y="68" fill="#0f172a" font-size="11">transaction_code : string</text>
    <text x="12" y="88" fill="#0f172a" font-size="11">transaction_date : date</text>
    <text x="12" y="108" fill="#0f172a" font-size="11"><b>FK</b> party_id : int (optional)</text>
    <text x="12" y="128" fill="#0f172a" font-size="11">payment_mode : CASH / CREDIT</text>
    <text x="12" y="148" fill="#0f172a" font-size="11">payment_status : PAID / PARTIAL / UNPAID</text>
    <text x="12" y="168" fill="#0f172a" font-size="11">total_amount : decimal</text>
    <text x="12" y="188" fill="#2563eb" font-size="11" font-weight="600">paid_amount : decimal</text>
    <text x="12" y="208" fill="#dc2626" font-size="11" font-weight="600">balance_due : decimal</text>
  </g>

  <!-- 3. SALE_TRANSACTION -->
  <g transform="translate(690, 60)">
    <rect width="280" height="220" rx="4" fill="#ffffff" stroke="#334155" stroke-width="1.5"/>
    <rect width="280" height="28" fill="#f1f5f9" stroke="#334155" stroke-width="1.5"/>
    <text x="140" y="19" text-anchor="middle" fill="#0f172a" font-size="12" font-weight="700">SALE_TRANSACTION</text>
    <text x="12" y="48" fill="#0f172a" font-size="11"><b>PK</b> id : int</text>
    <text x="12" y="68" fill="#0f172a" font-size="11">transaction_code : string</text>
    <text x="12" y="88" fill="#0f172a" font-size="11">transaction_date : date</text>
    <text x="12" y="108" fill="#0f172a" font-size="11"><b>FK</b> party_id : int (optional)</text>
    <text x="12" y="128" fill="#0f172a" font-size="11">payment_mode : CASH / CREDIT</text>
    <text x="12" y="148" fill="#0f172a" font-size="11">payment_status : PAID / PARTIAL / UNPAID</text>
    <text x="12" y="168" fill="#0f172a" font-size="11">total_amount : decimal</text>
    <text x="12" y="188" fill="#2563eb" font-size="11" font-weight="600">paid_amount : decimal</text>
    <text x="12" y="208" fill="#dc2626" font-size="11" font-weight="600">balance_due : decimal</text>
  </g>

  <!-- 4. ITEM -->
  <g transform="translate(40, 310)">
    <rect width="200" height="210" rx="4" fill="#ffffff" stroke="#334155" stroke-width="1.5"/>
    <rect width="200" height="28" fill="#f1f5f9" stroke="#334155" stroke-width="1.5"/>
    <text x="100" y="19" text-anchor="middle" fill="#0f172a" font-size="12" font-weight="700">ITEM</text>
    <text x="12" y="48" fill="#0f172a" font-size="11"><b>PK</b> id : int</text>
    <text x="12" y="68" fill="#0f172a" font-size="11">name : string</text>
    <text x="12" y="88" fill="#0f172a" font-size="11">category : string</text>
    <text x="12" y="108" fill="#0f172a" font-size="11">barcode : string</text>
    <text x="12" y="128" fill="#0f172a" font-size="11">unit : string</text>
    <text x="12" y="148" fill="#0f172a" font-size="11">default_sale_price : decimal</text>
    <text x="12" y="168" fill="#0f172a" font-size="11">current_stock : decimal</text>
    <text x="12" y="188" fill="#0f172a" font-size="11">is_low_stock : boolean (&lt; 5)</text>
  </g>

  <!-- 5. PURCHASE_ITEM -->
  <g transform="translate(350, 330)">
    <rect width="280" height="190" rx="4" fill="#ffffff" stroke="#334155" stroke-width="1.5"/>
    <rect width="280" height="28" fill="#f1f5f9" stroke="#334155" stroke-width="1.5"/>
    <text x="140" y="19" text-anchor="middle" fill="#0f172a" font-size="12" font-weight="700">PURCHASE_ITEM (FIFO Lots)</text>
    <text x="12" y="48" fill="#0f172a" font-size="11"><b>PK</b> id : int</text>
    <text x="12" y="68" fill="#0f172a" font-size="11"><b>FK</b> purchase_transaction_id : int</text>
    <text x="12" y="88" fill="#0f172a" font-size="11"><b>FK</b> item_id : int</text>
    <text x="12" y="108" fill="#0f172a" font-size="11">quantity : decimal</text>
    <text x="12" y="128" fill="#0f172a" font-size="11">unit_cost : decimal</text>
    <text x="12" y="148" fill="#0f172a" font-size="11">total_amount : decimal</text>
    <text x="12" y="168" fill="#d97706" font-size="11" font-weight="600">remaining_quantity : decimal</text>
  </g>

  <!-- 6. SALE_ITEM -->
  <g transform="translate(690, 330)">
    <rect width="280" height="170" rx="4" fill="#ffffff" stroke="#334155" stroke-width="1.5"/>
    <rect width="280" height="28" fill="#f1f5f9" stroke="#334155" stroke-width="1.5"/>
    <text x="140" y="19" text-anchor="middle" fill="#0f172a" font-size="12" font-weight="700">SALE_ITEM</text>
    <text x="12" y="48" fill="#0f172a" font-size="11"><b>PK</b> id : int</text>
    <text x="12" y="68" fill="#0f172a" font-size="11"><b>FK</b> sale_transaction_id : int</text>
    <text x="12" y="88" fill="#0f172a" font-size="11"><b>FK</b> item_id : int</text>
    <text x="12" y="108" fill="#0f172a" font-size="11">quantity : decimal</text>
    <text x="12" y="128" fill="#0f172a" font-size="11">unit_price : decimal</text>
    <text x="12" y="148" fill="#0f172a" font-size="11">total_amount : decimal</text>
  </g>

  <!-- 7. SALE_STOCK_ALLOCATION -->
  <g transform="translate(480, 560)">
    <rect width="280" height="150" rx="4" fill="#ffffff" stroke="#334155" stroke-width="1.5"/>
    <rect width="280" height="28" fill="#f1f5f9" stroke="#334155" stroke-width="1.5"/>
    <text x="140" y="19" text-anchor="middle" fill="#0f172a" font-size="12" font-weight="700">SALE_STOCK_ALLOCATION</text>
    <text x="12" y="48" fill="#0f172a" font-size="11"><b>PK</b> id : int</text>
    <text x="12" y="68" fill="#0f172a" font-size="11"><b>FK</b> sale_item_id : int</text>
    <text x="12" y="88" fill="#0f172a" font-size="11"><b>FK</b> purchase_item_id : int</text>
    <text x="12" y="108" fill="#0f172a" font-size="11">quantity_depleted : decimal</text>
    <text x="12" y="128" fill="#0f172a" font-size="11">unit_cost : decimal (for COGS)</text>
  </g>

  <!-- 8. PAYMENT_SETTLEMENT -->
  <g transform="translate(780, 560)">
    <rect width="200" height="170" rx="4" fill="#ffffff" stroke="#334155" stroke-width="1.5"/>
    <rect width="200" height="28" fill="#f1f5f9" stroke="#334155" stroke-width="1.5"/>
    <text x="100" y="19" text-anchor="middle" fill="#0f172a" font-size="12" font-weight="700">PAYMENT_SETTLEMENT</text>
    <text x="12" y="48" fill="#0f172a" font-size="11"><b>PK</b> id : int</text>
    <text x="12" y="68" fill="#0f172a" font-size="11">settlement_date : date</text>
    <text x="12" y="88" fill="#0f172a" font-size="11"><b>FK</b> party_id : int</text>
    <text x="12" y="108" fill="#0f172a" font-size="11">transaction_type : string</text>
    <text x="12" y="128" fill="#0f172a" font-size="11">amount_paid : decimal</text>
    <text x="12" y="148" fill="#0f172a" font-size="11">payment_mode : string</text>
  </g>

  <!-- 9. RETURN_TRANSACTION & RETURN_ITEM -->
  <g transform="translate(40, 700)">
    <rect width="230" height="190" rx="4" fill="#ffffff" stroke="#334155" stroke-width="1.5"/>
    <rect width="230" height="28" fill="#f1f5f9" stroke="#334155" stroke-width="1.5"/>
    <text x="115" y="19" text-anchor="middle" fill="#0f172a" font-size="12" font-weight="700">RETURN_TRANSACTION</text>
    <text x="12" y="48" fill="#0f172a" font-size="11"><b>PK</b> id : int</text>
    <text x="12" y="68" fill="#0f172a" font-size="11">return_code : string</text>
    <text x="12" y="88" fill="#0f172a" font-size="11">return_date : date</text>
    <text x="12" y="108" fill="#0f172a" font-size="11">return_type : CUSTOMER / VENDOR</text>
    <text x="12" y="128" fill="#0f172a" font-size="11">settlement_type : REFUND / DUE</text>
    <text x="12" y="148" fill="#0f172a" font-size="11">total_amount : decimal</text>
    <text x="12" y="168" fill="#0f172a" font-size="11">reason : string</text>
  </g>

  <g transform="translate(300, 700)">
    <rect width="210" height="170" rx="4" fill="#ffffff" stroke="#334155" stroke-width="1.5"/>
    <rect width="210" height="28" fill="#f1f5f9" stroke="#334155" stroke-width="1.5"/>
    <text x="105" y="19" text-anchor="middle" fill="#0f172a" font-size="12" font-weight="700">RETURN_ITEM</text>
    <text x="12" y="48" fill="#0f172a" font-size="11"><b>PK</b> id : int</text>
    <text x="12" y="68" fill="#0f172a" font-size="11"><b>FK</b> return_transaction_id : int</text>
    <text x="12" y="88" fill="#0f172a" font-size="11"><b>FK</b> item_id : int</text>
    <text x="12" y="108" fill="#0f172a" font-size="11">quantity : decimal</text>
    <text x="12" y="128" fill="#0f172a" font-size="11">unit_price : decimal</text>
    <text x="12" y="148" fill="#0f172a" font-size="11">condition : RESTOCK / SCRAP</text>
  </g>

  <!-- 10. STOCK_ADJUSTMENT & SHOP_SETTING -->
  <g transform="translate(540, 750)">
    <rect width="220" height="130" rx="4" fill="#ffffff" stroke="#334155" stroke-width="1.5"/>
    <rect width="220" height="28" fill="#f1f5f9" stroke="#334155" stroke-width="1.5"/>
    <text x="110" y="19" text-anchor="middle" fill="#0f172a" font-size="12" font-weight="700">STOCK_ADJUSTMENT</text>
    <text x="12" y="48" fill="#0f172a" font-size="11"><b>PK</b> id : int</text>
    <text x="12" y="68" fill="#0f172a" font-size="11">adjustment_date : date</text>
    <text x="12" y="88" fill="#0f172a" font-size="11"><b>FK</b> item_id : int (Lookup)</text>
    <text x="12" y="108" fill="#0f172a" font-size="11">quantity : decimal (+ / -)</text>
  </g>

  <g transform="translate(780, 750)">
    <rect width="200" height="130" rx="4" fill="#ffffff" stroke="#334155" stroke-width="1.5"/>
    <rect width="200" height="28" fill="#f1f5f9" stroke="#334155" stroke-width="1.5"/>
    <text x="100" y="19" text-anchor="middle" fill="#0f172a" font-size="12" font-weight="700">SHOP_SETTING</text>
    <text x="12" y="48" fill="#0f172a" font-size="11"><b>PK</b> id : int</text>
    <text x="12" y="68" fill="#0f172a" font-size="11">shop_name : string</text>
    <text x="12" y="88" fill="#0f172a" font-size="11">phone : string</text>
    <text x="12" y="108" fill="#0f172a" font-size="11">tax_id : string</text>
  </g>
</svg>"""
    with open(os.path.join(DOCS_IMG_DIR, "entity_relationship_diagram.svg"), "w", encoding="utf-8") as f:
        f.write(svg)
    print("Generated simple entity_relationship_diagram.svg")

if __name__ == "__main__":
    generate_simple_system_arch_svg()
    generate_simple_erd_svg()
