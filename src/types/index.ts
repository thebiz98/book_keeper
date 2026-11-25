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
  /** Base units already returned. */
  returnedQuantity: number;
  allocations: SaleStockAllocation[];
  /** Sold rate net of the bill discount share (FR-19): the linked-refund cap. */
  netUnitPrice: number;
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
  balanceDue: number;
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
  cogs: number;
  grossProfit: number;
  inventoryLosses: number;
  netProfit: number;
  totalPurchases: number;
  /** Σ sale discounts (FR-19). */
  salesDiscounts: number;
  /** saleChargesIncome + purchaseDiscountsIncome. */
  otherIncome: number;
  /** Expenses + purchaseChargesExpense. */
  operatingExpenses: number;
  saleChargesIncome: number;
  purchaseDiscountsIncome: number;
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
