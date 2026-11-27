/**
 * Core domain models for Book Keeper.
 * camelCase mirrors of src-tauri/src/core/entities & dtos. The IPC layer
 * (src/api/caseConvert.ts) converts Rust snake_case keys to camelCase.
 * Enums are SCREAMING_SNAKE_CASE strings (DD-23).
 */

export * from './store';
export * from './api';

// Enum string literals are SCREAMING_SNAKE_CASE, identical to the Rust serde names and the
// SQLite TEXT values (DD-23). Do not change one side without the others.
export type PartyType = 'CUSTOMER' | 'VENDOR';
export type ItemType = 'PHYSICAL' | 'SERVICE';
export type PaymentStatus = 'PAID' | 'PARTIAL' | 'UNPAID';
export type AdjustmentReason = 'DAMAGED' | 'EXPIRED' | 'COUNT_CORRECTION' | 'THEFT_LOST' | 'UNRECORDED_STOCK';
/** Refund = value applied to dues first, remainder as cash. Replacement = customer-only no-money swap. */
export type SettlementType = 'REFUND' | 'REPLACEMENT';
export type StockCondition = 'RESTOCKABLE' | 'DAMAGED_SCRAP';
export type ReturnType = 'CUSTOMER' | 'VENDOR';
export type ReturnLineType = 'RETURNED' | 'REPLACEMENT_ISSUED';
export type PaymentKind = 'UPFRONT' | 'SETTLEMENT' | 'RETURN_CREDIT' | 'REFUND_OUT' | 'REFUND_IN';
export type TransactionType = 'SALE' | 'PURCHASE';
export type LotSource = 'PURCHASE' | 'ADJUSTMENT' | 'OPENING' | 'RETURN';
export type OutflowSourceType = 'ADJUSTMENT' | 'REPLACEMENT' | 'VENDOR_RETURN';

export interface ItemBundle {
  id: number;
  itemId: number;
  bundleName: string;
  unitsPerBundle: number;
  mrp: number | null;
  defaultBundlePrice: number | null;
  barcode: string | null;
}

/** create_bundle / update_bundle result: the saved bundle plus the backend's suggested rate (DD-56). */
export interface SavedBundle extends ItemBundle {
  /** Suggested sale rate per pack: the bundle price, else item price x units (backend-computed). */
  defaultRate: number;
}

export interface ItemBundleSearchResult {
  bundleId: number;
  bundleName: string;
  unitsPerBundle: number;
  bundlePrice: number | null;
  /** Suggested sale rate per pack (backend-computed, DD-56). */
  defaultRate: number;
  barcode: string | null;
}

/** Catalog + search row (get_items_catalog, search_items_fts). */
export interface ItemSearchResult {
  itemId: number;
  name: string;
  category: string | null;
  barcode: string | null;
  baseUnit: string;
  itemType: ItemType;
  mrp: number | null;
  defaultSalePrice: number;
  currentStock: number;
  isLowStock: boolean;
  /** Per-item low stock level in base units (0 = never low). */
  lowStockThreshold: number;
  bundles: ItemBundleSearchResult[];
  /** Set when the query matched a bundle barcode. */
  matchedBundleId: number | null;
  /** SERVICE only (FR-36): intended cost per unit. */
  serviceCost?: number | null;
  /** SERVICE only (FR-36): "Can make N" from component stock; null without components. */
  canMake?: number | null;
  /** FR-45: false for a deleted item (opened read-only from history). Absent = active. */
  isActive?: boolean;
}

/** FR-45 A1 (DD-79): HARD = never used, removed permanently; SOFT = moved to Deleted items. */
export type DeleteMode = 'HARD' | 'SOFT';

/** FR-45: backend dry run of a delete. Every figure and sentence comes from the backend (DD-56). */
export interface ItemDeletePreview {
  itemId: number;
  name: string;
  itemType: ItemType;
  stock: number;
  baseUnit: string;
  stockValue: number;
  blockingServices: string[];
  canDelete: boolean;
  reason: string | null;
  mode: DeleteMode;
  reasonsUsed: string[];
  /** FR-58 (DD-93): what "Book as loss" would write off; null without stock, for HARD or when blocked. */
  writeOff?: WriteOffPreview | null;
}

/** FR-58: the whole stock at its FIFO cost, from the backend dry run. */
export interface WriteOffPreview {
  quantity: number;
  baseUnit: string;
  loss: number;
}

/** `delete_item` result. `adjustmentId` is the write-off adjustment (FR-58), null for "Just delete". */
export interface DeleteItemResult {
  mode: DeleteMode;
  adjustmentId?: number | null;
}

/** One row of the Deleted items list (FR-45). */
export interface DeletedItemRow {
  itemId: number;
  name: string;
  itemType: ItemType;
  category: string | null;
  baseUnit: string;
  currentStock: number;
  stockValue: number;
  deletedAt: string;
  /** A1: never used, so "Delete permanently" is offered. */
  canPurge?: boolean;
}

/** FR-46 (DD-80): backend dry run of a party delete. Every figure and sentence comes from the backend. */
export interface PartyDeletePreview {
  partyId: number;
  name: string;
  partyType: PartyType;
  mode: DeleteMode;
  openBillsCount: number;
  balanceDueTotal: number;
  canDelete: boolean;
  reason: string | null;
  reasonsUsed: string[];
}

/** One row of the Deleted parties list (customers and vendors together). */
export interface DeletedPartyRow {
  partyId: number;
  name: string;
  partyType: PartyType;
  phone: string | null;
  deletedAt: string;
}

/** One component of a service (FR-36): base units of a physical item per service unit. */
export interface ServiceComponentInput {
  componentItemId: number;
  quantityPerUnit: number;
}

/** Component row of a service for the edit drawer (get_service_components). */
export interface ServiceComponentView {
  componentItemId: number;
  name: string;
  baseUnit: string;
  quantityPerUnit: number;
  currentStock: number;
}

/** preview_service_cost input / result (FR-36, DD-56). */
export interface PreviewServiceCostPayload {
  components: ServiceComponentInput[];
  serviceCost: number | null;
}
export interface ServiceCostPreview {
  componentsCost: number;
  miscCost: number;
  unitCost: number;
}

/** Base units of an earlier cart line (FR-31 / FR-37); the backend expands services. */
export interface PriorLine {
  itemId: number;
  quantity: number;
}

/** One component's draw inside a service line's lot draw (FR-37). */
export interface LotDrawComponent {
  itemId: number;
  name: string;
  baseUnit: string;
  quantity: number;
  lots: LotDrawLot[];
  drawCost: number;
  available: number;
  shortfall: number;
}

/** One open lot in a sale-row lot draw (FR-31). `drawQuantity` is the part of the entered quantity taken from it. */
export interface LotDrawLot {
  purchaseItemId: number;
  transactionCode: string;
  source: LotSource;
  unitCost: number;
  remainingQuantity: number;
  drawQuantity: number;
}

/** Read-only FIFO draw for the sale entry row (FR-31, DD-60). */
export interface LotDraw {
  lots: LotDrawLot[];
  drawCost: number;
  available: number;
  shortfall: number;
  /** Service lines (FR-37): per-component draws; empty for a physical item. */
  components?: LotDrawComponent[];
  componentsCost?: number;
  miscCost?: number;
  /** Physical: drawCost. Service: componentsCost + miscCost. */
  lineCost?: number;
}

export interface ItemLot {
  purchaseItemId: number;
  purchaseTransactionId: number;
  transactionCode: string;
  source: LotSource;
  transactionDate: string;
  vendorName: string | null;
  /** Pack the lot was entered in (null = base units). */
  packLabel: string | null;
  packQuantity: number;
  quantity: number;
  remainingQuantity: number;
  unitCost: number;
}

/** FR-23: one bill that contains an item (Item details Sales / Purchases tabs). */
export interface ItemBillRef {
  billId: number;
  transactionCode: string;
  transactionDate: string;
  partyName: string | null;
}

export interface LowStockItem {
  id: number;
  name: string;
  category: string | null;
  baseUnit: string;
  currentStock: number;
  defaultSalePrice: number;
  mrp: number | null;
  lowStockThreshold: number;
}

export interface Party {
  id: number;
  name: string;
  phone: string | null;
  partyType: PartyType;
  notes: string | null;
  isActive: boolean;
  createdAt: string;
  updatedAt: string;
  /** The party's opening bill (FR-35); null / absent when none. */
  openingBalance?: PartyOpeningBalance | null;
}

/** A party's opening bill as returned with the party (FR-35). */
export interface PartyOpeningBalance {
  billId: number;
  transactionCode: string;
  amount: number;
  asOfDate: string;
  /** Settlements + return credits already applied to the bill. */
  settledAmount: number;
}

/** list_parties / search_parties row: Party flattened plus ledger stats. */
export interface PartyListRow extends Party {
  outstandingBalance: number;
  transactionCount: number;
}

export interface SaleTransaction {
  id: number;
  transactionCode: string;
  transactionDate: string;
  partyId: number | null;
  partyName: string | null;
  paymentMode: string | null;
  paymentStatus: PaymentStatus;
  totalAmount: number;
  paidAmount: number;
  balanceDue: number;
  notes: string | null;
  createdAt: string;
  itemsSubtotal: number;
  chargesTotal: number;
  discountsTotal: number;
  /** Last edit time (FR-20); null if never edited. */
  updatedAt: string | null;
  /** Party opening-balance bill (FR-35): read-only, no lines. */
  isOpeningBalance?: boolean;
}

export interface SaleItem {
  id: number;
  saleTransactionId: number;
  itemId: number;
  itemName: string;
  packLabel: string | null;
  packQuantity: number;
  /** Base units. */
  quantity: number;
  unitPrice: number;
  totalAmount: number;
}

export interface SaleStockAllocation {
  id: number;
  saleItemId: number;
  purchaseItemId: number;
  quantityDepleted: number;
  returnedQuantity: number;
  unitCost: number;
}

export interface SaleItemDetail {
  saleItem: SaleItem;
  itemName: string;
  bundleName: string | null;
  /** E4 / DD-89: the item's bundle this line was sold in (null: base units or a bundle that is gone). */
  bundleId?: number | null;
  /** Base units already returned. */
  returnedQuantity: number;
  allocations: SaleStockAllocation[];
  /** Sold rate net of the bill discount share (FR-19): the linked-refund cap. */
  netUnitPrice: number;
  /** FIFO cost of the line net of returned units (FR-34). */
  costAmount: number;
  /** Line total less its bill discount share less costAmount (FR-34). */
  profit: number;
  /** Service lines (FR-37): cost not covered by the components' FIFO cost (already inside costAmount). */
  miscCost?: number;
  /** Service lines are not returnable (DD-67); return pickers skip them. */
  isService?: boolean;
  /** FR-45: the item was deleted after the sale; return pickers disable the line. */
  itemDeleted?: boolean;
}

/** Reconciliation of a sale's profit (FR-34): linesProfit + charges + returnAdjustment = total. */
export interface ProfitSummary {
  linesProfit: number;
  charges: number;
  /** Refunds, scrapped cost and replacement cost, as a number <= 0. */
  returnAdjustment: number;
  total: number;
}

export interface PurchaseTransaction {
  id: number;
  transactionCode: string;
  source: LotSource;
  supplierInvoiceRef: string | null;
  transactionDate: string;
  partyId: number | null;
  partyName: string | null;
  paymentMode: string | null;
  paymentStatus: PaymentStatus;
  totalAmount: number;
  paidAmount: number;
  balanceDue: number;
  notes: string | null;
  createdAt: string;
  itemsSubtotal: number;
  chargesTotal: number;
  discountsTotal: number;
  /** Last edit time (FR-20); null if never edited. */
  updatedAt: string | null;
  /** Party opening-balance bill (FR-35): read-only, no lines. */
  isOpeningBalance?: boolean;
}

export interface PurchaseItem {
  id: number;
  purchaseTransactionId: number;
  itemId: number;
  itemName: string;
  packLabel: string | null;
  packQuantity: number;
  quantity: number;
  unitCost: number;
  totalAmount: number;
  remainingQuantity: number;
}

export interface PurchaseItemDetail {
  purchaseItem: PurchaseItem;
  itemName: string;
  bundleName: string | null;
  /** E4 / DD-89: the item's bundle of this lot (null: base units or a bundle that is gone). */
  bundleId?: number | null;
}

/** Ledger row (payment_settlement). */
export interface PaymentSettlement {
  id: number;
  settlementDate: string;
  partyId: number | null;
  kind: PaymentKind;
  transactionType: TransactionType;
  transactionId: number | null;
  returnId: number | null;
  amountPaid: number;
  paymentMode: string | null;
  notes: string | null;
  createdAt: string;
}

export interface SaleHistoryRow {
  id: number;
  transactionCode: string;
  transactionDate: string;
  createdAt: string;
  partyId: number | null;
  partyName: string | null;
  lineCount: number;
  totalAmount: number;
  paidAmount: number;
  balanceDue: number;
  paymentStatus: PaymentStatus;
  itemsSubtotal: number;
  chargesTotal: number;
  discountsTotal: number;
  /** Bill profit (FR-34); null unless the request set includeProfit. Sales only. */
  profit?: number | null;
  /** Party opening-balance bill (FR-35); only listed when the request set includeOpeningBalance. */
  isOpeningBalance?: boolean;
}

export interface PurchaseHistoryRow extends SaleHistoryRow {
  supplierInvoiceRef: string | null;
}

export interface SaleDetailView {
  sale: SaleTransaction;
  party: Party | null;
  items: SaleItemDetail[];
  settlements: PaymentSettlement[];
  charges: BillCharge[];
  profitSummary: ProfitSummary;
}

export interface PurchaseDetailView {
  purchase: PurchaseTransaction;
  party: Party | null;
  items: PurchaseItemDetail[];
  settlements: PaymentSettlement[];
  charges: BillCharge[];
}

export type ChargeEffect = 'CHARGE' | 'DISCOUNT';
export type ChargeAppliesTo = 'SALE' | 'PURCHASE' | 'BOTH';

/** Reusable charge/discount type (FR-19). */
export interface ChargeType {
  id: number;
  name: string;
  effect: ChargeEffect;
  appliesTo: ChargeAppliesTo;
  defaultAmount: number | null;
  isActive: boolean;
  createdAt: string;
}

/** A charge or discount line on a bill; name and effect are snapshots. */
export interface BillCharge {
  id: number;
  transactionType: TransactionType;
  transactionId: number;
  chargeTypeId: number;
  chargeName: string;
  effect: ChargeEffect;
  amount: number;
}

export interface ReturnTransaction {
  id: number;
  returnCode: string;
  returnDate: string;
  returnType: ReturnType;
  partyId: number | null;
  partyName: string | null;
  originalTransactionId: number | null;
  settlementType: SettlementType;
  totalAmount: number;
  reason: string | null;
  notes: string | null;
  createdAt: string;
}

export interface ReturnItem {
  id: number;
  returnTransactionId: number;
  lineType: ReturnLineType;
  itemId: number;
  itemName: string;
  saleItemId: number | null;
  purchaseItemId: number | null;
  quantity: number;
  unitPrice: number;
  totalAmount: number;
  costAmount: number;
  condition: StockCondition | null;
}

export interface ReturnItemDetail {
  returnItem: ReturnItem;
  itemName: string;
}

export interface StockOutflowAllocation {
  id: number;
  sourceType: OutflowSourceType;
  sourceId: number;
  purchaseItemId: number;
  quantityDepleted: number;
  unitCost: number;
}

export interface ReturnDetailView {
  returnTx: ReturnTransaction;
  party: Party | null;
  items: ReturnItemDetail[];
  allocations: StockOutflowAllocation[];
  ledger: PaymentSettlement[];
}

export interface StockAdjustment {
  id: number;
  adjustmentDate: string;
  itemId: number;
  quantity: number;
  unitCost: number | null;
  reason: AdjustmentReason;
  notes: string | null;
  createdAt: string;
}

/** Rust PartyDueSummary. */
export interface PartyDueSummary {
  partyId: number;
  partyName: string;
  phone: string | null;
  partyType: PartyType;
  unsettledBillsCount: number;
  earliestBillDate: string;
  totalInvoiced: number;
  totalPaid: number;
  /** L3: upfront + settlements + return credits of the open bills (totalInvoiced - balanceDue). */
  paidAndCredited?: number;
  balanceDue: number;
  /** FR-46: false for a deleted party that still has dues (badge only). Absent = active. */
  isActive?: boolean;
}

export interface PartyLedgerEntry {
  date: string;
  transactionCode: string;
  transactionType: string;
  debit: number;
  credit: number;
  runningBalance: number;
  notes: string | null;
}

export interface PartyLedgerBill {
  id: number;
  transactionType: TransactionType;
  transactionCode: string;
  transactionDate: string;
  totalAmount: number;
  paidAmount: number;
  balanceDue: number;
  paymentStatus: PaymentStatus;
  isOpeningBalance?: boolean;
}

export interface PartyLedgerView {
  party: Party;
  currentBalance: number;
  bills: PartyLedgerBill[];
  payments: PaymentSettlement[];
  returns: ReturnTransaction[];
  entries: PartyLedgerEntry[];
}

export interface PnLReport {
  grossSales: number;
  salesReturns: number;
  netSales: number;
  /** DD-91: gross FIFO cost by sale date + replacement cost by return date. */
  cogs: number;
  /** DD-91: cost of customer-returned goods credited on the return date. */
  returnedGoodsCost?: number;
  /** Miscellaneous service costs (FR-37), shown under COGS. */
  serviceMiscCosts?: number;
  grossProfit: number;
  inventoryLosses: number;
  netProfit: number;
  totalPurchases: number;
  /** Σ sale discounts (FR-19). */
  salesDiscounts: number;
  /** saleChargesIncome + purchaseDiscountsIncome - purchaseDiscountReversed. */
  otherIncome: number;
  /** Expenses + purchaseChargesExpense. */
  operatingExpenses: number;
  saleChargesIncome: number;
  purchaseDiscountsIncome: number;
  /** DD-87: discount share given back by vendor returns (already inside otherIncome). */
  purchaseDiscountReversed?: number;
  purchaseChargesExpense: number;
  expensesByType: ExpenseTypeTotal[];
}

export interface CategoryValuation {
  category: string;
  itemsCount: number;
  valuation: number;
}

export interface StockValuationReport {
  totalValuation: number;
  activeLotsCount: number;
  categoriesBreakdown: CategoryValuation[];
}

export interface CashFlowReport {
  salesCashInflow: number;
  customerSettlementsInflow: number;
  vendorRefundsInflow: number;
  purchaseCashOutflow: number;
  vendorSettlementsOutflow: number;
  refundsCashOutflow: number;
  /** Σ store expenses dated in the range (FR-18). */
  expensesCashOutflow: number;
  /** Σ of the three inflow lines (backend-computed, DD-56). */
  totalInflow: number;
  /** Σ of the four outflow lines, expenses included. */
  totalOutflow: number;
  netCashBalance: number;
}

export interface DashboardMetrics {
  todaySalesAmount: number;
  todaySalesCount: number;
  todayPurchaseAmount: number;
  todayPurchaseCount: number;
  totalStockValuation: number;
  customerReceivables: number;
  vendorPayables: number;
  /** All-time: opening cash + every cash movement up to today (FR-33). */
  cashInHand: number;
  todayExpensesAmount: number;
}

export interface TrendDataPoint {
  date: string;
  salesAmount: number;
  purchaseAmount: number;
}

export type RecentTransactionKind = 'SALE' | 'PURCHASE' | 'RETURN';

export interface RecentTransaction {
  kind: RecentTransactionKind;
  id: number;
  code: string;
  date: string;
  createdAt: string;
  partyName: string | null;
  amount: number;
  /** null for returns. */
  paymentStatus: PaymentStatus | null;
}

/** Runtime config + backup folder state shown on the Settings page. */
export interface AppConfigView {
  testingMode: boolean;
  backupDirectory: string;
  isDefaultDirectory: boolean;
  lastAutoBackupAt: string | null;
}

export type ResetScope = 'Transactions' | 'All';

export interface ResetSummary {
  scope: ResetScope;
  deleted: { table: string; rows: number }[];
  backupPath: string;
}

export interface ShopSettings {
  id: number;
  shopName: string;
  phone: string | null;
  address: string | null;
  taxId: string | null;
  /** Cash already in the drawer when the shop started using the app (FR-33). */
  openingCash: number;
}

// --- Store expenses (FR-18) ---
export interface ExpenseType {
  id: number;
  name: string;
  isActive: boolean;
  createdAt: string;
}

export interface Expense {
  id: number;
  expenseCode: string;
  expenseDate: string;
  expenseTypeId: number;
  /** Snapshot of the type name at the last save (DD-30). */
  expenseTypeName: string;
  amount: number;
  notes: string | null;
  createdAt: string;
  updatedAt: string | null;
}

export interface ExpenseTypeTotal {
  expenseTypeId: number;
  expenseTypeName: string;
  amount: number;
}


// ---- Item profit and inventory speed (FR-38, FR-39, DD-68) ----

export interface ReportSort {
  field: string;
  dir: 'ASC' | 'DESC';
}

export interface ItemProfitFilter {
  fromDate: string;
  toDate: string;
  search?: string | null;
  sort?: ReportSort | null;
  page?: number | null;
  pageSize?: number | null;
}

export interface ItemProfitRow {
  itemId: number;
  name: string;
  baseUnit: string;
  qtySold: number;
  revenue: number;
  cost: number;
  profit: number;
  marginPct: number | null;
  /** DD-91: refund value of the item's returns dated in range. */
  returnsValue?: number;
  /** DD-91: returned cost credited in range (as the P&L). */
  returnedCost?: number;
}

export interface ItemProfitReport {
  rows: ItemProfitRow[];
  totals: { revenue: number; cost: number; profit: number; marginPct: number | null };
  /** DD-91: items profit = P&L gross profit of the range (FR-38). */
  reconciliation: {
    itemsProfit: number;
    charges: number;
    replacementCost: number;
    billsProfit: number;
    pnlGrossProfit?: number;
  };
  totalCount: number;
  page: number;
  pageSize: number;
}

export type SpeedClass = 'FAST' | 'NORMAL' | 'SLOW' | 'DEAD';

export interface InventorySpeedFilter {
  fromDate: string;
  toDate: string;
  /** ALL / absent, FAST, NORMAL, SLOW or DEAD. */
  class?: string | null;
  search?: string | null;
  sort?: ReportSort | null;
  page?: number | null;
  pageSize?: number | null;
}

export interface InventorySpeedRow {
  itemId: number;
  name: string;
  baseUnit: string;
  unitsOut: number;
  unitsPerDay: number;
  stock: number;
  daysOfCover: number | null;
  class: SpeedClass;
}

export interface InventorySpeedReport {
  rows: InventorySpeedRow[];
  counts: { fast: number; normal: number; slow: number; dead: number };
  days: number;
  totalCount: number;
  page: number;
  pageSize: number;
}
