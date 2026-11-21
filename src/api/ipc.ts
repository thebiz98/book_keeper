import { invoke } from '@tauri-apps/api/core';
import type {
  ApiClient,
  CreateSalePayload,
  SaleSummaryResponse,
  CreatePurchasePayload,
  CreateItemPayload,
  UpdateItemPayload,
  CreateBundlePayload,
  SettlePaymentPayload,
  Item,
  Party,
  Sale,
  Purchase,
  PartyBalance,
  DashboardMetrics,
  TrendDataPoint,
  PnlReport,
  StockValuationReport,
  CashFlowReport,
  ShopSettings,
  BundleSuggestion,
  PaginatedList,
  BackupFileInfo,
  BackupInspection,
  MigrationReport,
} from '@/types';

/**
 * TauriClient: Production IPC abstraction calling Tauri v2 backend commands.
 * Normalizes frontend payloads into Rust Serde format and provides structured error handling.
 */
export class TauriClient implements ApiClient {
  private async call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
    try {
      return await invoke<T>(cmd, args);
    } catch (err: any) {
      const msg =
        typeof err === 'string'
          ? err
          : err?.message || `IPC command "${cmd}" failed`;
      console.error(`[TauriClient] Error calling "${cmd}":`, msg);
      throw new Error(msg);
    }
  }

  // --- Sales ---
  async getNextSaleCode(): Promise<string> {
    return this.call<string>('get_next_sale_code');
  }

  async createSale(
    payload: CreateSalePayload | any
  ): Promise<SaleSummaryResponse> {
    const rustPayload = {
      transaction_code: payload.transactionCode ?? payload.saleCode ?? null,
      transaction_date:
        payload.transactionDate ??
        payload.date ??
        new Date().toISOString().split('T')[0],
      party_id: payload.partyId ?? payload.customerId ?? null,
      paid_amount: payload.paidAmount ?? 0,
      payment_mode: payload.paymentMode ?? 'CASH',
      notes: payload.notes ?? null,
      items: (payload.items || []).map((i: any) => ({
        item_id: i.itemId ?? i.item_id,
        bundle_id: i.bundleId ?? i.bundle_id ?? null,
        bundle_quantity:
          i.bundleQuantity ?? i.bundle_quantity ?? i.quantity ?? 1,
        unit_price: i.unitPrice ?? i.unit_price ?? i.rate ?? null,
      })),
    };
    return this.call<SaleSummaryResponse>('create_sale', {
      payload: rustPayload,
    });
  }

  async getSalesHistory(filter?: any): Promise<PaginatedList<Sale>> {
    const rustFilter = filter
      ? {
          start_date:
            filter.startDate ??
            filter.start_date ??
            filter.fromDate ??
            filter.from_date,
          end_date:
            filter.endDate ??
            filter.end_date ??
            filter.toDate ??
            filter.to_date,
          payment_status: filter.paymentStatus ?? filter.payment_status,
          party_id: filter.partyId ?? filter.party_id,
          page: filter.page ?? 1,
          limit: filter.limit ?? 50,
        }
      : {};
    return this.call<PaginatedList<Sale>>('get_sales_history', {
      filter: rustFilter,
    });
  }

  async getSaleDetails(saleId: number): Promise<any> {
    return this.call('get_sale_details', { saleId });
  }

  // --- Purchases ---
  async getNextPurchaseCode(): Promise<string> {
    return this.call<string>('get_next_purchase_code');
  }

  async createPurchase(payload: CreatePurchasePayload | any): Promise<number> {
    const rustPayload = {
      transaction_code:
        payload.transactionCode ?? payload.purchaseCode ?? null,
      transaction_date:
        payload.transactionDate ??
        payload.date ??
        new Date().toISOString().split('T')[0],
      party_id: payload.partyId ?? payload.vendorId ?? null,
      paid_amount: payload.paidAmount ?? 0,
      payment_mode: payload.paymentMode ?? 'CASH',
      notes: payload.notes ?? null,
      items: (payload.items || []).map((i: any) => ({
        item_id: i.itemId ?? i.item_id,
        bundle_id: i.bundleId ?? i.bundle_id ?? null,
        bundle_quantity:
          i.bundleQuantity ?? i.bundle_quantity ?? i.quantity ?? 1,
        unit_cost: i.unitCost ?? i.unit_cost ?? 0,
      })),
    };
    return this.call<number>('create_purchase', { payload: rustPayload });
  }

  async getPurchasesHistory(filter?: any): Promise<PaginatedList<Purchase>> {
    const rustFilter = filter
      ? {
          start_date:
            filter.startDate ??
            filter.start_date ??
            filter.fromDate ??
            filter.from_date,
          end_date:
            filter.endDate ??
            filter.end_date ??
            filter.toDate ??
            filter.to_date,
          payment_status: filter.paymentStatus ?? filter.payment_status,
          party_id: filter.partyId ?? filter.party_id,
          page: filter.page ?? 1,
          limit: filter.limit ?? 50,
        }
      : {};
    return this.call<PaginatedList<Purchase>>('get_purchases_history', {
      filter: rustFilter,
    });
  }

  async getPurchaseDetails(purchaseId: number): Promise<any> {
    return this.call('get_purchase_details', { purchaseId });
  }

  // --- Items & Catalog ---
  async searchItemsFts(query: string, limit?: number): Promise<Item[]> {
    return this.call<Item[]>('search_items_fts', {
      query,
      limit: limit ?? 50,
    });
  }

  async getItemsCatalog(
    filter?: any
  ): Promise<{ total: number; items: Item[] }> {
    const category = filter?.category ?? null;
    const lowStockOnly =
      filter?.lowStockOnly ?? filter?.low_stock_only ?? false;
    const page = filter?.page ?? 1;
    const limit = filter?.limit ?? 50;
    const result = await this.call<any>('get_items_catalog', {
      category,
      lowStockOnly,
      page,
      limit,
    });
    return {
      total:
        result.total_count ??
        result.totalCount ??
        result.total ??
        result.items?.length ??
        0,
      items: result.items || [],
    };
  }

  async createItem(payload: CreateItemPayload | any): Promise<number> {
    const rustPayload = {
      name: payload.name,
      category: payload.category ?? null,
      barcode: payload.barcode ?? null,
      base_unit: payload.baseUnit ?? payload.base_unit ?? 'PCS',
      mrp: payload.mrp ?? null,
      default_sale_price:
        payload.defaultSalePrice ?? payload.default_sale_price ?? 0,
      initial_stock: payload.initialStock ?? payload.initial_stock ?? 0,
      initial_cost: payload.initialCost ?? payload.initial_cost ?? 0,
      bundles: payload.bundles ?? null,
    };
    return this.call<number>('create_item', { payload: rustPayload });
  }

  async updateItem(
    itemId: number,
    payload: UpdateItemPayload | any
  ): Promise<boolean> {
    const rustPayload = {
      name: payload.name,
      category: payload.category ?? null,
      barcode: payload.barcode ?? null,
      base_unit: payload.baseUnit ?? payload.base_unit ?? 'PCS',
      mrp: payload.mrp ?? null,
      default_sale_price:
        payload.defaultSalePrice ?? payload.default_sale_price ?? 0,
    };
    await this.call('update_item', { itemId, payload: rustPayload });
    return true;
  }

  async archiveItem(itemId: number): Promise<boolean> {
    await this.call('archive_item', { itemId });
    return true;
  }

  async getBundleSuggestions(): Promise<BundleSuggestion[]> {
    return this.call<BundleSuggestion[]>('get_bundle_suggestions');
  }

  async createBundle(payload: CreateBundlePayload | any): Promise<number> {
    const rustPayload = {
      item_id: payload.itemId ?? payload.item_id ?? null,
      bundle_name: payload.bundleName ?? payload.bundle_name ?? payload.name,
      units_per_bundle:
        payload.unitsPerBundle ??
        payload.units_per_bundle ??
        payload.multiplier ??
        1,
      mrp: payload.mrp ?? null,
      default_bundle_price:
        payload.defaultBundlePrice ??
        payload.default_bundle_price ??
        payload.salePrice ??
        payload.sale_price ??
        null,
      barcode: payload.barcode ?? null,
    };
    return this.call<number>('create_bundle', { payload: rustPayload });
  }

  // --- Dues & Khata ---
  async getPartyDuesSummary(
    partyType: 'CUSTOMER' | 'VENDOR' | 'Customer' | 'Vendor'
  ): Promise<PartyBalance[]> {
    const normalizedType =
      partyType.toUpperCase() === 'CUSTOMER' ? 'Customer' : 'Vendor';
    return this.call<PartyBalance[]>('get_party_dues_summary', {
      partyType: normalizedType,
    });
  }

  async settleBillPayment(payload: SettlePaymentPayload | any): Promise<boolean> {
    const rustPayload = {
      party_id: payload.partyId ?? payload.party_id,
      amount: payload.amount,
      settlement_date:
        payload.settlementDate ??
        payload.date ??
        new Date().toISOString().split('T')[0],
      payment_mode: payload.paymentMode ?? 'CASH',
      specific_bill_id:
        payload.specificBillId ?? payload.specific_bill_id ?? null,
      notes: payload.notes ?? null,
    };
    await this.call('settle_bill_payment', { payload: rustPayload });
    return true;
  }

  async getPartyLedger(partyId: number): Promise<any> {
    return this.call('get_party_ledger', { partyId });
  }

  // --- Parties ---
  async createParty(payload: any): Promise<number> {
    const rustPayload = {
      name: payload.name,
      phone: payload.phone ?? null,
      address: payload.address ?? null,
      party_type: payload.partyType ?? payload.party_type,
      notes: payload.notes ?? null,
    };
    return this.call<number>('create_party', { payload: rustPayload });
  }

  async updateParty(partyId: number, payload: any): Promise<boolean> {
    const rustPayload = {
      name: payload.name,
      phone: payload.phone ?? null,
      address: payload.address ?? null,
      party_type: payload.partyType ?? payload.party_type,
      notes: payload.notes ?? null,
    };
    await this.call('update_party', { partyId, payload: rustPayload });
    return true;
  }

  async archiveParty(partyId: number): Promise<boolean> {
    await this.call('archive_party', { partyId });
    return true;
  }

  async getParty(partyId: number): Promise<Party> {
    return this.call<Party>('get_party', { partyId });
  }

  async listParties(
    partyType?: 'CUSTOMER' | 'VENDOR' | 'Customer' | 'Vendor',
    activeOnly = true
  ): Promise<Party[]> {
    const normalizedType = partyType
      ? partyType.toUpperCase() === 'CUSTOMER'
        ? 'Customer'
        : 'Vendor'
      : null;
    return this.call<Party[]>('list_parties', {
      partyType: normalizedType,
      activeOnly,
    });
  }

  async searchParties(
    query: string,
    partyType?: 'CUSTOMER' | 'VENDOR' | 'Customer' | 'Vendor'
  ): Promise<Party[]> {
    const normalizedType = partyType
      ? partyType.toUpperCase() === 'CUSTOMER'
        ? 'Customer'
        : 'Vendor'
      : null;
    return this.call<Party[]>('search_parties', {
      query,
      partyType: normalizedType,
    });
  }

  // --- Returns & Adjustments ---
  async getNextReturnCode(): Promise<string> {
    return this.call<string>('get_next_return_code');
  }

  async createCustomerReturn(payload: any): Promise<string> {
    return this.call<string>('create_customer_return', { payload });
  }

  async createVendorReturn(payload: any): Promise<string> {
    return this.call<string>('create_vendor_return', { payload });
  }

  async getReturnDetails(returnId: number): Promise<any> {
    return this.call('get_return_details', { returnId });
  }

  async createStockAdjustment(payload: any): Promise<number> {
    return this.call<number>('create_stock_adjustment', { payload });
  }

  async listStockAdjustments(
    itemId?: number,
    page = 1,
    limit = 50
  ): Promise<any> {
    return this.call('list_stock_adjustments', {
      itemId: itemId ?? null,
      page,
      limit,
    });
  }

  // --- Reports ---
  async getDashboardMetrics(): Promise<DashboardMetrics> {
    return this.call<DashboardMetrics>('get_dashboard_metrics');
  }

  async getLowStockAlerts(): Promise<Item[]> {
    return this.call<Item[]>('get_low_stock_alerts');
  }

  async getSalesPurchaseTrend(
    period: '7d' | '30d' | '1y' | string
  ): Promise<TrendDataPoint[]> {
    return this.call<TrendDataPoint[]>('get_sales_purchase_trend', { period });
  }

  async getPnLReport(fromDate: string, toDate: string): Promise<PnlReport> {
    return this.call<PnlReport>('get_pnl_report', { fromDate, toDate });
  }

  async getStockValuationReport(): Promise<StockValuationReport> {
    return this.call<StockValuationReport>('get_stock_valuation_report');
  }

  async getCashFlowReport(
    fromDate: string,
    toDate: string
  ): Promise<CashFlowReport> {
    return this.call<CashFlowReport>('get_cash_flow_report', {
      fromDate,
      toDate,
    });
  }

  // --- Settings & Maintenance ---
  async getShopSettings(): Promise<ShopSettings> {
    return this.call<ShopSettings>('get_shop_settings');
  }

  async updateShopSettings(settings: ShopSettings | any): Promise<boolean> {
    const rustPayload = {
      id: settings.id ?? 1,
      shop_name: settings.shopName ?? settings.shop_name,
      phone: settings.phone ?? null,
      address: settings.address ?? null,
      tax_id: settings.taxId ?? settings.tax_id ?? null,
    };
    await this.call('update_shop_settings', { settings: rustPayload });
    return true;
  }

  async backupDatabase(destinationPath: string): Promise<string> {
    return this.call<string>('backup_database', { destinationPath });
  }

  async verifyDatabaseIntegrity(): Promise<boolean> {
    return this.call<boolean>('verify_database_integrity');
  }

  // --- Additional Tauri Commands ---
  async scanBackupDirectory(directoryPath: string): Promise<BackupFileInfo[]> {
    return this.call('scan_backup_directory', { directoryPath });
  }

  async inspectBackupFile(filePath: string): Promise<BackupInspection> {
    return this.call('inspect_backup_file', { filePath });
  }

  async executeMigration(filePath: string): Promise<MigrationReport> {
    return this.call('execute_migration', { filePath });
  }

  async exportItemsToExcel(destinationPath: string): Promise<string> {
    return this.call('export_items_to_excel', { destinationPath });
  }

  async importItemsFromExcel(filePath: string): Promise<MigrationReport> {
    return this.call('import_items_from_excel', { filePath });
  }

  async exportTransactionsToExcel(
    destinationPath: string,
    filter?: any
  ): Promise<string> {
    return this.call('export_transactions_to_excel', {
      destinationPath,
      filter: filter ?? null,
    });
  }
}
