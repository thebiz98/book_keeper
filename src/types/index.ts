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
export type PaymentStatus = 'PAID' | 'PARTIAL' | 'UNPAID';
export type AdjustmentReason = 'DAMAGED' | 'EXPIRED' | 'COUNT_CORRECTION' | 'THEFT_LOST';
/** Refund = value applied to dues first, remainder as cash. Replacement = customer-only no-money swap. */
export type SettlementType = 'REFUND' | 'REPLACEMENT';
export type StockCondition = 'RESTOCKABLE' | 'DAMAGED_SCRAP';
export type ReturnType = 'CUSTOMER' | 'VENDOR';
export type ReturnLineType = 'RETURNED' | 'REPLACEMENT_ISSUED';
export type PaymentKind = 'UPFRONT' | 'SETTLEMENT' | 'RETURN_CREDIT' | 'REFUND_OUT' | 'REFUND_IN';
export type TransactionType = 'SALE' | 'PURCHASE';
export type LotSource = 'PURCHASE' | 'ADJUSTMENT' | 'OPENING' | 'RETURN';
export type OutflowSourceType = 'ADJUSTMENT' | 'REPLACEMENT' | 'VENDOR_RETURN';

export interface Item {
  id: number;
  name: string;
  category: string | null;
  barcode: string | null;
  baseUnit: string;
  mrp: number | null;
  defaultSalePrice: number;
  currentStock: number;
  isLowStock: boolean;
  isActive: boolean;
  createdAt: string;
  updatedAt: string;
}

export interface ItemBundle {
  id: number;
  itemId: number;
  bundleName: string;
  unitsPerBundle: number;
  mrp: number | null;
  defaultBundlePrice: number | null;
  barcode: string | null;
}

export interface ItemBundleSearchResult {
  bundleId: number;
  bundleName: string;
  unitsPerBundle: number;
  bundlePrice: number | null;
  barcode: string | null;
}

export interface ItemSearchResult {
  itemId: number;
  name: string;
  category: string | null;
  barcode: string | null;
  baseUnit: string;
  mrp: number | null;
  defaultSalePrice: number;
  currentStock: number;
  isLowStock: boolean;
  bundles: ItemBundleSearchResult[];
}

export interface LowStockItem {
  id: number;
  itemId: number;
  name: string;
  category: string | null;
  baseUnit: string;
  currentStock: number;
  defaultSalePrice: number;
  mrp: number | null;
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

export interface SaleItem {
  id: number;
  saleTransactionId: number;
  itemId: number;
  bundleId: number | null;
  bundleQuantity: number;
  quantity: number;
  unitPrice: number;
  totalAmount: number;
}

export interface StockLot {
  id: number;
  saleItemId: number;
  purchaseItemId: number;
  quantityDepleted: number;
  unitCost: number;
}

export interface Sale {
  id: number;
  transactionCode: string;
  transactionDate: string;
  partyId: number | null;
  paymentMode: string | null;
  paymentStatus: PaymentStatus;
  totalAmount: number;
  paidAmount: number;
  balanceDue: number;
  notes: string | null;
  createdAt: string;
}

export interface SaleItemDetail {
  saleItem: SaleItem;
  itemName: string;
  bundleName: string | null;
  allocations: StockLot[];
}

export interface PurchaseItem {
  id: number;
  purchaseTransactionId: number;
  itemId: number;
  bundleId: number | null;
  bundleQuantity: number;
  quantity: number;
  unitCost: number;
  totalAmount: number;
  remainingQuantity: number;
}

export interface Purchase {
  id: number;
  transactionCode: string;
  transactionDate: string;
  partyId: number | null;
  paymentMode: string | null;
  paymentStatus: PaymentStatus;
  totalAmount: number;
  paidAmount: number;
  balanceDue: number;
  notes: string | null;
  createdAt: string;
}

export interface PurchaseItemDetail {
  purchaseItem: PurchaseItem;
  itemName: string;
  bundleName: string | null;
}

export interface Payment {
  id: number;
  settlementDate: string;
  partyId: number | null;
  transactionType: string;
  transactionId: number;
  amountPaid: number;
  paymentMode: string | null;
  notes: string | null;
  createdAt: string;
}

export interface SaleDetailView {
  sale: Sale;
  party: Party | null;
  items: SaleItemDetail[];
  settlements: Payment[];
}

export interface PurchaseDetailView {
  purchase: Purchase;
  party: Party | null;
  items: PurchaseItemDetail[];
  settlements: Payment[];
}

export interface ReturnTransaction {
  id: number;
  returnCode: string;
  returnDate: string;
  returnType: ReturnType;
  partyId: number | null;
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
  itemId: number;
  saleItemId: number | null;
  purchaseItemId: number | null;
  quantity: number;
  unitPrice: number;
  totalAmount: number;
  condition: StockCondition;
}

export interface ReturnItemDetail {
  returnItem: ReturnItem;
  itemName: string;
}

export interface ReturnDetailView {
  returnTx: ReturnTransaction;
  party: Party | null;
  items: ReturnItemDetail[];
}

export interface StockAdjustment {
  id: number;
  adjustmentDate: string;
  itemId: number;
  quantity: number;
  reason: AdjustmentReason;
  notes: string | null;
  createdAt: string;
}

/** Rust PartyDueSummary. */
export interface PartyBalance {
  partyId: number;
  partyName: string;
  phone: string | null;
  partyType: PartyType;
  totalDue: number;
  oldestBillDate: string | null;
  unpaidBillsCount: number;
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

export interface PartyLedgerView {
  party: Party;
  currentBalance: number;
  entries: PartyLedgerEntry[];
}

export interface PnlReport {
  fromDate: string;
  toDate: string;
  grossSales: number;
  cogs: number;
  netProfit: number;
  totalDiscounts: number;
  returnsRefunded: number;
}

export interface CategoryValuation {
  category: string;
  itemsCount: number;
  valuation: number;
}

export interface StockValuationReport {
  totalValuation: number;
  activeLotsCount: number;
  totalItemsCount: number;
  categoriesBreakdown: CategoryValuation[];
  categories: CategoryValuation[];
}

export interface CashFlowReport {
  fromDate: string;
  toDate: string;
  totalInflow: number;
  totalOutflow: number;
  netCashFlow: number;
  closingDrawerBalance: number;
  salesCashInflow: number;
  customerSettlementsInflow: number;
  purchaseCashOutflow: number;
  vendorSettlementsOutflow: number;
  refundsCashOutflow: number;
  netCashBalance: number;
}

export interface ShopSettings {
  id: number;
  shopName: string;
  phone: string | null;
  address: string | null;
  taxId: string | null;
}
