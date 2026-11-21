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
  PaymentStatus,
  PnlReport,
  Purchase,
  PurchaseDetailView,
  PurchaseItem,
  ReturnDetailView,
  Sale,
  SaleDetailView,
  SaleItem,
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
import {
  initialItems,
  initialCustomers,
  initialVendors,
  initialShopSettings,
  initialBundleSuggestions,
  type MockItem,
  type MockParty,
  type MockBundle,
} from './seedData';

export type { MockItem, MockParty, MockBundle };

interface SaleRecord {
  sale: Sale;
  items: SaleItem[];
}

interface PurchaseRecord {
  purchase: Purchase;
  items: PurchaseItem[];
}

const LOW_STOCK_THRESHOLD = 5;
const nowIso = () => new Date().toISOString();

function paginate<T>(all: T[], page = 1, limit = 50): PaginatedList<T> {
  const start = (page - 1) * limit;
  return {
    items: all.slice(start, start + limit),
    totalCount: all.length,
    page,
    limit,
    totalPages: Math.ceil(all.length / limit) || 1,
  };
}

function statusFor(total: number, paid: number): PaymentStatus {
  if (paid >= total) return 'Paid';
  return paid > 0 ? 'Partial' : 'Unpaid';
}

/** In-memory ApiClient returning the same camelCase shapes as the Tauri client. */
export class MockApiClient implements ApiClient {
  private items: MockItem[];
  private parties: MockParty[];
  private sales: SaleRecord[] = [];
  private purchases: PurchaseRecord[] = [];
  private adjustments: StockAdjustment[] = [];
  private settings: ShopSettings;
  private bundleSuggestions: BundleSuggestion[];
  private saleCounter = 1001;
  private purchaseCounter = 1001;
  private cashDrawer = {
    salesCash: 4000.0,
    customerDuesSettled: 500.0,
    purchaseCash: 1500.0,
    vendorDuesSettled: 200.0,
    refunds: 100.0,
  };

  constructor() {
    this.items = structuredClone(initialItems);
    this.parties = [
      ...structuredClone(initialCustomers),
      ...structuredClone(initialVendors),
    ];
    this.settings = structuredClone(initialShopSettings);
    this.bundleSuggestions = structuredClone(initialBundleSuggestions);
  }

  // --- helpers ---
  private findItem(id: number | null | undefined): MockItem | undefined {
    return this.items.find((i) => i.id === id);
  }

  private findParty(id: number | null | undefined): MockParty | undefined {
    return this.parties.find((p) => p.id === id);
  }

  private unitsFor(item: MockItem | undefined, bundleId: number | null): number {
    if (bundleId === null) return 1;
    return item?.bundles.find((b) => b.id === bundleId)?.unitsPerBundle ?? 1;
  }

  private adjustStock(item: MockItem, delta: number): void {
    item.currentStock = Math.max(0, item.currentStock + delta);
    item.isLowStock = item.currentStock < LOW_STOCK_THRESHOLD;
  }

  private addDue(party: MockParty | undefined, delta: number): void {
    if (party) party.balanceDue = Math.max(0, party.balanceDue + delta);
  }

  private activeItems(): MockItem[] {
    return this.items.filter((i) => i.isActive);
  }

  private toSearchResult(i: MockItem): ItemSearchResult {
    return {
      itemId: i.id,
      name: i.name,
      category: i.category,
      barcode: i.barcode,
      baseUnit: i.baseUnit,
      mrp: i.mrp,
      defaultSalePrice: i.defaultSalePrice,
      currentStock: i.currentStock,
      isLowStock: i.isLowStock,
      bundles: i.bundles.map((b) => ({
        bundleId: b.id,
        bundleName: b.bundleName,
        unitsPerBundle: b.unitsPerBundle,
        bundlePrice: b.defaultBundlePrice,
        barcode: b.barcode,
      })),
    };
  }

  private filterHistory<T extends { transactionDate: string; paymentStatus: PaymentStatus; partyId: number | null }>(
    rows: T[],
    f?: HistoryFilter
  ): T[] {
    return rows.filter(
      (r) =>
        (!f?.startDate || r.transactionDate >= f.startDate) &&
        (!f?.endDate || r.transactionDate <= f.endDate) &&
        (!f?.paymentStatus || r.paymentStatus === f.paymentStatus) &&
        (!f?.partyId || r.partyId === f.partyId)
    );
  }

  // --- Items & Catalog ---
  async searchItemsFts(query: string, limit = 50): Promise<ItemSearchResult[]> {
    const q = (query || '').trim().toLowerCase();
    const active = this.activeItems();
    if (!q) return active.slice(0, limit).map((i) => this.toSearchResult(i));

    const exact = active.find((i) => i.barcode === query.trim());
    if (exact) return [this.toSearchResult(exact)];

    return active
      .filter((i) =>
        `${i.name} ${i.category ?? ''} ${i.barcode ?? ''}`.toLowerCase().includes(q)
      )
      .slice(0, limit)
      .map((i) => this.toSearchResult(i));
  }

  async getItemsCatalog(filter?: CatalogFilter): Promise<PaginatedList<Item>> {
    let rows: Item[] = this.activeItems();
    if (filter?.category) {
      const c = filter.category.toLowerCase();
      rows = rows.filter((i) => (i.category ?? '').toLowerCase() === c);
    }
    if (filter?.lowStockOnly) {
      rows = rows.filter((i) => i.currentStock < LOW_STOCK_THRESHOLD);
    }
    return paginate(rows, filter?.page ?? 1, filter?.limit ?? 500);
  }

  async createItem(payload: CreateItemPayload): Promise<number> {
    const id = Math.max(0, ...this.items.map((i) => i.id)) + 1;
    const stock = payload.initialStock ?? 0;
    const ts = nowIso();
    this.items.push({
      id,
      name: payload.name,
      category: payload.category || 'General Stationery',
      barcode: payload.barcode || `890103038${id}`,
      baseUnit: payload.baseUnit || 'PCS',
      mrp: payload.mrp ?? payload.defaultSalePrice,
      defaultSalePrice: payload.defaultSalePrice,
      currentStock: stock,
      isLowStock: stock < LOW_STOCK_THRESHOLD,
      isActive: true,
      createdAt: ts,
      updatedAt: ts,
      costPrice: payload.initialCost ?? 0,
      bundles: [],
    });
    const created = this.items[this.items.length - 1];
    let nextBundleId = Math.floor(Math.random() * 100000);
    for (const b of payload.bundles ?? []) {
      created.bundles.push({
        id: nextBundleId++,
        itemId: id,
        bundleName: b.bundleName,
        unitsPerBundle: b.unitsPerBundle,
        mrp: b.mrp,
        defaultBundlePrice: b.defaultBundlePrice,
        barcode: b.barcode,
      });
    }
    return id;
  }

  async updateItem(itemId: number, payload: UpdateItemPayload): Promise<void> {
    const item = this.findItem(itemId);
    if (!item) throw new Error(`Item ${itemId} not found`);
    Object.assign(item, payload, { updatedAt: nowIso() });
  }

  async archiveItem(itemId: number): Promise<void> {
    const item = this.findItem(itemId);
    if (item) item.isActive = false;
  }

  async getBundleSuggestions(): Promise<BundleSuggestion[]> {
    return this.bundleSuggestions;
  }

  async createBundle(payload: CreateBundlePayload): Promise<number> {
    const id = Math.floor(Math.random() * 100000);
    const item = this.findItem(payload.itemId);
    item?.bundles.push({
      id,
      itemId: payload.itemId ?? 0,
      bundleName: payload.bundleName,
      unitsPerBundle: payload.unitsPerBundle,
      mrp: payload.mrp,
      defaultBundlePrice: payload.defaultBundlePrice,
      barcode: payload.barcode,
    });
    return id;
  }

  // --- Sales ---
  async getNextSaleCode(): Promise<string> {
    return `SAL-${this.saleCounter++}`;
  }

  async createSale(payload: CreateSalePayload): Promise<SaleSummaryResponse> {
    const code = payload.transactionCode || (await this.getNextSaleCode());
    const id = this.sales.length + 1;
    let total = 0;
    const lines: SaleItem[] = [];

    for (const line of payload.items) {
      const item = this.findItem(line.itemId);
      const units = this.unitsFor(item, line.bundleId);
      const rate = line.unitPrice ?? item?.defaultSalePrice ?? 0;
      const lineTotal = line.bundleQuantity * rate;
      total += lineTotal;
      if (item) this.adjustStock(item, -(line.bundleQuantity * units));
      lines.push({
        id: lines.length + 1,
        saleTransactionId: id,
        itemId: line.itemId,
        bundleId: line.bundleId,
        bundleQuantity: line.bundleQuantity,
        quantity: line.bundleQuantity * units,
        unitPrice: units > 0 ? rate / units : rate,
        totalAmount: lineTotal,
      });
    }

    const paid = payload.paidAmount;
    const balance = Math.max(0, total - paid);
    const change = Math.max(0, paid - total);
    if (balance > 0) this.addDue(this.findParty(payload.partyId), balance);
    this.cashDrawer.salesCash += Math.min(paid, total);

    const paymentStatus = statusFor(total, paid);
    const sale: Sale = {
      id,
      transactionCode: code,
      transactionDate: payload.transactionDate,
      partyId: payload.partyId,
      paymentMode: payload.paymentMode,
      paymentStatus,
      totalAmount: total,
      paidAmount: paid,
      balanceDue: balance,
      notes: payload.notes,
      createdAt: nowIso(),
    };
    this.sales.push({ sale, items: lines });
    return {
      saleId: id,
      transactionCode: code,
      totalAmount: total,
      paidAmount: paid,
      balanceDue: balance,
      changeAmount: change,
      changeReturned: change,
      paymentStatus,
    };
  }

  async getSalesHistory(filter?: HistoryFilter): Promise<PaginatedList<Sale>> {
    const rows = this.filterHistory(
      this.sales.map((r) => r.sale),
      filter
    );
    return paginate(rows, filter?.page ?? 1, filter?.limit ?? 50);
  }

  async getSaleDetails(saleId: number): Promise<SaleDetailView> {
    const rec = this.sales.find((s) => s.sale.id === saleId);
    if (!rec) throw new Error(`Sale ${saleId} not found`);
    return {
      sale: rec.sale,
      party: this.findParty(rec.sale.partyId) ?? null,
      items: rec.items.map((saleItem) => ({
        saleItem,
        itemName: this.findItem(saleItem.itemId)?.name ?? '',
        bundleName:
          this.findItem(saleItem.itemId)?.bundles.find((b) => b.id === saleItem.bundleId)
            ?.bundleName ?? null,
        allocations: [],
      })),
      settlements: [],
    };
  }

  // --- Purchases ---
  async getNextPurchaseCode(): Promise<string> {
    return `PUR-${this.purchaseCounter++}`;
  }

  async createPurchase(payload: CreatePurchasePayload): Promise<number> {
    const code = payload.transactionCode || (await this.getNextPurchaseCode());
    const id = this.purchases.length + 1;
    let total = 0;
    const lines: PurchaseItem[] = [];

    for (const line of payload.items) {
      const item = this.findItem(line.itemId);
      const units = this.unitsFor(item, line.bundleId);
      const lineTotal = line.bundleQuantity * line.unitCost;
      total += lineTotal;
      if (item) this.adjustStock(item, line.bundleQuantity * units);
      lines.push({
        id: lines.length + 1,
        purchaseTransactionId: id,
        itemId: line.itemId,
        bundleId: line.bundleId,
        bundleQuantity: line.bundleQuantity,
        quantity: line.bundleQuantity * units,
        unitCost: units > 0 ? line.unitCost / units : line.unitCost,
        totalAmount: lineTotal,
        remainingQuantity: line.bundleQuantity * units,
      });
    }

    const paid = payload.paidAmount;
    const balance = Math.max(0, total - paid);
    if (balance > 0) this.addDue(this.findParty(payload.partyId), balance);
    this.cashDrawer.purchaseCash += Math.min(paid, total);

    const purchase: Purchase = {
      id,
      transactionCode: code,
      transactionDate: payload.transactionDate,
      partyId: payload.partyId,
      paymentMode: payload.paymentMode,
      paymentStatus: statusFor(total, paid),
      totalAmount: total,
      paidAmount: paid,
      balanceDue: balance,
      notes: payload.notes,
      createdAt: nowIso(),
    };
    this.purchases.push({ purchase, items: lines });
    return id;
  }

  async getPurchasesHistory(filter?: HistoryFilter): Promise<PaginatedList<Purchase>> {
    const rows = this.filterHistory(
      this.purchases.map((r) => r.purchase),
      filter
    );
    return paginate(rows, filter?.page ?? 1, filter?.limit ?? 50);
  }

  async getPurchaseDetails(purchaseId: number): Promise<PurchaseDetailView> {
    const rec = this.purchases.find((p) => p.purchase.id === purchaseId);
    if (!rec) throw new Error(`Purchase ${purchaseId} not found`);
    return {
      purchase: rec.purchase,
      party: this.findParty(rec.purchase.partyId) ?? null,
      items: rec.items.map((purchaseItem) => ({
        purchaseItem,
        itemName: this.findItem(purchaseItem.itemId)?.name ?? '',
        bundleName:
          this.findItem(purchaseItem.itemId)?.bundles.find(
            (b) => b.id === purchaseItem.bundleId
          )?.bundleName ?? null,
      })),
      settlements: [],
    };
  }

  // --- Parties & Accounting ---
  async listParties(partyType?: PartyType, activeOnly = true): Promise<Party[]> {
    return this.parties.filter(
      (p) => (!activeOnly || p.isActive) && (!partyType || p.partyType === partyType)
    );
  }

  async getParty(partyId: number): Promise<Party> {
    const party = this.findParty(partyId);
    if (!party) throw new Error(`Party ${partyId} not found`);
    return party;
  }

  async getPartyDuesSummary(partyType: PartyType): Promise<PartyBalance[]> {
    return this.parties
      .filter((p) => p.partyType === partyType && p.balanceDue > 0)
      .map((p) => ({
        partyId: p.id,
        partyName: p.name,
        phone: p.phone,
        partyType: p.partyType,
        totalDue: p.balanceDue,
        oldestBillDate: p.oldestBillDate,
        unpaidBillsCount: p.unpaidBillsCount,
      }));
  }

  async settleBillPayment(payload: SettlePaymentPayload): Promise<void> {
    const party = this.findParty(payload.partyId);
    if (!party) throw new Error(`Party ${payload.partyId} not found`);
    this.addDue(party, -payload.amount);
    if (party.partyType === 'Customer') {
      this.cashDrawer.customerDuesSettled += payload.amount;
    } else {
      this.cashDrawer.vendorDuesSettled += payload.amount;
    }
  }

  async getPartyLedger(partyId: number): Promise<PartyLedgerView> {
    const party = await this.getParty(partyId);
    return {
      party,
      currentBalance: this.findParty(partyId)?.balanceDue ?? 0,
      entries: [],
    };
  }

  async createParty(payload: CreatePartyPayload): Promise<number> {
    const id = Math.max(0, ...this.parties.map((p) => p.id)) + 1;
    const ts = nowIso();
    this.parties.push({
      id,
      name: payload.name,
      phone: payload.phone,
      partyType: payload.partyType,
      notes: payload.notes,
      isActive: true,
      createdAt: ts,
      updatedAt: ts,
      balanceDue: 0,
      oldestBillDate: null,
      unpaidBillsCount: 0,
    });
    return id;
  }

  async updateParty(partyId: number, payload: UpdatePartyPayload): Promise<void> {
    const party = this.findParty(partyId);
    if (!party) throw new Error(`Party ${partyId} not found`);
    Object.assign(party, payload, { updatedAt: nowIso() });
  }

  async archiveParty(partyId: number): Promise<void> {
    const party = this.findParty(partyId);
    if (party) party.isActive = false;
  }

  async searchParties(query: string, partyType?: PartyType): Promise<Party[]> {
    const q = (query || '').toLowerCase();
    return this.parties.filter(
      (p) =>
        (!partyType || p.partyType === partyType) &&
        (p.name.toLowerCase().includes(q) || (p.phone?.includes(q) ?? false))
    );
  }

  // --- Reports & Dashboard ---
  async getLowStockAlerts(): Promise<LowStockItem[]> {
    return this.activeItems()
      .filter((i) => i.currentStock < LOW_STOCK_THRESHOLD)
      .map((i) => ({
        id: i.id,
        itemId: i.id,
        name: i.name,
        category: i.category,
        baseUnit: i.baseUnit,
        currentStock: i.currentStock,
        defaultSalePrice: i.defaultSalePrice,
        mrp: i.mrp,
      }));
  }

  private stockValuation(): number {
    return this.items.reduce((sum, i) => sum + i.currentStock * i.costPrice, 0);
  }

  private dueTotal(type: PartyType): number {
    return this.parties
      .filter((p) => p.partyType === type)
      .reduce((sum, p) => sum + p.balanceDue, 0);
  }

  async getDashboardMetrics(): Promise<DashboardMetrics> {
    const d = this.cashDrawer;
    return {
      todaySales: 4200.0,
      todayPurchases: 1500.0,
      todaySalesAmount: 4200.0,
      todaySalesCount: 12,
      todayPurchaseAmount: 1500.0,
      todayPurchaseCount: 2,
      todayNetProfit: 1450.0,
      totalStockValuation: this.stockValuation(),
      customerReceivables: this.dueTotal('Customer'),
      vendorPayables: this.dueTotal('Vendor'),
      liveCashInDrawer:
        d.salesCash + d.customerDuesSettled - (d.purchaseCash + d.vendorDuesSettled + d.refunds),
    };
  }

  async getPnLReport(fromDate: string, toDate: string): Promise<PnlReport> {
    const grossSales = 45000.0;
    const cogs = 28500.0;
    return {
      fromDate,
      toDate,
      grossSales,
      cogs,
      netProfit: grossSales - cogs,
      totalDiscounts: 850.0,
      returnsRefunded: 350.0,
    };
  }

  async getStockValuationReport(): Promise<StockValuationReport> {
    return {
      totalValuation: this.stockValuation(),
      activeLotsCount: this.items.length,
      totalItemsCount: this.items.length,
      categoriesBreakdown: [],
      categories: [],
    };
  }

  async getCashFlowReport(fromDate: string, toDate: string): Promise<CashFlowReport> {
    const d = this.cashDrawer;
    const inflow = d.salesCash + d.customerDuesSettled;
    const outflow = d.purchaseCash + d.vendorDuesSettled + d.refunds;
    return {
      fromDate,
      toDate,
      totalInflow: inflow,
      totalOutflow: outflow,
      netCashFlow: inflow - outflow,
      closingDrawerBalance: inflow - outflow,
      salesCashInflow: d.salesCash,
      customerSettlementsInflow: d.customerDuesSettled,
      purchaseCashOutflow: d.purchaseCash,
      vendorSettlementsOutflow: d.vendorDuesSettled,
      refundsCashOutflow: d.refunds,
      netCashBalance: inflow - outflow,
    };
  }

  async getSalesPurchaseTrend(_period: string): Promise<TrendDataPoint[]> {
    const rows: [string, number, number][] = [
      ['2026-11-14', 3200, 1100],
      ['2026-11-15', 4100, 800],
      ['2026-11-16', 2900, 1500],
      ['2026-11-17', 5200, 2200],
      ['2026-11-18', 3800, 950],
      ['2026-11-19', 4600, 1400],
      ['2026-11-20', 4200, 1500],
    ];
    return rows.map(([date, salesAmount, purchaseAmount]) => ({
      date,
      salesAmount,
      purchaseAmount,
    }));
  }

  // --- Returns & Adjustments ---
  async getNextReturnCode(): Promise<string> {
    return `RET-${Math.floor(1000 + Math.random() * 9000)}`;
  }

  async createCustomerReturn(payload: CustomerReturnPayload): Promise<string> {
    let refund = 0;
    for (const line of payload.items) {
      const item = this.findItem(line.itemId);
      if (item && line.condition === 'Restockable') this.adjustStock(item, line.quantity);
      refund += line.quantity * line.unitPrice;
    }
    if (payload.settlementType === 'CashRefund') this.cashDrawer.refunds += refund;
    return payload.returnCode || `RET-SAL-${Math.floor(1000 + Math.random() * 9000)}`;
  }

  async createVendorReturn(payload: VendorReturnPayload): Promise<string> {
    for (const line of payload.items) {
      const item = this.findItem(line.itemId);
      if (item) this.adjustStock(item, -line.quantity);
    }
    return `RET-PUR-${Math.floor(1000 + Math.random() * 9000)}`;
  }

  async getReturnDetails(returnId: number): Promise<ReturnDetailView> {
    return {
      returnTx: {
        id: returnId,
        returnCode: `RET-${returnId}`,
        returnDate: '2026-11-20',
        returnType: 'Customer',
        partyId: null,
        originalTransactionId: null,
        settlementType: 'CashRefund',
        totalAmount: 0,
        reason: null,
        notes: null,
        createdAt: nowIso(),
      },
      party: null,
      items: [],
    };
  }

  async createStockAdjustment(payload: StockAdjustmentPayload): Promise<number> {
    const item = this.findItem(payload.itemId);
    if (item) this.adjustStock(item, payload.quantity);
    const id = this.adjustments.length + 1;
    this.adjustments.push({ id, ...payload, createdAt: nowIso() });
    return id;
  }

  async listStockAdjustments(
    itemId?: number,
    page = 1,
    limit = 50
  ): Promise<PaginatedList<StockAdjustment>> {
    return paginate(
      this.adjustments.filter((a) => itemId === undefined || a.itemId === itemId),
      page,
      limit
    );
  }

  // --- Shop Settings & Backup ---
  async getShopSettings(): Promise<ShopSettings> {
    return this.settings;
  }

  async updateShopSettings(settings: ShopSettings): Promise<void> {
    this.settings = { ...this.settings, ...settings };
  }

  async backupDatabase(destinationPath: string): Promise<string> {
    return destinationPath || '/backup/shop_backup.db';
  }

  async verifyDatabaseIntegrity(): Promise<boolean> {
    return true;
  }

  // --- Migration & Excel ---
  async scanBackupDirectory(directoryPath?: string): Promise<BackupFileInfo[]> {
    const dir = (directoryPath || '/backup').replace(/\/$/, '');
    return [
      {
        fileName: 'vedbill_backup.db',
        filePath: `${dir}/vedbill_backup.db`,
        fileSizeBytes: 245760,
        modifiedAt: nowIso(),
      },
    ];
  }

  async inspectBackupFile(filePath: string): Promise<BackupInspection> {
    return {
      companyName: this.settings.shopName,
      schemaVersion: 1,
      createdAt: nowIso(),
      backupFilePath: filePath,
      backupFileSizeBytes: 245760,
      tableStats: {
        itemsCount: this.items.length,
        partiesCount: this.parties.length,
        salesCount: this.sales.length,
        purchasesCount: this.purchases.length,
        documentItemsCount: 0,
      },
    };
  }

  async executeMigration(_filePath: string): Promise<MigrationReport> {
    return {
      success: true,
      itemsImported: this.items.length,
      partiesImported: this.parties.length,
      salesImported: 0,
      purchasesImported: 0,
      durationMs: 0,
      errors: [],
    };
  }

  async exportItemsToExcel(destinationPath: string): Promise<string> {
    return destinationPath || '/export/items.xlsx';
  }

  async importItemsFromExcel(_filePath: string): Promise<ImportSummary> {
    return { totalRows: 0, importedCount: 0, skippedCount: 0, errors: [] };
  }

  async exportTransactionsToExcel(
    destinationPath: string,
    _filter?: TransactionExportFilter | null
  ): Promise<string> {
    return destinationPath || '/export/transactions.xlsx';
  }
}

export const mockApiClient = new MockApiClient();
