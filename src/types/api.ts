/**
 * API client contracts, payloads, and DTOs. camelCase mirrors of the Rust DTOs
 * in src-tauri/src/core/dtos; the IPC layer converts keys at the boundary.
 */

import type {
  AdjustmentReason,
  CashFlowReport,
  Item,
  ItemSearchResult,
  LowStockItem,
  PartyBalance,
  PartyLedgerView,
  PartyType,
  Party,
  PaymentStatus,
  PnlReport,
  Purchase,
  PurchaseDetailView,
  ReturnDetailView,
  Sale,
  SaleDetailView,
  SettlementType,
  ShopSettings,
  StockAdjustment,
  StockCondition,
  StockValuationReport,
} from './index';

export interface PaginatedList<T> {
  items: T[];
  totalCount: number;
  page: number;
  limit: number;
  totalPages: number;
}

export interface HistoryFilter {
  startDate?: string | null;
  endDate?: string | null;
  paymentStatus?: PaymentStatus | null;
  partyId?: number | null;
  page?: number | null;
  limit?: number | null;
}

export interface CatalogFilter {
  category?: string | null;
  lowStockOnly?: boolean;
  page?: number;
  limit?: number;
}

export interface CreateSaleItemPayload {
  itemId: number;
  bundleId: number | null;
  bundleQuantity: number;
  unitPrice: number | null;
}

export interface CreateSalePayload {
  transactionCode?: string | null;
  transactionDate: string;
  partyId: number | null;
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
  changeReturned: number;
  paymentStatus: PaymentStatus;
}

export interface CreatePurchaseItemPayload {
  itemId: number;
  bundleId: number | null;
  bundleQuantity: number;
  unitCost: number;
}

export interface CreatePurchasePayload {
  transactionCode?: string | null;
  transactionDate: string;
  partyId: number | null;
  paidAmount: number;
  paymentMode: string | null;
  notes: string | null;
  items: CreatePurchaseItemPayload[];
}

export interface SettlePaymentPayload {
  partyId: number;
  amount: number;
  settlementDate: string;
  paymentMode: string | null;
  specificBillId: number | null;
  notes: string | null;
}

export interface CreateBundlePayload {
  itemId: number | null;
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
}

export interface CustomerReturnPayload {
  returnCode?: string | null;
  originalSaleId: number | null;
  partyId: number | null;
  returnDate: string;
  settlementType: SettlementType;
  reason?: string | null;
  notes: string | null;
  items: CustomerReturnItemPayload[];
}

export interface VendorReturnItemPayload {
  itemId: number;
  purchaseItemId: number | null;
  quantity: number;
  unitCost: number;
}

export interface VendorReturnPayload {
  originalPurchaseId: number | null;
  partyId: number | null;
  returnDate: string;
  settlementType: SettlementType;
  reason: string | null;
  notes: string | null;
  items: VendorReturnItemPayload[];
}

export interface StockAdjustmentPayload {
  adjustmentDate: string;
  itemId: number;
  quantity: number;
  reason: AdjustmentReason;
  notes: string | null;
}

export interface DashboardMetrics {
  todaySales: number;
  todayPurchases: number;
  todaySalesAmount: number;
  todaySalesCount: number;
  todayPurchaseAmount: number;
  todayPurchaseCount: number;
  todayNetProfit: number;
  totalStockValuation: number;
  customerReceivables: number;
  vendorPayables: number;
  liveCashInDrawer: number;
}

export interface TrendDataPoint {
  date: string;
  salesAmount: number;
  purchaseAmount: number;
}

export interface ImportSummary {
  totalRows: number;
  importedCount: number;
  skippedCount: number;
  errors: string[];
}

export interface ApiClient {
  // Sales
  getNextSaleCode(): Promise<string>;
  createSale(payload: CreateSalePayload): Promise<SaleSummaryResponse>;
  getSalesHistory(filter?: HistoryFilter): Promise<PaginatedList<Sale>>;
  getSaleDetails(saleId: number): Promise<SaleDetailView>;

  // Purchases
  getNextPurchaseCode(): Promise<string>;
  createPurchase(payload: CreatePurchasePayload): Promise<number>;
  getPurchasesHistory(filter?: HistoryFilter): Promise<PaginatedList<Purchase>>;
  getPurchaseDetails(purchaseId: number): Promise<PurchaseDetailView>;

  // Items & Catalog
  searchItemsFts(query: string, limit?: number): Promise<ItemSearchResult[]>;
  getItemsCatalog(filter?: CatalogFilter): Promise<PaginatedList<Item>>;
  createItem(payload: CreateItemPayload): Promise<number>;
  updateItem(itemId: number, payload: UpdateItemPayload): Promise<void>;
  getBundleSuggestions(): Promise<BundleSuggestion[]>;
  createBundle(payload: CreateBundlePayload): Promise<number>;

  // Dues & Ledger
  getPartyDuesSummary(partyType: PartyType): Promise<PartyBalance[]>;
  settleBillPayment(payload: SettlePaymentPayload): Promise<void>;
  getPartyLedger(partyId: number): Promise<PartyLedgerView>;

  // Parties
  createParty(payload: CreatePartyPayload): Promise<number>;
  updateParty(partyId: number, payload: UpdatePartyPayload): Promise<void>;
  getParty(partyId: number): Promise<Party>;
  listParties(partyType?: PartyType, activeOnly?: boolean): Promise<Party[]>;
  searchParties(query: string, partyType?: PartyType): Promise<Party[]>;

  // Returns & Adjustments
  getNextReturnCode(): Promise<string>;
  createCustomerReturn(payload: CustomerReturnPayload): Promise<string>;
  createVendorReturn(payload: VendorReturnPayload): Promise<string>;
  getReturnDetails(returnId: number): Promise<ReturnDetailView>;
  createStockAdjustment(payload: StockAdjustmentPayload): Promise<number>;
  listStockAdjustments(
    itemId?: number,
    page?: number,
    limit?: number
  ): Promise<PaginatedList<StockAdjustment>>;

  // Reports
  getDashboardMetrics(): Promise<DashboardMetrics>;
  getLowStockAlerts(): Promise<LowStockItem[]>;
  getSalesPurchaseTrend(period: string): Promise<TrendDataPoint[]>;
  getPnLReport(fromDate: string, toDate: string): Promise<PnlReport>;
  getStockValuationReport(): Promise<StockValuationReport>;
  getCashFlowReport(fromDate: string, toDate: string): Promise<CashFlowReport>;

  // Settings & System
  getShopSettings(): Promise<ShopSettings>;
  updateShopSettings(settings: ShopSettings): Promise<void>;
  backupDatabase(destinationPath: string): Promise<string>;
  verifyDatabaseIntegrity(): Promise<boolean>;

  // Items import
  importItemsFromExcel(filePath: string): Promise<ImportSummary>;
}
