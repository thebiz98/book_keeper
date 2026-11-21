/**
 * API client contracts, payloads, and DTOs. camelCase mirrors of the Rust DTOs
 * in src-tauri/src/core/dtos; the IPC layer converts keys at the boundary.
 */

import type {
  AdjustmentReason,
  CashFlowReport,
  DashboardMetrics,
  ItemBundle,
  ItemLot,
  ItemSearchResult,
  LowStockItem,
  Party,
  PartyDueSummary,
  PartyLedgerView,
  PartyListRow,
  PartyType,
  PaymentStatus,
  PnLReport,
  PurchaseDetailView,
  PurchaseHistoryRow,
  RecentTransaction,
  ReturnDetailView,
  ReturnTransaction,
  ReturnType,
  SaleDetailView,
  SaleHistoryRow,
  SettlementType,
  ShopSettings,
  StockAdjustment,
  StockCondition,
  StockValuationReport,
  TransactionType,
  TrendDataPoint,
} from './index';

/** Rust Paginated<T>. Page count on the client is ceil(total / limit). */
export interface Paginated<T> {
  items: T[];
  total: number;
  page: number;
  limit: number;
}

export type PaymentStatusFilter = 'ALL' | PaymentStatus;

/** Filter for get_sales_history and get_purchases_history. Dates are YYYY-MM-DD, inclusive. */
export interface HistoryFilter {
  startDate?: string | null;
  endDate?: string | null;
  paymentStatus?: PaymentStatusFilter | null;
  partyId?: number | null;
  searchQuery?: string | null;
  page?: number | null;
  limit?: number | null;
}

export interface ReturnsFilter {
  returnType?: ReturnType | null;
  startDate?: string | null;
  endDate?: string | null;
  page?: number | null;
  limit?: number | null;
}

export interface CatalogFilter {
  category?: string | null;
  lowStockOnly?: boolean | null;
  search?: string | null;
  page?: number | null;
  limit?: number | null;
}

export interface CreateSaleItemPayload {
  itemId: number;
  bundleId: number | null;
  bundleQuantity: number;
  unitPrice: number;
}

export interface CreateSalePayload {
  transactionDate: string;
  partyId: number | null;
  paymentStatus: PaymentStatus;
  paidAmount: number;
  paymentMode: string | null;
  notes: string | null;
  items: CreateSaleItemPayload[];
}

export interface SaleSummaryResponse {
  saleId: number;
  transactionCode: string;
  totalAmount: number;
  paidAmount: number;
  balanceDue: number;
  changeAmount: number;
  paymentStatus: PaymentStatus;
}

export interface CreatePurchaseItemPayload {
  itemId: number;
  bundleId: number | null;
  bundleQuantity: number;
  unitCost: number;
}

export interface CreatePurchasePayload {
  transactionDate: string;
  partyId: number | null;
  supplierInvoiceRef: string | null;
  paymentStatus: PaymentStatus;
  paidAmount: number;
  paymentMode: string | null;
  notes: string | null;
  items: CreatePurchaseItemPayload[];
}

export interface PurchaseSummaryResponse {
  purchaseId: number;
  transactionCode: string;
}

export interface SettlePaymentPayload {
  partyId: number;
  transactionType: TransactionType;
  /** null = lump-sum settlement across open bills (oldest first). */
  transactionId: number | null;
  settlementDate: string;
  amountPaid: number;
  paymentMode: string | null;
  notes: string | null;
}

export interface SettlementResult {
  settlementIds: number[];
  billsTouched: number[];
}

export interface CreateBundlePayload {
  itemId: number | null;
  bundleName: string;
  unitsPerBundle: number;
  mrp: number | null;
  defaultBundlePrice: number | null;
  barcode: string | null;
}

export interface UpdateBundlePayload {
  bundleName: string;
  unitsPerBundle: number;
  mrp: number | null;
  defaultBundlePrice: number | null;
  barcode: string | null;
}
export interface CreateItemPayload {
  name: string;
  category: string | null;
  barcode: string | null;
  baseUnit: string;
  mrp: number | null;
  defaultSalePrice: number;
  initialStock: number | null;
  initialCost: number | null;
  bundles: CreateBundlePayload[] | null;
}

export interface UpdateItemPayload {
  name: string;
  category: string | null;
  barcode: string | null;
  baseUnit: string;
  mrp: number | null;
  defaultSalePrice: number;
}

export interface BundleSuggestion {
  bundleName: string;
  unitsPerBundle: number;
  usageCount: number;
}

export interface CreatePartyPayload {
  name: string;
  phone: string | null;
  partyType: PartyType;
  notes: string | null;
}

export type UpdatePartyPayload = CreatePartyPayload;

export interface CustomerReturnItemPayload {
  itemId: number;
  saleItemId: number | null;
  quantity: number;
  unitPrice: number;
  condition: StockCondition;
  unitCostOverride: number | null;
}

export interface ReplacementItemPayload {
  itemId: number;
  quantity: number;
}

export interface CustomerReturnPayload {
  originalSaleId: number | null;
  partyId: number | null;
  returnDate: string;
  settlementType: SettlementType;
  reason?: string | null;
  notes: string | null;
  items: CustomerReturnItemPayload[];
  replacementItems: ReplacementItemPayload[];
}

export interface VendorReturnItemPayload {
  itemId: number;
  purchaseItemId: number | null;
  quantity: number;
}

/** Vendor returns must use settlementType REFUND. Unit cost comes from the lot. */
export interface VendorReturnPayload {
  originalPurchaseId: number | null;
  partyId: number | null;
  returnDate: string;
  settlementType: SettlementType;
  reason: string | null;
  notes: string | null;
  items: VendorReturnItemPayload[];
}

export interface ReturnResult {
  returnId: number;
  returnCode: string;
  creditedAmount: number;
  cashRefunded: number;
}

export interface StockAdjustmentPayload {
  adjustmentDate: string;
  itemId: number;
  /** Positive adds stock, negative writes off. */
  quantity: number;
  reason: AdjustmentReason;
  /** Lot cost for positive adjustments. */
  unitCost: number | null;
  notes: string | null;
}

export interface StockAdjustmentResult {
  adjustmentId: number;
  lossValue: number;
}

export type ConflictStrategy = 'SKIP' | 'OVERWRITE' | 'ADD_STOCK';

export interface ImportRowError {
  row: number;
  field: string;
  message: string;
}

export interface ImportDuplicate {
  row: number;
  name: string;
  existingItemId: number;
}

export interface ImportValidationReport {
  validCount: number;
  rowsWithErrors: ImportRowError[];
  duplicates: ImportDuplicate[];
}

export interface ImportSummary {
  created: number;
  updated: number;
  skipped: number;
  stockLotsCreated: number;
}

export interface ApiClient {
  // Sales
  getNextSaleCode(): Promise<string>;
  createSale(payload: CreateSalePayload): Promise<SaleSummaryResponse>;
  getSalesHistory(filter?: HistoryFilter): Promise<Paginated<SaleHistoryRow>>;
  getSaleDetails(saleId: number): Promise<SaleDetailView>;

  // Purchases
  getNextPurchaseCode(): Promise<string>;
  createPurchase(payload: CreatePurchasePayload): Promise<PurchaseSummaryResponse>;
  getPurchasesHistory(filter?: HistoryFilter): Promise<Paginated<PurchaseHistoryRow>>;
  getPurchaseDetails(purchaseId: number): Promise<PurchaseDetailView>;

  // Items & Catalog
  searchItemsFts(query: string, limit?: number): Promise<ItemSearchResult[]>;
  getItemsCatalog(filter?: CatalogFilter): Promise<Paginated<ItemSearchResult>>;
  listCategories(): Promise<string[]>;
  createItem(payload: CreateItemPayload): Promise<number>;
  updateItem(itemId: number, payload: UpdateItemPayload): Promise<void>;
  getBundleSuggestions(): Promise<BundleSuggestion[]>;
  createBundle(payload: CreateBundlePayload): Promise<ItemBundle>;
  updateBundle(bundleId: number, payload: UpdateBundlePayload): Promise<ItemBundle>;
  deleteBundle(bundleId: number): Promise<void>;

  // Dues & Ledger
  getPartyDuesSummary(partyType: PartyType): Promise<PartyDueSummary[]>;
  settleBillPayment(payload: SettlePaymentPayload): Promise<SettlementResult>;
  getPartyLedger(partyId: number): Promise<PartyLedgerView>;

  // Parties
  createParty(payload: CreatePartyPayload): Promise<number>;
  updateParty(partyId: number, payload: UpdatePartyPayload): Promise<void>;
  getParty(partyId: number): Promise<Party>;
  listParties(partyType?: PartyType, activeOnly?: boolean): Promise<PartyListRow[]>;
  searchParties(query: string, partyType?: PartyType): Promise<PartyListRow[]>;

  // Returns & Adjustments
  getNextReturnCode(): Promise<string>;
  createCustomerReturn(payload: CustomerReturnPayload): Promise<ReturnResult>;
  createVendorReturn(payload: VendorReturnPayload): Promise<ReturnResult>;
  getReturnDetails(returnId: number): Promise<ReturnDetailView>;
  listReturns(filter?: ReturnsFilter): Promise<Paginated<ReturnTransaction>>;
  createStockAdjustment(payload: StockAdjustmentPayload): Promise<StockAdjustmentResult>;
  /**
   * NOTE: get_item_lots is implemented in adjustment_controller.rs but is NOT yet in
   * main.rs generate_handler!. It will reject until the backend registers it.
   */
  getItemLots(itemId: number): Promise<ItemLot[]>;
  listStockAdjustments(
    itemId?: number,
    page?: number,
    limit?: number
  ): Promise<Paginated<StockAdjustment>>;

  // Reports
  getDashboardMetrics(): Promise<DashboardMetrics>;
  getLowStockAlerts(): Promise<LowStockItem[]>;
  /** period: '7d' | '30d' | '1y'. */
  getSalesPurchaseTrend(period: string): Promise<TrendDataPoint[]>;
  getRecentTransactions(limit: number): Promise<RecentTransaction[]>;
  getPnLReport(fromDate: string, toDate: string): Promise<PnLReport>;
  getStockValuationReport(): Promise<StockValuationReport>;
  getCashFlowReport(fromDate: string, toDate: string): Promise<CashFlowReport>;

  // Settings & System
  getShopSettings(): Promise<ShopSettings>;
  updateShopSettings(settings: ShopSettings): Promise<void>;
  backupDatabase(destinationPath: string): Promise<string>;
  verifyDatabaseIntegrity(): Promise<boolean>;

  // Items import
  validateItemsImport(filePath: string): Promise<ImportValidationReport>;
  executeItemsImport(
    filePath: string,
    conflictStrategy: ConflictStrategy
  ): Promise<ImportSummary>;
}
