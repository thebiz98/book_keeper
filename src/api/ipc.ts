import { invoke } from '@tauri-apps/api/core';
import { camelToSnake, snakeToCamel } from './caseConvert';
import type {
  ApiClient,
  BackupFileInfo,
  BackupInspection,
  BundleSuggestion,
  CashFlowReport,
  CatalogFilter,
  CreateBundlePayload,
  CreateItemPayload,
  CreatePartyPayload,
  CreatePurchasePayload,
  CreateSalePayload,
  CustomerReturnPayload,
  DashboardMetrics,
  HistoryFilter,
  ImportSummary,
  Item,
  ItemSearchResult,
  LowStockItem,
  MigrationReport,
  PaginatedList,
  Party,
  PartyBalance,
  PartyLedgerView,
  PartyType,
  PnlReport,
  Purchase,
  PurchaseDetailView,
  ReturnDetailView,
  Sale,
  SaleDetailView,
  SaleSummaryResponse,
  SettlePaymentPayload,
  ShopSettings,
  StockAdjustment,
  StockAdjustmentPayload,
  StockValuationReport,
  TransactionExportFilter,
  TrendDataPoint,
  UpdateItemPayload,
  UpdatePartyPayload,
  VendorReturnPayload,
} from '@/types';

/**
 * Single normalization layer for Tauri IPC.
 * - Top-level arg names stay camelCase (Tauri v2 maps them to Rust params).
 * - Nested request objects are converted camelCase -> snake_case.
 * - Responses are converted snake_case -> camelCase.
 */
export async function invokeCmd<T>(
  cmd: string,
  args?: Record<string, unknown>
): Promise<T> {
  const converted = args
    ? Object.fromEntries(
        Object.entries(args).map(([k, v]) => [k, camelToSnake(v)])
      )
    : undefined;
  try {
    const res = await invoke<unknown>(cmd, converted);
    return snakeToCamel<T>(res);
  } catch (err) {
    const msg =
      typeof err === 'string'
        ? err
        : err instanceof Error
          ? err.message
          : `IPC command "${cmd}" failed`;
    console.error(`[TauriClient] Error calling "${cmd}":`, msg);
    throw new Error(msg);
  }
}

/** Production ApiClient calling Tauri v2 backend commands. */
export class TauriClient implements ApiClient {
  // --- Sales ---
  getNextSaleCode() {
    return invokeCmd<string>('get_next_sale_code');
  }
  createSale(payload: CreateSalePayload) {
    return invokeCmd<SaleSummaryResponse>('create_sale', { payload });
  }
  getSalesHistory(filter?: HistoryFilter) {
    return invokeCmd<PaginatedList<Sale>>('get_sales_history', {
      filter: filter ?? {},
    });
  }
  getSaleDetails(saleId: number) {
    return invokeCmd<SaleDetailView>('get_sale_details', { saleId });
  }

  // --- Purchases ---
  getNextPurchaseCode() {
    return invokeCmd<string>('get_next_purchase_code');
  }
  createPurchase(payload: CreatePurchasePayload) {
    return invokeCmd<number>('create_purchase', { payload });
  }
  getPurchasesHistory(filter?: HistoryFilter) {
    return invokeCmd<PaginatedList<Purchase>>('get_purchases_history', {
      filter: filter ?? {},
    });
  }
  getPurchaseDetails(purchaseId: number) {
    return invokeCmd<PurchaseDetailView>('get_purchase_details', { purchaseId });
  }

  // --- Items & Catalog ---
  searchItemsFts(query: string, limit?: number) {
    return invokeCmd<ItemSearchResult[]>('search_items_fts', {
      query,
      limit: limit ?? null,
    });
  }
  getItemsCatalog(filter?: CatalogFilter) {
    return invokeCmd<PaginatedList<Item>>('get_items_catalog', {
      category: filter?.category ?? null,
      lowStockOnly: filter?.lowStockOnly ?? null,
      page: filter?.page ?? null,
      limit: filter?.limit ?? null,
    });
  }
  createItem(payload: CreateItemPayload) {
    return invokeCmd<number>('create_item', { payload });
  }
  updateItem(itemId: number, payload: UpdateItemPayload) {
    return invokeCmd<void>('update_item', { itemId, payload });
  }
  archiveItem(itemId: number) {
    return invokeCmd<void>('archive_item', { itemId });
  }
  getBundleSuggestions() {
    return invokeCmd<BundleSuggestion[]>('get_bundle_suggestions');
  }
  createBundle(payload: CreateBundlePayload) {
    return invokeCmd<number>('create_bundle', { payload });
  }

  // --- Dues & Khata ---
  getPartyDuesSummary(partyType: PartyType) {
    return invokeCmd<PartyBalance[]>('get_party_dues_summary', { partyType });
  }
  settleBillPayment(payload: SettlePaymentPayload) {
    return invokeCmd<void>('settle_bill_payment', { payload });
  }
  getPartyLedger(partyId: number) {
    return invokeCmd<PartyLedgerView>('get_party_ledger', { partyId });
  }

  // --- Parties ---
  createParty(payload: CreatePartyPayload) {
    return invokeCmd<number>('create_party', { payload });
  }
  updateParty(partyId: number, payload: UpdatePartyPayload) {
    return invokeCmd<void>('update_party', { partyId, payload });
  }
  archiveParty(partyId: number) {
    return invokeCmd<void>('archive_party', { partyId });
  }
  getParty(partyId: number) {
    return invokeCmd<Party>('get_party', { partyId });
  }
  listParties(partyType?: PartyType, activeOnly = true) {
    return invokeCmd<Party[]>('list_parties', {
      partyType: partyType ?? null,
      activeOnly,
    });
  }
  searchParties(query: string, partyType?: PartyType) {
    return invokeCmd<Party[]>('search_parties', {
      query,
      partyType: partyType ?? null,
    });
  }

  // --- Returns & Adjustments ---
  getNextReturnCode() {
    return invokeCmd<string>('get_next_return_code');
  }
  createCustomerReturn(payload: CustomerReturnPayload) {
    return invokeCmd<string>('create_customer_return', { payload });
  }
  createVendorReturn(payload: VendorReturnPayload) {
    return invokeCmd<string>('create_vendor_return', { payload });
  }
  getReturnDetails(returnId: number) {
    return invokeCmd<ReturnDetailView>('get_return_details', { returnId });
  }
  createStockAdjustment(payload: StockAdjustmentPayload) {
    return invokeCmd<number>('create_stock_adjustment', { payload });
  }
  listStockAdjustments(itemId?: number, page = 1, limit = 50) {
    return invokeCmd<PaginatedList<StockAdjustment>>('list_stock_adjustments', {
      itemId: itemId ?? null,
      page,
      limit,
    });
  }

  // --- Reports ---
  getDashboardMetrics() {
    return invokeCmd<DashboardMetrics>('get_dashboard_metrics');
  }
  getLowStockAlerts() {
    return invokeCmd<LowStockItem[]>('get_low_stock_alerts');
  }
  getSalesPurchaseTrend(period: string) {
    return invokeCmd<TrendDataPoint[]>('get_sales_purchase_trend', { period });
  }
  getPnLReport(fromDate: string, toDate: string) {
    return invokeCmd<PnlReport>('get_pnl_report', { fromDate, toDate });
  }
  getStockValuationReport() {
    return invokeCmd<StockValuationReport>('get_stock_valuation_report');
  }
  getCashFlowReport(fromDate: string, toDate: string) {
    return invokeCmd<CashFlowReport>('get_cash_flow_report', { fromDate, toDate });
  }

  // --- Settings & Maintenance ---
  getShopSettings() {
    return invokeCmd<ShopSettings>('get_shop_settings');
  }
  updateShopSettings(settings: ShopSettings) {
    return invokeCmd<void>('update_shop_settings', { settings });
  }
  backupDatabase(destinationPath: string) {
    return invokeCmd<string>('backup_database', { destinationPath });
  }
  verifyDatabaseIntegrity() {
    return invokeCmd<boolean>('verify_database_integrity');
  }

  // --- Migration & Excel ---
  scanBackupDirectory(directoryPath?: string) {
    return invokeCmd<BackupFileInfo[]>('scan_backup_directory', {
      directoryPath: directoryPath ?? null,
    });
  }
  inspectBackupFile(filePath: string) {
    return invokeCmd<BackupInspection>('inspect_backup_file', { filePath });
  }
  executeMigration(filePath: string) {
    return invokeCmd<MigrationReport>('execute_migration', { filePath });
  }
  exportItemsToExcel(destinationPath: string) {
    return invokeCmd<string>('export_items_to_excel', { destinationPath });
  }
  importItemsFromExcel(filePath: string) {
    return invokeCmd<ImportSummary>('import_items_from_excel', { filePath });
  }
  exportTransactionsToExcel(
    destinationPath: string,
    filter?: TransactionExportFilter | null
  ) {
    return invokeCmd<string>('export_transactions_to_excel', {
      destinationPath,
      filter: filter ?? null,
    });
  }
}
