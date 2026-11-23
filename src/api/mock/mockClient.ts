/**
 * Thin FIXTURE ApiClient for browser dev and tests (DD-27).
 *
 * Returns contract-shaped data from seed fixtures and appends a few rows in
 * memory so screens feel alive. It deliberately implements NO business rules:
 * no FIFO, ledger, stock guards or validation. The Rust backend is the only
 * source of truth. The single simulated failure is `searchItemsFts('__error__')`,
 * which rejects with a ValidationError-shaped AppError for UI error testing.
 */
import type {
  ApiClient,
  CashFlowReport,
  ItemBundle,
  ItemSearchResult,
  Paginated,
  Party,
  PartyDueSummary,
  PartyListRow,
  PartyType,
  PurchaseDetailView,
  PurchaseHistoryRow,
  ReturnDetailView,
  ReturnTransaction,
  SaleDetailView,
  SaleHistoryRow,
  ShopSettings,
  AppConfigView,
  ResetScope,
  ResetSummary,
  StockAdjustment,
} from '@/types';
import type { AppError } from '../errors';
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

const MOCK_DEFAULT_BACKUP_DIR = 'C:/mock/backups';
const nowIso = () => new Date().toISOString();
const localDate = (d = new Date()) =>
  `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`;

function paginate<T>(all: T[], page?: number | null, limit?: number | null): Paginated<T> {
  const p = page ?? 1;
  const l = limit ?? 50;
  return { items: all.slice((p - 1) * l, p * l), total: all.length, page: p, limit: l };
}

function toSearchResult(i: MockItem, matchedBundleId: number | null = null): ItemSearchResult {
  return {
    itemId: i.id,
    name: i.name,
    category: i.category,
    barcode: i.barcode,
    baseUnit: i.baseUnit,
    itemType: i.itemType,
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
    matchedBundleId,
  };
}

const toParty = (p: MockParty): Party => ({
  id: p.id,
  name: p.name,
  phone: p.phone,
  partyType: p.partyType,
  notes: p.notes,
  isActive: p.isActive,
  createdAt: p.createdAt,
  updatedAt: p.updatedAt,
});

const toListRow = (p: MockParty): PartyListRow => ({
  ...toParty(p),
  outstandingBalance: p.balanceDue,
  transactionCount: p.unpaidBillsCount,
});

function notFound(entity: string, id: number): AppError {
  return { type: 'NotFound', details: { entity, id: String(id) } };
}

/** In-memory fixture ApiClient returning the same camelCase shapes as the Tauri client. */
export class MockApiClient implements ApiClient {
  private items: MockItem[] = structuredClone(initialItems);
  private parties: MockParty[] = [
    ...structuredClone(initialCustomers),
    ...structuredClone(initialVendors),
  ];
  private settings: ShopSettings = structuredClone(initialShopSettings);
  private sales: SaleHistoryRow[] = [];
  private purchases: PurchaseHistoryRow[] = [];
  private returns: ReturnTransaction[] = [];
  private adjustments: StockAdjustment[] = [];
  private nextId = 1;
  private nextBundleId = 100000;

  private id() {
    return this.nextId++;
  }
  private partyName(id: number | null) {
    return this.parties.find((p) => p.id === id)?.name ?? null;
  }
  private findParty(id: number): MockParty {
    const p = this.parties.find((x) => x.id === id);
    if (!p) throw notFound('Party', id);
    return p;
  }

  // --- Sales ---
  async getNextSaleCode() {
    return `SAL-${1001 + this.sales.length}`;
  }
  async createSale(payload: Parameters<ApiClient['createSale']>[0]) {
    const total = payload.items.reduce((s, l) => s + l.bundleQuantity * l.unitPrice, 0);
    const paid = Math.min(payload.paidAmount, total);
    const id = this.id();
    const code = `SAL-${1001 + this.sales.length}`;
    const row: SaleHistoryRow = {
      id,
      transactionCode: code,
      transactionDate: payload.transactionDate,
      createdAt: nowIso(),
      partyId: payload.partyId,
      partyName: this.partyName(payload.partyId),
      lineCount: payload.items.length,
      totalAmount: total,
      paidAmount: paid,
      balanceDue: total - paid,
      paymentStatus: payload.paymentStatus,
    };
    this.sales.unshift(row);
    return {
      saleId: id,
      transactionCode: code,
      totalAmount: total,
      paidAmount: paid,
      balanceDue: total - paid,
      changeAmount: Math.max(0, payload.paidAmount - total),
      paymentStatus: payload.paymentStatus,
    };
  }
  async getSalesHistory(filter: Parameters<ApiClient['getSalesHistory']>[0] = {}) {
    return paginate(this.sales, filter?.page, filter?.limit);
  }
  async getSaleDetails(saleId: number): Promise<SaleDetailView> {
    const row = this.sales.find((s) => s.id === saleId);
    if (!row) throw notFound('SaleTransaction', saleId);
    const item = this.items[0];
    return {
      sale: {
        id: row.id,
        transactionCode: row.transactionCode,
        transactionDate: row.transactionDate,
        partyId: row.partyId,
        partyName: row.partyName,
        paymentMode: 'CASH',
        paymentStatus: row.paymentStatus,
        totalAmount: row.totalAmount,
        paidAmount: row.paidAmount,
        balanceDue: row.balanceDue,
        notes: null,
        createdAt: row.createdAt,
      },
      party: row.partyId ? toParty(this.findParty(row.partyId)) : null,
      items: [
        {
          saleItem: {
            id: 1,
            saleTransactionId: row.id,
            itemId: item.id,
            itemName: item.name,
            packLabel: null,
            packQuantity: 1,
            quantity: 1,
            unitPrice: row.totalAmount,
            totalAmount: row.totalAmount,
          },
          itemName: item.name,
          bundleName: null,
          returnedQuantity: 0,
          allocations: [],
        },
      ],
      settlements: [],
    };
  }

  // --- Purchases ---
  async getNextPurchaseCode() {
    return `PUR-${1001 + this.purchases.length}`;
  }
  async createPurchase(payload: Parameters<ApiClient['createPurchase']>[0]) {
    const total = payload.items.reduce((s, l) => s + l.bundleQuantity * l.unitCost, 0);
    const id = this.id();
    const code = `PUR-${1001 + this.purchases.length}`;
    this.purchases.unshift({
      id,
      transactionCode: code,
      supplierInvoiceRef: payload.supplierInvoiceRef,
      transactionDate: payload.transactionDate,
      createdAt: nowIso(),
      partyId: payload.partyId,
      partyName: this.partyName(payload.partyId),
      lineCount: payload.items.length,
      totalAmount: total,
      paidAmount: payload.paidAmount,
      balanceDue: total - payload.paidAmount,
      paymentStatus: payload.paymentStatus,
    });
    return { purchaseId: id, transactionCode: code };
  }
  async getPurchasesHistory(filter: Parameters<ApiClient['getPurchasesHistory']>[0] = {}) {
    return paginate(this.purchases, filter?.page, filter?.limit);
  }
  async getPurchaseDetails(purchaseId: number): Promise<PurchaseDetailView> {
    const row = this.purchases.find((p) => p.id === purchaseId);
    if (!row) throw notFound('PurchaseTransaction', purchaseId);
    const item = this.items[0];
    return {
      purchase: {
        id: row.id,
        transactionCode: row.transactionCode,
        source: 'PURCHASE',
        supplierInvoiceRef: row.supplierInvoiceRef,
        transactionDate: row.transactionDate,
        partyId: row.partyId,
        partyName: row.partyName,
        paymentMode: 'CASH',
        paymentStatus: row.paymentStatus,
        totalAmount: row.totalAmount,
        paidAmount: row.paidAmount,
        balanceDue: row.balanceDue,
        notes: null,
        createdAt: row.createdAt,
      },
      party: row.partyId ? toParty(this.findParty(row.partyId)) : null,
      items: [
        {
          purchaseItem: {
            id: 1,
            purchaseTransactionId: row.id,
            itemId: item.id,
            itemName: item.name,
            packLabel: null,
            packQuantity: 1,
            quantity: 1,
            unitCost: row.totalAmount,
            totalAmount: row.totalAmount,
            remainingQuantity: 1,
          },
          itemName: item.name,
          bundleName: null,
        },
      ],
      settlements: [],
    };
  }

  // --- Items & Catalog ---
  async searchItemsFts(query: string, limit = 8) {
    if (query === '__error__') {
      throw {
        type: 'ValidationError',
        details: { field: 'query', message: 'Simulated validation error' },
      } satisfies AppError;
    }
    const q = query.trim().toLowerCase();
    if (!q) return [];
    const out: ItemSearchResult[] = [];
    for (const i of this.items) {
      const bundle = i.bundles.find((b) => b.barcode === query);
      if (bundle) out.push(toSearchResult(i, bundle.id));
      else if (i.name.toLowerCase().includes(q) || i.barcode === query) out.push(toSearchResult(i));
      if (out.length >= limit) break;
    }
    return out;
  }
  async getItemsCatalog(filter: Parameters<ApiClient['getItemsCatalog']>[0] = {}) {
    let rows = this.items.filter((i) => i.isActive);
    if (filter?.category) {
      const c = filter.category.toLowerCase();
      rows = rows.filter((i) => i.category?.toLowerCase() === c);
    }
    if (filter?.lowStockOnly) rows = rows.filter((i) => i.isLowStock);
    if (filter?.search) {
      const q = filter.search.toLowerCase();
      rows = rows.filter((i) => i.name.toLowerCase().includes(q));
    }
    rows = [...rows].sort((a, b) => a.name.toLowerCase().localeCompare(b.name.toLowerCase()));
    const page = paginate(rows, filter?.page, filter?.limit);
    return { ...page, items: page.items.map((i) => toSearchResult(i)) };
  }
  async listCategories() {
    return [...new Set(this.items.map((i) => i.category).filter((c): c is string => !!c))].sort();
  }
  async createItem(payload: Parameters<ApiClient['createItem']>[0]) {
    const id = this.id() + 100000;
    this.items.push({
      id,
      name: payload.name,
      category: payload.category,
      barcode: payload.barcode,
      baseUnit: payload.baseUnit,
      itemType: payload.itemType ?? 'PHYSICAL',
      mrp: payload.mrp,
      defaultSalePrice: payload.defaultSalePrice,
      currentStock: payload.initialStock ?? 0,
      isLowStock: false,
      isActive: true,
      createdAt: nowIso(),
      updatedAt: nowIso(),
      costPrice: payload.initialCost ?? 0,
      bundles: [],
    });
    return id;
  }
  async updateItem(itemId: number, payload: Parameters<ApiClient['updateItem']>[1]) {
    const item = this.items.find((i) => i.id === itemId);
    if (!item) throw notFound('Item', itemId);
    const { itemType, ...rest } = payload;
    Object.assign(item, rest, itemType ? { itemType } : {}, { updatedAt: nowIso() });
  }
  async getBundleSuggestions() {
    return structuredClone(initialBundleSuggestions);
  }
  async createBundle(payload: Parameters<ApiClient['createBundle']>[0]): Promise<ItemBundle> {
    const bundle: ItemBundle = {
      id: this.nextBundleId++,
      itemId: payload.itemId ?? 0,
      bundleName: payload.bundleName,
      unitsPerBundle: payload.unitsPerBundle,
      mrp: payload.mrp,
      defaultBundlePrice: payload.defaultBundlePrice,
      barcode: payload.barcode,
    };
    this.items.find((i) => i.id === payload.itemId)?.bundles.push(bundle);
    return bundle;
  }
  async updateBundle(
    bundleId: number,
    payload: Parameters<ApiClient['updateBundle']>[1]
  ): Promise<ItemBundle> {
    for (const i of this.items) {
      const b = i.bundles.find((x) => x.id === bundleId);
      if (b) {
        Object.assign(b, payload);
        return { ...b };
      }
    }
    throw notFound('ItemBundle', bundleId);
  }
  async deleteBundle(bundleId: number) {
    for (const i of this.items) i.bundles = i.bundles.filter((b) => b.id !== bundleId);
  }

  // --- Dues & Ledger ---
  async getPartyDuesSummary(partyType: PartyType): Promise<PartyDueSummary[]> {
    return this.parties
      .filter((p) => p.partyType === partyType && p.balanceDue > 0)
      .map((p) => ({
        partyId: p.id,
        partyName: p.name,
        phone: p.phone,
        partyType: p.partyType,
        unsettledBillsCount: p.unpaidBillsCount,
        earliestBillDate: p.oldestBillDate ?? localDate(),
        totalInvoiced: p.balanceDue,
        totalPaid: 0,
        balanceDue: p.balanceDue,
      }));
  }
  async settleBillPayment(payload: Parameters<ApiClient['settleBillPayment']>[0]) {
    const p = this.findParty(payload.partyId);
    p.balanceDue = Math.max(0, p.balanceDue - payload.amountPaid);
    return {
      settlementIds: [this.id()],
      billsTouched: payload.transactionId ? [payload.transactionId] : [],
    };
  }
  async getPartyLedger(partyId: number) {
    const p = this.findParty(partyId);
    return {
      party: toParty(p),
      currentBalance: p.balanceDue,
      bills: [],
      payments: [],
      returns: [],
      entries: [],
    };
  }

  // --- Parties ---
  async createParty(payload: Parameters<ApiClient['createParty']>[0]) {
    const id = this.id() + 1000;
    this.parties.push({
      id,
      ...payload,
      isActive: true,
      createdAt: nowIso(),
      updatedAt: nowIso(),
      balanceDue: 0,
      oldestBillDate: null,
      unpaidBillsCount: 0,
    });
    return id;
  }
  async updateParty(partyId: number, payload: Parameters<ApiClient['updateParty']>[1]) {
    Object.assign(this.findParty(partyId), payload, { updatedAt: nowIso() });
  }
  async getParty(partyId: number) {
    return toParty(this.findParty(partyId));
  }
  async listParties(partyType?: PartyType, activeOnly = true) {
    return this.parties
      .filter((p) => (!partyType || p.partyType === partyType) && (!activeOnly || p.isActive))
      .sort((a, b) => a.name.localeCompare(b.name))
      .map(toListRow);
  }
  async searchParties(query: string, partyType?: PartyType) {
    const q = query.trim().toLowerCase();
    return (await this.listParties(partyType)).filter(
      (p) => p.name.toLowerCase().includes(q) || (p.phone ?? '').includes(q)
    );
  }

  // --- Returns & Adjustments ---
  async getNextReturnCode() {
    return `RET-${1001 + this.returns.length}`;
  }
  private addReturn(
    type: 'CUSTOMER' | 'VENDOR',
    p: {
      returnDate: string;
      partyId: number | null;
      settlementType: ReturnTransaction['settlementType'];
      reason?: string | null;
      notes: string | null;
      originalId: number | null;
      total: number;
    }
  ) {
    const id = this.id();
    const code = `RET-${1001 + this.returns.length}`;
    this.returns.unshift({
      id,
      returnCode: code,
      returnDate: p.returnDate,
      returnType: type,
      partyId: p.partyId,
      partyName: this.partyName(p.partyId),
      originalTransactionId: p.originalId,
      settlementType: p.settlementType,
      totalAmount: p.total,
      reason: p.reason ?? null,
      notes: p.notes,
      createdAt: nowIso(),
    });
    return { returnId: id, returnCode: code, creditedAmount: 0, cashRefunded: p.total };
  }
  async createCustomerReturn(payload: Parameters<ApiClient['createCustomerReturn']>[0]) {
    const total = payload.items.reduce((s, l) => s + l.quantity * l.unitPrice, 0);
    return this.addReturn('CUSTOMER', { ...payload, originalId: payload.originalSaleId, total });
  }
  async createVendorReturn(payload: Parameters<ApiClient['createVendorReturn']>[0]) {
    return this.addReturn('VENDOR', { ...payload, originalId: payload.originalPurchaseId, total: 0 });
  }
  async getReturnDetails(returnId: number): Promise<ReturnDetailView> {
    const r = this.returns.find((x) => x.id === returnId);
    if (!r) throw notFound('ReturnTransaction', returnId);
    return {
      returnTx: r,
      party: r.partyId ? toParty(this.findParty(r.partyId)) : null,
      items: [],
      allocations: [],
      ledger: [],
    };
  }
  async listReturns(filter: Parameters<ApiClient['listReturns']>[0] = {}) {
    const rows = filter?.returnType
      ? this.returns.filter((r) => r.returnType === filter.returnType)
      : this.returns;
    return paginate(rows, filter?.page, filter?.limit);
  }
  async createStockAdjustment(payload: Parameters<ApiClient['createStockAdjustment']>[0]) {
    const id = this.id();
    this.adjustments.unshift({
      id,
      adjustmentDate: payload.adjustmentDate,
      itemId: payload.itemId,
      quantity: payload.quantity,
      unitCost: payload.unitCost,
      reason: payload.reason,
      notes: payload.notes,
      createdAt: nowIso(),
    });
    return { adjustmentId: id, lossValue: 0 };
  }
  async getItemLots(itemId: number) {
    return [
      {
        purchaseItemId: 1,
        purchaseTransactionId: 1,
        transactionCode: 'PUR-1000',
        source: 'PURCHASE' as const,
        transactionDate: '2026-11-01',
        vendorName: 'Sample Vendor',
        quantity: 100,
        remainingQuantity: this.items.find((i) => i.id === itemId)?.currentStock ?? 0,
        unitCost: 5,
      },
    ];
  }
  async listStockAdjustments(itemId?: number, page?: number, limit?: number) {
    const rows = itemId ? this.adjustments.filter((a) => a.itemId === itemId) : this.adjustments;
    return paginate(rows, page, limit);
  }

  // --- Reports (static fixtures) ---
  async getDashboardMetrics() {
    return {
      todaySalesAmount: this.sales.reduce((s, r) => s + r.totalAmount, 0) + 4500,
      todaySalesCount: this.sales.length + 12,
      todayPurchaseAmount: 1500,
      todayPurchaseCount: 2,
      todayNetProfit: 1200,
      totalStockValuation: 185000,
      customerReceivables: this.parties
        .filter((p) => p.partyType === 'CUSTOMER')
        .reduce((s, p) => s + p.balanceDue, 0),
      vendorPayables: this.parties
        .filter((p) => p.partyType === 'VENDOR')
        .reduce((s, p) => s + p.balanceDue, 0),
      liveCashInDrawer: 3800,
    };
  }
  async getLowStockAlerts() {
    return this.items
      .filter((i) => i.isLowStock)
      .sort((a, b) => a.currentStock - b.currentStock || a.name.localeCompare(b.name))
      .map((i) => ({
        id: i.id,
        name: i.name,
        category: i.category,
        baseUnit: i.baseUnit,
        currentStock: i.currentStock,
        defaultSalePrice: i.defaultSalePrice,
        mrp: i.mrp,
      }));
  }
  async getSalesPurchaseTrend(period: string) {
    const n = period === '30d' ? 30 : period === '1y' ? 12 : 7;
    const out = [];
    for (let k = n - 1; k >= 0; k--) {
      const d = new Date();
      if (period === '1y') {
        d.setDate(1);
        d.setMonth(d.getMonth() - k);
      } else d.setDate(d.getDate() - k);
      out.push({
        date: period === '1y' ? `${localDate(d).slice(0, 8)}01` : localDate(d),
        salesAmount: 3000 + ((k * 731) % 2000),
        purchaseAmount: 1000 + ((k * 377) % 1500),
      });
    }
    return out;
  }
  async getRecentTransactions(limit: number) {
    return this.sales.slice(0, limit).map((s) => ({
      kind: 'SALE' as const,
      id: s.id,
      code: s.transactionCode,
      date: s.transactionDate,
      createdAt: s.createdAt,
      partyName: s.partyName,
      amount: s.totalAmount,
      paymentStatus: s.paymentStatus,
    }));
  }
  async getPnLReport() {
    return {
      grossSales: 120000,
      salesReturns: 2000,
      netSales: 118000,
      cogs: 85000,
      grossProfit: 33000,
      inventoryLosses: 500,
      netProfit: 32500,
      totalPurchases: 90000,
    };
  }
  async getStockValuationReport() {
    return {
      totalValuation: 185000,
      activeLotsCount: this.items.length,
      categoriesBreakdown: (await this.listCategories()).map((category) => ({
        category,
        itemsCount: this.items.filter((i) => i.category === category).length,
        valuation: 10000,
      })),
    };
  }
  async getCashFlowReport(): Promise<CashFlowReport> {
    return {
      salesCashInflow: 4000,
      customerSettlementsInflow: 500,
      vendorRefundsInflow: 0,
      purchaseCashOutflow: 1500,
      vendorSettlementsOutflow: 200,
      refundsCashOutflow: 100,
      netCashBalance: 2700,
    };
  }

  // --- Settings & System ---
  async getShopSettings() {
    return { ...this.settings };
  }
  async updateShopSettings(settings: ShopSettings) {
    this.settings = { ...settings };
  }
  async backupDatabase(destinationPath: string) {
    return `${destinationPath}/book_keeper_backup.db`;
  }
  private backupDir: string | null = null;
  private mockConfigView(): AppConfigView {
    return {
      testingMode: this.testingMode,
      backupDirectory: this.backupDir ?? MOCK_DEFAULT_BACKUP_DIR,
      isDefaultDirectory: this.backupDir === null,
      lastAutoBackupAt: null,
    };
  }
  async getAppConfig() {
    return this.mockConfigView();
  }
  async setBackupDirectory(path: string | null) {
    this.backupDir = path && path.trim() ? path.trim() : null;
    return this.mockConfigView();
  }
  private testingMode = true;
  async resetData(scope: ResetScope): Promise<ResetSummary> {
    if (!this.testingMode) {
      throw { type: 'OperationNotAllowed', details: 'Data reset is only available in testing mode' };
    }
    const deleted = [
      { table: 'sales', rows: this.sales.length },
      { table: 'purchases', rows: this.purchases.length },
      { table: 'returns', rows: this.returns.length },
      { table: 'stock_adjustments', rows: this.adjustments.length },
    ];
    this.sales = [];
    this.purchases = [];
    this.returns = [];
    this.adjustments = [];
    if (scope === 'All') {
      deleted.push({ table: 'items', rows: this.items.length }, { table: 'parties', rows: this.parties.length });
      this.items = [];
      this.parties = [];
    }
    return { scope, deleted, backupPath: `${this.backupDir ?? MOCK_DEFAULT_BACKUP_DIR}/book_keeper_prereset_mock.db` };
  }
  async verifyDatabaseIntegrity() {
    return true;
  }

  // --- Import ---
  async validateItemsImport() {
    return {
      validCount: 10,
      rowsWithErrors: [],
      duplicates: [],
      headerIssues: [],
      warnings: [],
      lotsToCreate: 10,
      expectedColumns: [],
    };
  }
  async executeItemsImport() {
    return { created: 10, updated: 0, skipped: 0, stockLotsCreated: 10 };
  }
  async validateVedbillImport() {
    return {
      validCount: 3,
      rowsWithErrors: [],
      duplicates: [],
      headerIssues: [],
      warnings: [{ row: 1, name: 'WHITE GLUE 20', message: 'VedBill shows stock 29 but its lots add up to 32.' }],
      lotsToCreate: 4,
      expectedColumns: [],
    };
  }
  async executeVedbillImport() {
    return { created: 3, updated: 0, skipped: 0, stockLotsCreated: 4 };
  }
  async writeItemsImportTemplate(destPath: string) {
    return destPath;
  }
}

export const mockApiClient = new MockApiClient();
