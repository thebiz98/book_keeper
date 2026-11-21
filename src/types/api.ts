/**
 * API Client contracts, payloads, and DTOs matching Tauri IPC and MockApiClient.
 */

import type {
  Item,
  PackagingBundle,
  Party,
  Sale,
  Purchase,
  PartyBalance,
  PnlReport,
  StockValuationReport,
  CashFlowReport,
  ShopSettings,
} from './index';

export interface PaginatedList<T> {
  items: T[];
  totalCount?: number;
  total_count?: number;
  total: number;
  page: number;
  limit: number;
  totalPages: number;
  total_pages?: number;
}

export interface CreateSaleItemPayload {
  itemId?: number;
  item_id?: number;
  bundleId?: number | null;
  bundle_id?: number | null;
  quantity?: number;
  bundleQuantity?: number;
  bundle_quantity?: number;
  unitsPerBundle?: number;
  units_per_bundle?: number;
  rate?: number;
  unitPrice?: number;
  unit_price?: number;
}

export interface CreateSalePayload {
  transactionCode?: string;
  transaction_code?: string;
  saleCode?: string;
  sale_code?: string;
  transactionDate?: string;
  transaction_date?: string;
  date?: string;
  partyId?: number | null;
  party_id?: number | null;
  customerId?: number | null;
  paidAmount?: number;
  paid_amount?: number;
  paymentMode?: string | null;
  payment_mode?: string | null;
  paymentStatus?: string;
  payment_status?: string;
  notes?: string | null;
  items: CreateSaleItemPayload[];
}

export interface SaleSummaryResponse {
  saleId: number;
  sale_id?: number;
  transactionCode: string;
  transaction_code?: string;
  saleCode?: string;
  sale_code?: string;
  totalAmount: number;
  total_amount?: number;
  paidAmount: number;
  paid_amount?: number;
  balanceDue: number;
  balance_due?: number;
  changeAmount?: number;
  change_amount?: number;
  changeReturned?: number;
  change_returned?: number;
  paymentStatus: string;
  payment_status?: string;
  date?: string;
  items?: any[];
}

export interface CreatePurchaseItemPayload {
  itemId?: number;
  item_id?: number;
  bundleId?: number | null;
  bundle_id?: number | null;
  quantity?: number;
  bundleQuantity?: number;
  bundle_quantity?: number;
  unitsPerBundle?: number;
  units_per_bundle?: number;
  unitCost?: number;
  unit_cost?: number;
}

export interface CreatePurchasePayload {
  transactionCode?: string;
  transaction_code?: string;
  purchaseCode?: string;
  purchase_code?: string;
  transactionDate?: string;
  transaction_date?: string;
  date?: string;
  partyId?: number | null;
  party_id?: number | null;
  vendorId?: number | null;
  paidAmount?: number;
  paid_amount?: number;
  paymentMode?: string | null;
  payment_mode?: string | null;
  paymentStatus?: string;
  payment_status?: string;
  notes?: string | null;
  items: CreatePurchaseItemPayload[];
}

export interface SettlePaymentPayload {
  partyId?: number;
  party_id?: number;
  amount: number;
  settlementDate?: string;
  settlement_date?: string;
  date?: string;
  paymentMode?: string | null;
  payment_mode?: string | null;
  specificBillId?: number | null;
  specific_bill_id?: number | null;
  notes?: string | null;
}

export interface CreateItemPayload {
  name: string;
  category?: string | null;
  barcode?: string | null;
  baseUnit?: string;
  base_unit?: string;
  mrp?: number | null;
  defaultSalePrice?: number;
  default_sale_price?: number;
  initialStock?: number;
  initial_stock?: number;
  initialCost?: number;
  initial_cost?: number;
  bundles?: CreateBundlePayload[];
}

export interface UpdateItemPayload {
  name: string;
  category?: string | null;
  barcode?: string | null;
  baseUnit?: string;
  base_unit?: string;
  mrp?: number | null;
  defaultSalePrice?: number;
  default_sale_price?: number;
  currentStock?: number;
  current_stock?: number;
}

export interface CreateBundlePayload {
  itemId?: number | null;
  item_id?: number | null;
  bundleName: string;
  bundle_name?: string;
  multiplier?: number;
  unitsPerBundle: number;
  units_per_bundle?: number;
  salePrice?: number | null;
  sale_price?: number | null;
  defaultBundlePrice?: number | null;
  default_bundle_price?: number | null;
  mrp?: number | null;
  barcode?: string | null;
}

export interface BundleSuggestion {
  bundleName: string;
  bundle_name?: string;
  name?: string;
  unitsPerBundle: number;
  units_per_bundle?: number;
  multiplier?: number;
  usageCount?: number;
  usage_count?: number;
}

export interface DashboardMetrics {
  todaySales?: number;
  today_sales?: number;
  todayGrossSales?: number;
  todayPurchases?: number;
  today_purchases?: number;
  todayNetProfit?: number;
  today_net_profit?: number;
  totalStockValuation?: number;
  total_stock_valuation?: number;
  stockValuation?: number;
  customerReceivables?: number;
  customer_receivables?: number;
  vendorPayables?: number;
  vendor_payables?: number;
  liveCashInDrawer?: number;
  live_cash_in_drawer?: number;
  cashDrawer?: number;
}

export interface TrendDataPoint {
  date: string;
  salesAmount?: number;
  sales_amount?: number;
  purchaseAmount?: number;
  purchase_amount?: number;
}

export interface BackupFileInfo {
  path: string;
  fileName?: string;
  file_name?: string;
  sizeBytes?: number;
  size_bytes?: number;
  modifiedAt?: string;
  modified_at?: string;
}

export interface BackupInspection {
  path?: string;
  itemsCount?: number;
  items_count?: number;
  partiesCount?: number;
  parties_count?: number;
  salesCount?: number;
  sales_count?: number;
  purchasesCount?: number;
  purchases_count?: number;
  warnings?: string[];
}

export interface MigrationReport {
  success?: boolean;
  itemsImported?: number;
  items_imported?: number;
  partiesImported?: number;
  parties_imported?: number;
  salesImported?: number;
  sales_imported?: number;
  purchasesImported?: number;
  purchases_imported?: number;
  errors?: string[];
  warnings?: string[];
}

export interface ApiClient {
  // Sales
  getNextSaleCode(): Promise<string>;
  createSale(payload: CreateSalePayload | any): Promise<SaleSummaryResponse | any>;
  getSalesHistory(filter?: any): Promise<PaginatedList<Sale> | any>;
  getSaleDetails(saleId: number): Promise<any>;

  // Purchases
  getNextPurchaseCode(): Promise<string>;
  createPurchase(payload: CreatePurchasePayload | any): Promise<any>;
  getPurchasesHistory(filter?: any): Promise<PaginatedList<Purchase> | any>;
  getPurchaseDetails(purchaseId: number): Promise<any>;

  // Items & Catalog
  searchItemsFts(query: string, limit?: number): Promise<Item[] | any[]>;
  getItemsCatalog(filter?: any, lowStockOnlyArg?: boolean): Promise<{ total: number; items: Item[]; page?: number; limit?: number; totalPages?: number } | any>;
  createItem(payload: CreateItemPayload | any): Promise<Item | any>;
  updateItem(itemId: number, payload: UpdateItemPayload | any): Promise<Item | boolean | any>;
  archiveItem(itemId: number): Promise<boolean>;
  getBundleSuggestions(): Promise<BundleSuggestion[] | any[]>;
  createBundle(payload: CreateBundlePayload | any): Promise<PackagingBundle | any>;

  // Dues & Ledger
  getPartyDuesSummary(partyType: 'CUSTOMER' | 'VENDOR' | 'Customer' | 'Vendor'): Promise<PartyBalance[] | any[]>;
  settleBillPayment(payload: SettlePaymentPayload | any): Promise<boolean>;
  getPartyLedger(partyId: number): Promise<any>;

  // Parties
  createParty(payload: any): Promise<any>;
  updateParty(partyId: number, payload: any): Promise<boolean>;
  archiveParty(partyId: number): Promise<boolean>;
  getParty(partyId: number): Promise<Party | any>;
  listParties(partyType?: 'CUSTOMER' | 'VENDOR' | 'Customer' | 'Vendor', activeOnly?: boolean): Promise<Party[] | any[]>;
  searchParties(query: string, partyType?: 'CUSTOMER' | 'VENDOR' | 'Customer' | 'Vendor'): Promise<Party[] | any[]>;

  // Returns & Adjustments
  getNextReturnCode(): Promise<string>;
  createCustomerReturn(payload: any): Promise<any>;
  createVendorReturn(payload: any): Promise<any>;
  getReturnDetails(returnId: number): Promise<any>;
  createStockAdjustment(payload: any): Promise<any>;
  listStockAdjustments(itemId?: number, page?: number, limit?: number): Promise<any>;

  // Reports
  getDashboardMetrics(): Promise<DashboardMetrics | any>;
  getLowStockAlerts(): Promise<Item[] | any[]>;
  getSalesPurchaseTrend(period: '7d' | '30d' | '1y' | string): Promise<TrendDataPoint[] | any[]>;
  getPnLReport(fromDate: string, toDate: string): Promise<PnlReport | any>;
  getStockValuationReport(): Promise<StockValuationReport | any>;
  getCashFlowReport(fromDate: string, toDate: string): Promise<CashFlowReport | any>;

  // Settings & System
  getShopSettings(): Promise<ShopSettings | any>;
  updateShopSettings(settings: ShopSettings | any): Promise<boolean>;
  backupDatabase(destinationPath: string): Promise<string>;
  verifyDatabaseIntegrity(): Promise<boolean>;

  // Migration & Excel
  scanBackupDirectory(directoryPath: string): Promise<BackupFileInfo[]>;
  inspectBackupFile(filePath: string): Promise<BackupInspection>;
  executeMigration(filePath: string): Promise<MigrationReport>;
  exportItemsToExcel(destinationPath: string): Promise<string>;
  importItemsFromExcel(filePath: string): Promise<MigrationReport>;
  exportTransactionsToExcel(destinationPath: string, filter?: any): Promise<string>;
}
