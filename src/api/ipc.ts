import { invoke } from '@tauri-apps/api/core';
import { errorMessage, normalizeError } from './errors';
import { camelToSnake, snakeToCamel } from './caseConvert';
import type { ApiClient } from '@/types';

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
    const typed = normalizeError(err, cmd);
    console.error(`[TauriClient] Error calling "${cmd}":`, errorMessage(typed));
    // Reject with the typed AppError. A non-enumerable `message` keeps legacy
    // `err.message` readers working until pages migrate to errorMessage() (T11).
    Object.defineProperty(typed, 'message', {
      value: errorMessage(typed),
      enumerable: false,
    });
    throw typed;
  }
}

/** Production ApiClient calling Tauri v2 backend commands. */
export class TauriClient implements ApiClient {
  // --- Sales ---
  getNextSaleCode: ApiClient['getNextSaleCode'] = () => invokeCmd('get_next_sale_code');
  createSale: ApiClient['createSale'] = (payload) => invokeCmd('create_sale', { payload });
  getSalesHistory: ApiClient['getSalesHistory'] = (filter) =>
    invokeCmd('get_sales_history', { filter: filter ?? {} });
  getSaleDetails: ApiClient['getSaleDetails'] = (saleId) =>
    invokeCmd('get_sale_details', { saleId });

  // --- Purchases ---
  getNextPurchaseCode: ApiClient['getNextPurchaseCode'] = () =>
    invokeCmd('get_next_purchase_code');
  createPurchase: ApiClient['createPurchase'] = (payload) =>
    invokeCmd('create_purchase', { payload });
  getPurchasesHistory: ApiClient['getPurchasesHistory'] = (filter) =>
    invokeCmd('get_purchases_history', { filter: filter ?? {} });
  getPurchaseDetails: ApiClient['getPurchaseDetails'] = (purchaseId) =>
    invokeCmd('get_purchase_details', { purchaseId });

  // --- Items & Catalog ---
  searchItemsFts: ApiClient['searchItemsFts'] = (query, limit) =>
    invokeCmd('search_items_fts', { query, limit: limit ?? null });
  getItemsCatalog: ApiClient['getItemsCatalog'] = (filter) =>
    invokeCmd('get_items_catalog', {
      category: filter?.category ?? null,
      lowStockOnly: filter?.lowStockOnly ?? null,
      search: filter?.search ?? null,
      page: filter?.page ?? null,
      limit: filter?.limit ?? null,
    });
  listCategories: ApiClient['listCategories'] = () => invokeCmd('list_categories');
  createItem: ApiClient['createItem'] = (payload) => invokeCmd('create_item', { payload });
  updateItem: ApiClient['updateItem'] = (itemId, payload) =>
    invokeCmd('update_item', { itemId, payload });
  getBundleSuggestions: ApiClient['getBundleSuggestions'] = () =>
    invokeCmd('get_bundle_suggestions');
  createBundle: ApiClient['createBundle'] = (payload) =>
    invokeCmd('create_bundle', { payload });
  updateBundle: ApiClient['updateBundle'] = (bundleId, payload) =>
    invokeCmd('update_bundle', { bundleId, payload });
  deleteBundle: ApiClient['deleteBundle'] = (bundleId) =>
    invokeCmd('delete_bundle', { bundleId });

  // --- Dues & Khata ---
  getPartyDuesSummary: ApiClient['getPartyDuesSummary'] = (partyType) =>
    invokeCmd('get_party_dues_summary', { partyType });
  settleBillPayment: ApiClient['settleBillPayment'] = (payload) =>
    invokeCmd('settle_bill_payment', { payload });
  getPartyLedger: ApiClient['getPartyLedger'] = (partyId) =>
    invokeCmd('get_party_ledger', { partyId });

  // --- Parties ---
  createParty: ApiClient['createParty'] = (payload) => invokeCmd('create_party', { payload });
  updateParty: ApiClient['updateParty'] = (partyId, payload) =>
    invokeCmd('update_party', { partyId, payload });
  getParty: ApiClient['getParty'] = (partyId) => invokeCmd('get_party', { partyId });
  listParties: ApiClient['listParties'] = (partyType, activeOnly) =>
    invokeCmd('list_parties', { partyType: partyType ?? null, activeOnly: activeOnly ?? null });
  searchParties: ApiClient['searchParties'] = (query, partyType) =>
    invokeCmd('search_parties', { query, partyType: partyType ?? null });

  // --- Returns & Adjustments ---
  getNextReturnCode: ApiClient['getNextReturnCode'] = () => invokeCmd('get_next_return_code');
  createCustomerReturn: ApiClient['createCustomerReturn'] = (payload) =>
    invokeCmd('create_customer_return', { payload });
  createVendorReturn: ApiClient['createVendorReturn'] = (payload) =>
    invokeCmd('create_vendor_return', { payload });
  getReturnDetails: ApiClient['getReturnDetails'] = (returnId) =>
    invokeCmd('get_return_details', { returnId });
  listReturns: ApiClient['listReturns'] = (filter) =>
    invokeCmd('list_returns', { filter: filter ?? {} });
  createStockAdjustment: ApiClient['createStockAdjustment'] = (payload) =>
    invokeCmd('create_stock_adjustment', { payload });
  getItemLots: ApiClient['getItemLots'] = (itemId) => invokeCmd('get_item_lots', { itemId });
  listStockAdjustments: ApiClient['listStockAdjustments'] = (itemId, page, limit) =>
    invokeCmd('list_stock_adjustments', {
      itemId: itemId ?? null,
      page: page ?? null,
      limit: limit ?? null,
    });

  // --- Reports ---
  getDashboardMetrics: ApiClient['getDashboardMetrics'] = () =>
    invokeCmd('get_dashboard_metrics');
  getLowStockAlerts: ApiClient['getLowStockAlerts'] = () => invokeCmd('get_low_stock_alerts');
  getSalesPurchaseTrend: ApiClient['getSalesPurchaseTrend'] = (period) =>
    invokeCmd('get_sales_purchase_trend', { period });
  getRecentTransactions: ApiClient['getRecentTransactions'] = (limit) =>
    invokeCmd('get_recent_transactions', { limit });
  getPnLReport: ApiClient['getPnLReport'] = (fromDate, toDate) =>
    invokeCmd('get_pnl_report', { fromDate, toDate });
  getStockValuationReport: ApiClient['getStockValuationReport'] = () =>
    invokeCmd('get_stock_valuation_report');
  getCashFlowReport: ApiClient['getCashFlowReport'] = (fromDate, toDate) =>
    invokeCmd('get_cash_flow_report', { fromDate, toDate });

  // --- Settings & Maintenance ---
  getShopSettings: ApiClient['getShopSettings'] = () => invokeCmd('get_shop_settings');
  updateShopSettings: ApiClient['updateShopSettings'] = (settings) =>
    invokeCmd('update_shop_settings', { settings });
  backupDatabase: ApiClient['backupDatabase'] = (destinationPath) =>
    invokeCmd('backup_database', { destinationPath });
  getAppConfig: ApiClient['getAppConfig'] = () => invokeCmd('get_app_config');
  setBackupDirectory: ApiClient['setBackupDirectory'] = (path) =>
    invokeCmd('set_backup_directory', { path });
  verifyDatabaseIntegrity: ApiClient['verifyDatabaseIntegrity'] = () =>
    invokeCmd('verify_database_integrity');

  // --- Items import ---
  validateItemsImport: ApiClient['validateItemsImport'] = (filePath) =>
    invokeCmd('validate_items_import', { filePath });
  executeItemsImport: ApiClient['executeItemsImport'] = (filePath, conflictStrategy) =>
    invokeCmd('execute_items_import', { filePath, conflictStrategy });
}
