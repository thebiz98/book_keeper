import type {
  ApiClient,
  BackupFileInfo,
  BackupInspection,
  MigrationReport,
} from '@/types/api';
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

export class MockApiClient implements ApiClient {
  private items: MockItem[];
  private parties: MockParty[];
  private sales: any[];
  private purchases: any[];
  private settings: typeof initialShopSettings;
  private bundleSuggestions: any[];
  private saleCounter: number = 1001;
  private purchaseCounter: number = 1001;
  private cashDrawer: {
    salesCash: number;
    customerDuesSettled: number;
    purchaseCash: number;
    vendorDuesSettled: number;
    refunds: number;
  };

  constructor() {
    this.items = structuredClone(initialItems);
    this.parties = [
      ...structuredClone(initialCustomers),
      ...structuredClone(initialVendors),
    ];
    this.settings = structuredClone(initialShopSettings);
    this.bundleSuggestions = structuredClone(initialBundleSuggestions);
    this.sales = [];
    this.purchases = [];
    this.cashDrawer = {
      salesCash: 4000.0,
      customerDuesSettled: 500.0,
      purchaseCash: 1500.0,
      vendorDuesSettled: 200.0,
      refunds: 100.0,
    };
  }

  // 1. searchItemsFts
  async searchItemsFts(query: string, limit: number = 50): Promise<MockItem[]> {
    const q = (query || '').trim().toLowerCase();
    if (!q) {
      return this.items
        .filter((i) => i.is_active !== false && i.isActive !== false)
        .slice(0, limit);
    }

    // Barcode exact match
    const exactBarcode = this.items.find(
      (i) =>
        i.is_active !== false &&
        i.isActive !== false &&
        i.barcode === query.trim()
    );
    if (exactBarcode) {
      return [exactBarcode];
    }

    return this.items
      .filter((i) => {
        if (i.is_active === false || i.isActive === false) return false;
        const text = `${i.name} ${i.category} ${i.barcode || ''}`.toLowerCase();
        return text.includes(q);
      })
      .slice(0, limit);
  }

  // 2. getItemsCatalog
  async getItemsCatalog(
    filterOrCategory?: any,
    lowStockOnlyArg?: boolean
  ): Promise<{
    items: MockItem[];
    total: number;
    page: number;
    limit: number;
    totalPages: number;
  }> {
    let category: string | undefined;
    let lowStockOnly: boolean | undefined = lowStockOnlyArg;
    let limit = 500;
    let page = 1;

    if (typeof filterOrCategory === 'object' && filterOrCategory !== null) {
      category = filterOrCategory.category;
      lowStockOnly =
        filterOrCategory.lowStockOnly ??
        filterOrCategory.low_stock_only ??
        lowStockOnlyArg;
      limit = filterOrCategory.limit ?? 500;
      page = filterOrCategory.page ?? 1;
    } else if (typeof filterOrCategory === 'string') {
      category = filterOrCategory;
    }

    let filtered = this.items.filter(
      (i) => i.is_active !== false && i.isActive !== false
    );
    if (category) {
      filtered = filtered.filter(
        (i) => i.category.toLowerCase() === category!.toLowerCase()
      );
    }
    if (lowStockOnly) {
      filtered = filtered.filter(
        (i) => (i.currentStock ?? i.current_stock) < 5
      );
    }

    const total = filtered.length;
    const startIndex = (page - 1) * limit;
    const paginatedItems = filtered.slice(startIndex, startIndex + limit);

    return {
      items: paginatedItems,
      total,
      page,
      limit,
      totalPages: Math.ceil(total / limit) || 1,
    };
  }

  // 3. createItem, updateItem, archiveItem
  async createItem(payload: any): Promise<MockItem> {
    const nextId = Math.max(0, ...this.items.map((i) => i.id)) + 1;
    const stock = Number(
      payload.initial_stock ??
        payload.initialStock ??
        payload.current_stock ??
        0
    );
    const cost = Number(
      payload.initial_cost ?? payload.cost_price ?? payload.costPrice ?? 0
    );
    const sale = Number(
      payload.default_sale_price ?? payload.defaultSalePrice ?? 0
    );
    const mrp = Number(payload.mrp ?? sale);

    const newItem: MockItem = {
      id: nextId,
      itemId: nextId,
      name: payload.name,
      category: payload.category || 'General Stationery',
      barcode: payload.barcode || `890103038${nextId}`,
      base_unit: payload.base_unit || payload.baseUnit || 'PCS',
      baseUnit: payload.base_unit || payload.baseUnit || 'PCS',
      cost_price: cost,
      costPrice: cost,
      default_sale_price: sale,
      defaultSalePrice: sale,
      mrp,
      current_stock: stock,
      currentStock: stock,
      is_low_stock: stock < 5,
      isLowStock: stock < 5,
      is_active: true,
      isActive: true,
      bundles: payload.bundles || [],
    };
    this.items.push(newItem);
    return newItem;
  }

  async updateItem(itemId: number, payload: any): Promise<MockItem> {
    const idx = this.items.findIndex(
      (i) => i.id === itemId || i.itemId === itemId
    );
    if (idx === -1) {
      throw new Error(`Item ${itemId} not found`);
    }
    const curr = this.items[idx];
    const updated = {
      ...curr,
      ...payload,
      id: curr.id,
      itemId: curr.itemId,
      current_stock:
        payload.current_stock ?? payload.currentStock ?? curr.current_stock,
      currentStock:
        payload.current_stock ?? payload.currentStock ?? curr.currentStock,
    };
    updated.is_low_stock = updated.current_stock < 5;
    updated.isLowStock = updated.is_low_stock;
    this.items[idx] = updated;
    return updated;
  }

  async archiveItem(itemId: number): Promise<boolean> {
    const item = this.items.find(
      (i) => i.id === itemId || i.itemId === itemId
    );
    if (!item) return false;
    item.is_active = false;
    item.isActive = false;
    (item as any).isArchived = true;
    return true;
  }

  // 4. Sales Workflow
  async getNextSaleCode(): Promise<string> {
    return `SAL-${this.saleCounter++}`;
  }

  async createSale(payload: any): Promise<any> {
    const saleCode =
      payload.saleCode ||
      payload.transaction_code ||
      (await this.getNextSaleCode());
    let total = 0;

    for (const line of payload.items || []) {
      const targetId = line.itemId ?? line.item_id;
      const item = this.items.find(
        (i) => i.id === targetId || i.itemId === targetId
      );
      const qty = Number(line.quantity ?? line.bundle_quantity ?? 1);
      const mult = Number(line.unitsPerBundle ?? line.units_per_bundle ?? 1);
      const rate = Number(
        line.rate ?? line.unit_price ?? item?.default_sale_price ?? 0
      );
      total += qty * rate;

      if (item) {
        const deducted = qty * mult;
        item.current_stock = Math.max(0, item.current_stock - deducted);
        item.currentStock = item.current_stock;
        item.is_low_stock = item.current_stock < 5;
        item.isLowStock = item.is_low_stock;
      }
    }

    const paid = Number(payload.paidAmount ?? payload.paid_amount ?? total);
    const balance = Math.max(0, total - paid);
    const change = Math.max(0, paid - total);

    const customerId = payload.customerId ?? payload.party_id;
    if (customerId) {
      const cust = this.parties.find((p) => p.id === customerId);
      if (cust && balance > 0) {
        cust.balance_due += balance;
        cust.balanceDue = cust.balance_due;
        cust.netDueBalance = cust.balance_due;
        cust.total_due = cust.balance_due;
      }
    }

    this.cashDrawer.salesCash += Math.min(paid, total);

    const saleRecord = {
      saleId: this.sales.length + 1,
      sale_id: this.sales.length + 1,
      saleCode,
      sale_code: saleCode,
      transactionCode: saleCode,
      transaction_code: saleCode,
      totalAmount: total,
      total_amount: total,
      paidAmount: paid,
      paid_amount: paid,
      balanceDue: balance,
      balance_due: balance,
      changeAmount: change,
      change_amount: change,
      changeReturned: change,
      change_returned: change,
      paymentStatus:
        payload.paymentStatus ||
        (balance === 0 ? 'PAID' : paid > 0 ? 'PARTIAL' : 'UNPAID'),
      payment_status:
        payload.paymentStatus ||
        (balance === 0 ? 'PAID' : paid > 0 ? 'PARTIAL' : 'UNPAID'),
      date: payload.date || payload.transaction_date || '2026-11-20',
      items: payload.items,
    };
    this.sales.push(saleRecord);
    return saleRecord;
  }

  async getSalesHistory(_filter?: any): Promise<any> {
    return {
      items: this.sales,
      total: this.sales.length,
      page: 1,
      limit: 50,
      totalPages: 1,
    };
  }

  async getSaleDetails(saleId: number): Promise<any> {
    const sale = this.sales.find(
      (s) => s.saleId === saleId || s.sale_id === saleId
    );
    return {
      sale,
      party: this.parties.find((p) => p.id === sale?.customerId),
      items: sale?.items || [],
      settlements: [],
    };
  }

  // 5. Purchase Workflow
  async getNextPurchaseCode(): Promise<string> {
    return `PUR-${this.purchaseCounter++}`;
  }

  async createPurchase(payload: any): Promise<any> {
    const purchaseCode =
      payload.purchaseCode ||
      payload.transaction_code ||
      (await this.getNextPurchaseCode());
    let total = 0;

    for (const line of payload.items || []) {
      const targetId = line.itemId ?? line.item_id;
      const item = this.items.find(
        (i) => i.id === targetId || i.itemId === targetId
      );
      const qty = Number(line.quantity ?? line.bundle_quantity ?? 1);
      const mult = Number(line.unitsPerBundle ?? line.units_per_bundle ?? 1);
      const cost = Number(
        line.unitCost ?? line.unit_cost ?? item?.cost_price ?? 0
      );
      total += qty * cost;

      if (item) {
        const added = qty * mult;
        item.current_stock += added;
        item.currentStock = item.current_stock;
        item.is_low_stock = item.current_stock < 5;
        item.isLowStock = item.is_low_stock;
      }
    }

    const paid = Number(payload.paidAmount ?? payload.paid_amount ?? total);
    const balance = Math.max(0, total - paid);

    const vendorId = payload.vendorId ?? payload.party_id;
    if (vendorId) {
      const vendor = this.parties.find((p) => p.id === vendorId);
      if (vendor && balance > 0) {
        vendor.balance_due += balance;
        vendor.balanceDue = vendor.balance_due;
        vendor.netDueBalance = vendor.balance_due;
        vendor.total_due = vendor.balance_due;
      }
    }

    this.cashDrawer.purchaseCash += Math.min(paid, total);

    const record = {
      purchaseId: this.purchases.length + 1,
      purchase_id: this.purchases.length + 1,
      purchaseCode,
      purchase_code: purchaseCode,
      transaction_code: purchaseCode,
      totalAmount: total,
      paidAmount: paid,
      balanceDue: balance,
    };
    this.purchases.push(record);
    return record;
  }

  async getPurchasesHistory(_filter?: any): Promise<any> {
    return {
      items: this.purchases,
      total: this.purchases.length,
      page: 1,
      limit: 50,
      totalPages: 1,
    };
  }

  async getPurchaseDetails(purchaseId: number): Promise<any> {
    return {
      purchase: this.purchases.find(
        (p) => p.purchaseId === purchaseId || p.purchase_id === purchaseId
      ),
      items: [],
      settlements: [],
    };
  }

  // 6. Parties & Accounting
  async listParties(
    partyType?: 'CUSTOMER' | 'VENDOR' | 'Customer' | 'Vendor',
    activeOnly: boolean = true
  ): Promise<any[]> {
    const normalizedType = partyType ? partyType.toUpperCase() : null;
    return this.parties.filter((p) => {
      if (activeOnly && p.is_active === false) return false;
      if (normalizedType && p.party_type !== normalizedType) return false;
      return true;
    });
  }

  async getParty(partyId: number): Promise<any> {
    return this.parties.find(
      (p) => p.id === partyId || p.partyId === partyId
    );
  }

  async getPartyDuesSummary(
    partyType: 'CUSTOMER' | 'VENDOR' | 'Customer' | 'Vendor'
  ): Promise<any[]> {
    const normalizedType = partyType.toUpperCase();
    return this.parties
      .filter(
        (p) =>
          p.party_type === normalizedType &&
          (p.balance_due > 0 || p.netDueBalance > 0)
      )
      .map((p) => ({
        partyId: p.id,
        party_id: p.id,
        id: p.id,
        partyName: p.name,
        party_name: p.name,
        name: p.name,
        phone: p.phone,
        partyType: p.party_type,
        party_type: p.party_type,
        totalDue: p.balance_due,
        total_due: p.balance_due,
        balanceDue: p.balance_due,
        balance_due: p.balance_due,
        netDueBalance: p.balance_due,
        oldestBillDate: p.oldest_bill_date,
        oldest_bill_date: p.oldest_bill_date,
        unpaidBillsCount: p.unpaid_bills_count,
        unpaid_bills_count: p.unpaid_bills_count,
      }));
  }

  async settleBillPayment(payload: any): Promise<boolean> {
    const partyId = payload.partyId ?? payload.party_id ?? payload.id;
    const amount = Number(payload.amount ?? 0);
    const party = this.parties.find(
      (p) => p.id === partyId || p.partyId === partyId
    );
    if (!party) return false;

    party.balance_due = Math.max(0, party.balance_due - amount);
    party.balanceDue = party.balance_due;
    party.netDueBalance = party.balance_due;
    party.total_due = party.balance_due;

    if (party.party_type === 'CUSTOMER') {
      this.cashDrawer.customerDuesSettled += amount;
    } else {
      this.cashDrawer.vendorDuesSettled += amount;
    }
    return true;
  }

  async getPartyLedger(partyId: number): Promise<any> {
    return { partyId, entries: [] };
  }

  async createParty(payload: any): Promise<any> {
    const nextId = Math.max(0, ...this.parties.map((p) => p.id)) + 1;
    const rawType = (payload.party_type || payload.partyType || 'CUSTOMER').toUpperCase();
    const partyType = rawType === 'VENDOR' ? 'VENDOR' : 'CUSTOMER';
    const newParty: MockParty = {
      id: nextId,
      partyId: nextId,
      name: payload.name,
      partyName: payload.name,
      phone: payload.phone,
      address: payload.address,
      partyType,
      party_type: partyType,
      balanceDue: 0,
      balance_due: 0,
      netDueBalance: 0,
      totalDue: 0,
      total_due: 0,
      oldestBillDate: null,
      oldest_bill_date: null,
      unpaidBillsCount: 0,
      unpaid_bills_count: 0,
      isActive: true,
      is_active: true,
    };
    this.parties.push(newParty);
    return { party_id: nextId, partyId: nextId };
  }

  async updateParty(partyId: number, payload: any): Promise<boolean> {
    const party = this.parties.find(
      (p) => p.id === partyId || p.partyId === partyId
    );
    if (!party) return false;
    Object.assign(party, payload);
    return true;
  }

  async archiveParty(partyId: number): Promise<boolean> {
    const party = this.parties.find(
      (p) => p.id === partyId || p.partyId === partyId
    );
    if (!party) return false;
    party.isActive = false;
    party.is_active = false;
    return true;
  }

  async searchParties(
    query: string,
    partyType?: 'CUSTOMER' | 'VENDOR' | 'Customer' | 'Vendor'
  ): Promise<any[]> {
    const q = (query || '').toLowerCase();
    const normalizedType = partyType ? partyType.toUpperCase() : null;
    return this.parties.filter((p) => {
      if (normalizedType && p.party_type !== normalizedType) return false;
      return (
        p.name.toLowerCase().includes(q) ||
        (p.phone && p.phone.includes(q))
      );
    });
  }

  // 7. Reports & Dashboard
  async getLowStockAlerts(): Promise<any[]> {
    return this.items
      .filter(
        (i) =>
          i.is_active !== false &&
          i.isActive !== false &&
          (i.currentStock ?? i.current_stock) < 5
      )
      .map((i) => ({
        id: i.id,
        itemId: i.id,
        item_id: i.id,
        name: i.name,
        category: i.category,
        base_unit: i.base_unit,
        baseUnit: i.base_unit,
        current_stock: i.current_stock,
        currentStock: i.current_stock,
        default_sale_price: i.default_sale_price,
        defaultSalePrice: i.default_sale_price,
        mrp: i.mrp,
      }));
  }

  async getDashboardMetrics(): Promise<any> {
    const totalValuation = this.items.reduce(
      (sum, item) => sum + item.current_stock * item.cost_price,
      0
    );
    const customerReceivables = this.parties
      .filter((p) => p.party_type === 'CUSTOMER')
      .reduce((sum, p) => sum + (p.balance_due || 0), 0);
    const vendorPayables = this.parties
      .filter((p) => p.party_type === 'VENDOR')
      .reduce((sum, p) => sum + (p.balance_due || 0), 0);
    const liveCash =
      this.cashDrawer.salesCash +
      this.cashDrawer.customerDuesSettled -
      (this.cashDrawer.purchaseCash +
        this.cashDrawer.vendorDuesSettled +
        this.cashDrawer.refunds);

    return {
      todayGrossSales: 4200.0,
      todaySales: 4200.0,
      today_sales: 4200.0,
      todayPurchases: 1500.0,
      today_purchases: 1500.0,
      todayNetProfit: 1450.0,
      today_net_profit: 1450.0,
      totalStockValuation: totalValuation,
      stockValuation: totalValuation,
      total_stock_valuation: totalValuation,
      customerReceivables,
      customer_receivables: customerReceivables,
      vendorPayables,
      vendor_payables: vendorPayables,
      liveCashInDrawer: liveCash,
      cashDrawer: liveCash,
      live_cash_in_drawer: liveCash,
    };
  }

  async getPnLReport(fromDate: string, toDate: string): Promise<any> {
    const grossSales = 45000.0;
    const cogs = 28500.0;
    const netProfit = grossSales - cogs;
    return {
      fromDate,
      from_date: fromDate,
      toDate,
      to_date: toDate,
      grossSales,
      gross_sales: grossSales,
      cogs,
      cost_of_goods_sold: cogs,
      netProfit,
      net_profit: netProfit,
      totalDiscounts: 850.0,
      total_discounts: 850.0,
      returnsRefunded: 350.0,
      returns_refunded: 350.0,
    };
  }

  async getStockValuationReport(): Promise<any> {
    const totalValuation = this.items.reduce(
      (sum, item) => sum + item.current_stock * item.cost_price,
      0
    );
    return {
      totalValuation,
      total_valuation: totalValuation,
      totalItemsCount: this.items.length,
      total_items_count: this.items.length,
      activeLotsCount: this.items.length,
      active_lots_count: this.items.length,
      categoriesBreakdown: [],
      categories: [],
    };
  }

  async getCashFlowReport(fromDate: string, toDate: string): Promise<any> {
    const inflow =
      this.cashDrawer.salesCash + this.cashDrawer.customerDuesSettled;
    const outflow =
      this.cashDrawer.purchaseCash +
      this.cashDrawer.vendorDuesSettled +
      this.cashDrawer.refunds;
    return {
      fromDate,
      from_date: fromDate,
      toDate,
      to_date: toDate,
      totalInflow: inflow,
      total_inflow: inflow,
      totalOutflow: outflow,
      total_outflow: outflow,
      netCashFlow: inflow - outflow,
      net_cash_flow: inflow - outflow,
      closingDrawerBalance: inflow - outflow,
      closing_drawer_balance: inflow - outflow,
    };
  }

  async getSalesPurchaseTrend(
    _period: '7d' | '30d' | '1y' | string
  ): Promise<any[]> {
    return [
      {
        date: '2026-11-14',
        sales_amount: 3200,
        purchase_amount: 1100,
        salesAmount: 3200,
        purchaseAmount: 1100,
      },
      {
        date: '2026-11-15',
        sales_amount: 4100,
        purchase_amount: 800,
        salesAmount: 4100,
        purchaseAmount: 800,
      },
      {
        date: '2026-11-16',
        sales_amount: 2900,
        purchase_amount: 1500,
        salesAmount: 2900,
        purchaseAmount: 1500,
      },
      {
        date: '2026-11-17',
        sales_amount: 5200,
        purchase_amount: 2200,
        salesAmount: 5200,
        purchaseAmount: 2200,
      },
      {
        date: '2026-11-18',
        sales_amount: 3800,
        purchase_amount: 950,
        salesAmount: 3800,
        purchaseAmount: 950,
      },
      {
        date: '2026-11-19',
        sales_amount: 4600,
        purchase_amount: 1400,
        salesAmount: 4600,
        purchaseAmount: 1400,
      },
      {
        date: '2026-11-20',
        sales_amount: 4200,
        purchase_amount: 1500,
        salesAmount: 4200,
        purchaseAmount: 1500,
      },
    ];
  }

  // 8. Bundles, Returns & Adjustments
  async getBundleSuggestions(): Promise<any[]> {
    return this.bundleSuggestions;
  }

  async createBundle(payload: any): Promise<any> {
    const item = this.items.find(
      (i) => i.id === payload.item_id || i.itemId === payload.itemId
    );
    const bundle = {
      id: Math.floor(Math.random() * 100000),
      bundleId: Math.floor(Math.random() * 100000),
      bundleName: payload.bundle_name || payload.bundleName,
      name: payload.bundle_name || payload.bundleName,
      unitsPerBundle:
        payload.units_per_bundle || payload.unitsPerBundle || payload.multiplier || 1,
      units_per_bundle:
        payload.units_per_bundle || payload.unitsPerBundle || payload.multiplier || 1,
      multiplier:
        payload.units_per_bundle || payload.unitsPerBundle || payload.multiplier || 1,
      bundlePrice: payload.bundle_price || payload.bundlePrice,
      mrp: payload.mrp,
      barcode: payload.barcode,
    };
    if (item) {
      item.bundles.push(bundle as any);
    }
    return bundle;
  }

  async getNextReturnCode(): Promise<string> {
    return `RET-${Math.floor(1000 + Math.random() * 9000)}`;
  }

  async createCustomerReturn(payload: any): Promise<any> {
    for (const line of payload.items || []) {
      const item = this.items.find(
        (i) => i.id === (line.itemId ?? line.item_id)
      );
      if (item) {
        item.current_stock += Number(line.quantity ?? 1);
        item.currentStock = item.current_stock;
      }
    }
    if (payload.refundAmount) {
      this.cashDrawer.refunds += Number(payload.refundAmount);
    }
    return {
      return_code: `RET-SAL-${Math.floor(1000 + Math.random() * 9000)}`,
    };
  }

  async createVendorReturn(payload: any): Promise<any> {
    for (const line of payload.items || []) {
      const item = this.items.find(
        (i) => i.id === (line.itemId ?? line.item_id)
      );
      if (item) {
        item.current_stock = Math.max(
          0,
          item.current_stock - Number(line.quantity ?? 1)
        );
        item.currentStock = item.current_stock;
      }
    }
    return {
      return_code: `RET-PUR-${Math.floor(1000 + Math.random() * 9000)}`,
    };
  }

  async getReturnDetails(returnId: number): Promise<any> {
    return { returnId, items: [] };
  }

  async createStockAdjustment(payload: any): Promise<any> {
    const item = this.items.find(
      (i) => i.id === (payload.itemId ?? payload.item_id)
    );
    if (item) {
      item.current_stock = Math.max(
        0,
        item.current_stock +
          Number(payload.quantityDelta ?? payload.quantity_delta ?? 0)
      );
      item.currentStock = item.current_stock;
    }
    return { adjustment_id: Math.floor(1000 + Math.random() * 9000) };
  }

  async listStockAdjustments(
    _itemId?: number,
    _page?: number,
    _limit?: number
  ): Promise<any> {
    return { items: [], total: 0, page: 1, limit: 50, totalPages: 1 };
  }

  // 9. Shop Settings & Backup
  async getShopSettings(): Promise<any> {
    return this.settings;
  }

  async updateShopSettings(settings: any): Promise<boolean> {
    this.settings = {
      ...this.settings,
      ...settings,
      shopName:
        settings.shopName || settings.shop_name || this.settings.shopName,
      shop_name:
        settings.shopName || settings.shop_name || this.settings.shop_name,
    };
    return true;
  }

  async backupDatabase(destinationPath: string): Promise<string> {
    return destinationPath || '/backup/shop_backup.db';
  }

  async verifyDatabaseIntegrity(): Promise<boolean> {
    return true;
  }

  // 10. Migration & Excel
  async scanBackupDirectory(directoryPath: string): Promise<BackupFileInfo[]> {
    const dir = (directoryPath || '/backup').replace(/\/$/, '');
    return [
      {
        path: `${dir}/vedbill_backup.db`,
        fileName: 'vedbill_backup.db',
        sizeBytes: 245760,
        modifiedAt: new Date().toISOString(),
      },
    ];
  }

  async inspectBackupFile(filePath: string): Promise<BackupInspection> {
    return {
      path: filePath,
      itemsCount: this.items.length,
      partiesCount: this.parties.length,
      salesCount: this.sales.length,
      purchasesCount: this.purchases.length,
      warnings: [],
    };
  }

  async executeMigration(_filePath: string): Promise<MigrationReport> {
    return {
      success: true,
      itemsImported: this.items.length,
      partiesImported: this.parties.length,
      salesImported: 0,
      purchasesImported: 0,
      errors: [],
      warnings: [],
    };
  }

  async exportItemsToExcel(destinationPath: string): Promise<string> {
    return destinationPath || '/export/items.xlsx';
  }

  async importItemsFromExcel(_filePath: string): Promise<MigrationReport> {
    return { success: true, itemsImported: 0, errors: [], warnings: [] };
  }

  async exportTransactionsToExcel(
    destinationPath: string,
    _filter?: any
  ): Promise<string> {
    return destinationPath || '/export/transactions.xlsx';
  }
}

export const mockApiClient = new MockApiClient();
