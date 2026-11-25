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
  updateSale: ApiClient['updateSale'] = (saleId, payload) =>
    invokeCmd('update_sale', { saleId, payload });
  deleteSale: ApiClient['deleteSale'] = (saleId) => invokeCmd('delete_sale', { saleId });
  previewSale: ApiClient['previewSale'] = (payload, saleId) =>
    invokeCmd('preview_sale', { payload, saleId: saleId ?? null });

  // --- Purchases ---
  getNextPurchaseCode: ApiClient['getNextPurchaseCode'] = () =>
    invokeCmd('get_next_purchase_code');
  createPurchase: ApiClient['createPurchase'] = (payload) =>
    invokeCmd('create_purchase', { payload });
  getPurchasesHistory: ApiClient['getPurchasesHistory'] = (filter) =>
    invokeCmd('get_purchases_history', { filter: filter ?? {} });
  getPurchaseDetails: ApiClient['getPurchaseDetails'] = (purchaseId) =>
    invokeCmd('get_purchase_details', { purchaseId });
  updatePurchase: ApiClient['updatePurchase'] = (purchaseId, payload) =>
    invokeCmd('update_purchase', { purchaseId, payload });
  updateLotPurchasePrice: ApiClient['updateLotPurchasePrice'] = (payload) =>
    invokeCmd('update_lot_purchase_price', { payload });
  previewPurchase: ApiClient['previewPurchase'] = (payload, purchaseId) =>
    invokeCmd('preview_purchase', { payload, purchaseId: purchaseId ?? null });
  previewLotPurchasePrice: ApiClient['previewLotPurchasePrice'] = (payload) =>
    invokeCmd('preview_lot_purchase_price', { payload });
  deletePurchase: ApiClient['deletePurchase'] = (purchaseId) =>
    invokeCmd('delete_purchase', { purchaseId });

  // --- Items & Catalog ---
  searchItemsFts: ApiClient['searchItemsFts'] = (query, limit) =>
    invokeCmd('search_items_fts', { query, limit: limit ?? null });
  getItem: ApiClient['getItem'] = (itemId) => invokeCmd('get_item', { itemId });
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
  updateItemLowStockThreshold: ApiClient['updateItemLowStockThreshold'] = (itemId, threshold) =>
    invokeCmd('update_item_low_stock_threshold', { itemId, threshold });
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
  previewSettlementAllocation: ApiClient['previewSettlementAllocation'] = (partyId, transactionType, amount) =>
    invokeCmd('preview_settlement_allocation', { partyId, transactionType, amount });
  settleBillsBatch: ApiClient['settleBillsBatch'] = (payload) =>
    invokeCmd('settle_bills_batch', { payload });
  updateSettlement: ApiClient['updateSettlement'] = (settlementId, payload) =>
    invokeCmd('update_settlement', { settlementId, payload });
  deleteSettlement: ApiClient['deleteSettlement'] = (settlementId) =>
    invokeCmd('delete_settlement', { settlementId });
  getSettlementHistory: ApiClient['getSettlementHistory'] = (filter) =>
    invokeCmd('get_settlement_history', { filter });
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

  // --- Charge types ---
  searchChargeTypes: ApiClient['searchChargeTypes'] = (query, appliesTo, limit) =>
    invokeCmd('search_charge_types', { query, appliesTo, limit: limit ?? null });
  createChargeType: ApiClient['createChargeType'] = (payload) => invokeCmd('create_charge_type', { payload });

  // --- Expenses ---
  searchExpenseTypes: ApiClient['searchExpenseTypes'] = (query, limit) =>
    invokeCmd('search_expense_types', { query, limit: limit ?? null });
  listExpenseTypes: ApiClient['listExpenseTypes'] = () => invokeCmd('list_expense_types');
  createExpenseType: ApiClient['createExpenseType'] = (name) =>
    invokeCmd('create_expense_type', { payload: { name } });
  updateExpenseType: ApiClient['updateExpenseType'] = (expenseTypeId, name) =>
    invokeCmd('update_expense_type', { expenseTypeId, payload: { name } });
  getNextExpenseCode: ApiClient['getNextExpenseCode'] = () => invokeCmd('get_next_expense_code');
  createExpense: ApiClient['createExpense'] = (payload) => invokeCmd('create_expense', { payload });
  updateExpense: ApiClient['updateExpense'] = (expenseId, payload) =>
    invokeCmd('update_expense', { expenseId, payload });
  deleteExpense: ApiClient['deleteExpense'] = (expenseId) => invokeCmd('delete_expense', { expenseId });
  listExpenses: ApiClient['listExpenses'] = (filter) => invokeCmd('list_expenses', { filter: filter ?? {} });

  // --- Returns & Adjustments ---
  getNextReturnCode: ApiClient['getNextReturnCode'] = () => invokeCmd('get_next_return_code');
  createCustomerReturn: ApiClient['createCustomerReturn'] = (payload) =>
    invokeCmd('create_customer_return', { payload });
  createVendorReturn: ApiClient['createVendorReturn'] = (payload) =>
    invokeCmd('create_vendor_return', { payload });
  previewCustomerReturn: ApiClient['previewCustomerReturn'] = (payload) =>
    invokeCmd('preview_customer_return', { payload });
  previewVendorReturn: ApiClient['previewVendorReturn'] = (payload) =>
    invokeCmd('preview_vendor_return', { payload });
  getReturnDetails: ApiClient['getReturnDetails'] = (returnId) =>
    invokeCmd('get_return_details', { returnId });
  deleteReturn: ApiClient['deleteReturn'] = (returnId) => invokeCmd('delete_return', { returnId });
  getBillEditGuard: ApiClient['getBillEditGuard'] = (target, id) =>
    invokeCmd('get_bill_edit_guard', { target, id });
  listReturns: ApiClient['listReturns'] = (filter) =>
    invokeCmd('list_returns', { filter: filter ?? {} });
  createStockAdjustment: ApiClient['createStockAdjustment'] = (payload) =>
    invokeCmd('create_stock_adjustment', { payload });
  setItemStock: ApiClient['setItemStock'] = (payload) => invokeCmd('set_item_stock', { payload });
  previewStockAdjustment: ApiClient['previewStockAdjustment'] = (payload) =>
    invokeCmd('preview_stock_adjustment', { payload });
  previewSetItemStock: ApiClient['previewSetItemStock'] = (payload) =>
    invokeCmd('preview_set_item_stock', { payload });
  previewLotDraw: ApiClient['previewLotDraw'] = (itemId, quantity, priorLines) =>
    invokeCmd('preview_lot_draw', { itemId, quantity, priorLines });
  previewServiceCost: ApiClient['previewServiceCost'] = (payload) => invokeCmd('preview_service_cost', { payload });
  getServiceComponents: ApiClient['getServiceComponents'] = (itemId) => invokeCmd('get_service_components', { itemId });
  getItemLots: ApiClient['getItemLots'] = (itemId) => invokeCmd('get_item_lots', { itemId });
  getItemSales: ApiClient['getItemSales'] = (itemId, page, limit) =>
    invokeCmd('get_item_sales', { itemId, page: page ?? null, limit: limit ?? null });
  getItemPurchases: ApiClient['getItemPurchases'] = (itemId, page, limit) =>
    invokeCmd('get_item_purchases', { itemId, page: page ?? null, limit: limit ?? null });
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
  getItemProfitReport: ApiClient['getItemProfitReport'] = (filter) =>
    invokeCmd('get_item_profit_report', { filter });
  getInventorySpeedReport: ApiClient['getInventorySpeedReport'] = (filter) =>
    invokeCmd('get_inventory_speed_report', { filter });
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
  resetData: ApiClient['resetData'] = (scope) => invokeCmd('reset_data', { scope });
  verifyDatabaseIntegrity: ApiClient['verifyDatabaseIntegrity'] = () =>
    invokeCmd('verify_database_integrity');

  // --- Items import ---
  validateItemsImport: ApiClient['validateItemsImport'] = (filePath) =>
    invokeCmd('validate_items_import', { filePath });
  executeItemsImport: ApiClient['executeItemsImport'] = (filePath, conflictStrategy) =>
    invokeCmd('execute_items_import', { filePath, conflictStrategy });
  validateVedbillImport: ApiClient['validateVedbillImport'] = (filePath) =>
    invokeCmd('validate_vedbill_import', { filePath });
  executeVedbillImport: ApiClient['executeVedbillImport'] = (filePath, conflictStrategy) =>
    invokeCmd('execute_vedbill_import', { filePath, conflictStrategy });
  writeItemsImportTemplate: ApiClient['writeItemsImportTemplate'] = (destPath, format) =>
    invokeCmd('write_import_template', { kind: 'ITEMS', destPath, format });
  validateServiceComponentsImport: ApiClient['validateServiceComponentsImport'] = (filePath) =>
    invokeCmd('validate_service_components_import', { filePath });
  executeServiceComponentsImport: ApiClient['executeServiceComponentsImport'] = (filePath) =>
    invokeCmd('execute_service_components_import', { filePath });
  validatePartiesImport: ApiClient['validatePartiesImport'] = (filePath, conflictStrategy) =>
    invokeCmd('validate_parties_import', { filePath, conflictStrategy });
  executePartiesImport: ApiClient['executePartiesImport'] = (filePath, conflictStrategy) =>
    invokeCmd('execute_parties_import', { filePath, conflictStrategy });
  validateExpensesImport: ApiClient['validateExpensesImport'] = (filePath) =>
    invokeCmd('validate_expenses_import', { filePath });
  executeExpensesImport: ApiClient['executeExpensesImport'] = (filePath) =>
    invokeCmd('execute_expenses_import', { filePath });
  writeImportTemplate: ApiClient['writeImportTemplate'] = (kind, destPath, format) =>
    invokeCmd('write_import_template', { kind, destPath, format });
}
