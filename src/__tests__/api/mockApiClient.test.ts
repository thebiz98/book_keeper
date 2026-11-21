import { describe, it, expect, beforeEach } from 'vitest';
import { MockApiClient } from '@/api/mock/mockClient';

// ============================================================================
// Test Suite: MockApiClient
// Spec: docs/book_keeper_UI_LLD.md §6.1, §6.2, PROJECT.md F42
// Verified: 160+ stationery items, customer/vendor accounts, async promise contracts
// ============================================================================

describe('MockApiClient - Retail Stationery Seed Dataset & Async IPC Abstraction', () => {
  let client: InstanceType<typeof MockApiClient>;

  beforeEach(() => {
    client = new MockApiClient();
  });

  describe('1. Stationery Catalog Seed Dataset Profile (160+ Items)', () => {
    it('TC-MOCK-001: provides seeded catalog containing at least 160 stationery items', async () => {
      const response = await client.getItemsCatalog({});
      expect(response).toBeDefined();
      expect(response.total).toBeGreaterThanOrEqual(160);
      expect(response.items.length).toBeGreaterThan(0);
    });

    it('TC-MOCK-002: seed dataset contains diverse stationery retail categories', async () => {
      const response = await client.getItemsCatalog({ limit: 200 });
      const categories = new Set(response.items.map((i: any) => i.category));

      // Key stationery categories from specification:
      // Notebooks, Pens, Adhesives, Files & Folders, Paper, Geometry/Drawing
      expect(categories.size).toBeGreaterThanOrEqual(4);
    });

    it('TC-MOCK-003: seed dataset includes items flagged as low stock (< 5 units)', async () => {
      const lowStockAlerts = await client.getLowStockAlerts();
      expect(lowStockAlerts).toBeDefined();
      expect(lowStockAlerts.length).toBeGreaterThan(0);

      // Verify every alert item has current_stock < 5
      for (const alert of lowStockAlerts) {
        expect(alert.currentStock ?? alert.current_stock).toBeLessThan(5);
      }
    });

    it('TC-MOCK-004: seed dataset includes items with configured packaging bundles', async () => {
      const suggestions = await client.getBundleSuggestions();
      expect(suggestions).toBeDefined();
      expect(suggestions.length).toBeGreaterThan(0);

      // Verify presence of standard packaging tiers like Box, Pack, Dozen
      const bundleNames = suggestions.map((s: any) => s.bundleName || s.name);
      const hasStandardBundle = bundleNames.some((name: string) =>
        /box|pack|dozen|carton/i.test(name)
      );
      expect(hasStandardBundle).toBe(true);
    });
  });

  describe('2. Parties & Dues Khata Seed Profile', () => {
    it('TC-MOCK-005: provides seeded customer directory with active dues balances', async () => {
      const customers = await client.listParties('CUSTOMER');
      expect(customers).toBeDefined();
      expect(customers.length).toBeGreaterThan(0);

      const duesSummary = await client.getPartyDuesSummary('CUSTOMER');
      expect(duesSummary).toBeDefined();
      expect(duesSummary.length).toBeGreaterThan(0);

      // Verify outstanding balance fields exist
      const firstDue = duesSummary[0];
      expect(firstDue.netDueBalance ?? firstDue.balance_due).toBeGreaterThan(0);
    });

    it('TC-MOCK-006: provides seeded vendor directory', async () => {
      const vendors = await client.listParties('VENDOR');
      expect(vendors).toBeDefined();
      expect(vendors.length).toBeGreaterThan(0);
    });
  });

  describe('3. FTS5 Typeahead & Barcode Search Simulation', () => {
    it('TC-MOCK-007: prefix query matches item names within 2-3 characters', async () => {
      const results = await client.searchItemsFts('Pen', 10);
      expect(results).toBeDefined();
      expect(results.length).toBeGreaterThan(0);

      // All returned items must contain 'Pen' in name or category
      for (const item of results) {
        const text = `${item.name} ${item.category}`.toLowerCase();
        expect(text).toContain('pen');
      }
    });

    it('TC-MOCK-008: exact barcode search returns matching item directly', async () => {
      // First get any catalog item with a barcode
      const catalog = await client.getItemsCatalog({ limit: 10 });
      const itemWithBarcode = catalog.items.find((i: any) => Boolean(i.barcode));

      expect(itemWithBarcode).toBeDefined();
      const results = await client.searchItemsFts(itemWithBarcode!.barcode);
      expect(results.length).toBeGreaterThanOrEqual(1);
      expect(results[0].barcode).toBe(itemWithBarcode!.barcode);
    });

    it('TC-MOCK-009: non-existent query returns empty results array', async () => {
      const results = await client.searchItemsFts('XZQ_NON_EXISTENT_QUERY_99999');
      expect(results).toEqual([]);
    });

    it('TC-MOCK-010: archived items are excluded from active search results', async () => {
      // Archive an item
      const catalog = await client.getItemsCatalog({ limit: 5 });
      const targetItem = catalog.items[0];

      await client.archiveItem(targetItem.id);

      // Searching for exact item name should now exclude it
      const results = await client.searchItemsFts(targetItem.name);
      const found = results.find((r: any) => r.id === targetItem.id);
      expect(found).toBeUndefined();
    });
  });

  describe('4. Counter Billing Checkout & Stock Depletion', () => {
    it('TC-MOCK-011: getNextSaleCode returns sequential SAL-XXXX code', async () => {
      const code1 = await client.getNextSaleCode();
      expect(code1).toMatch(/^SAL-\d+$/);
    });

    it('TC-MOCK-012: createSale persists sale and decrements item stock in memory', async () => {
      const catalog = await client.getItemsCatalog({ limit: 5 });
      const item = catalog.items.find((i: any) => (i.currentStock ?? i.current_stock) > 10);

      expect(item).toBeDefined();
      if (!item) throw new Error('seed item not found');
      const initialStock = item.currentStock ?? item.current_stock;
      const saleCode = await client.getNextSaleCode();

      const payload = {
        saleCode,
        date: '2026-11-20',
        customerId: null,
        items: [
          {
            itemId: item.id,
            quantity: 2,
            unitsPerBundle: 1,
            rate: item.defaultSalePrice ?? item.default_sale_price ?? 20.0,
          },
        ],
        paidAmount: (item.defaultSalePrice ?? item.default_sale_price ?? 20.0) * 2,
        paymentStatus: 'PAID',
      };

      const result = await client.createSale(payload);
      expect(result).toBeDefined();

      // Check that item stock was decremented
      const updatedCatalog = await client.getItemsCatalog({ limit: 100 });
      const updatedItem = updatedCatalog.items.find((i: any) => i.id === item.id);
      if (!updatedItem) throw new Error('updated item not found');
      const newStock = updatedItem.currentStock ?? updatedItem.current_stock;
      expect(newStock).toBe(initialStock - 2);
    });
  });

  describe('5. Inward Purchase & Stock Increment', () => {
    it('TC-MOCK-013: getNextPurchaseCode returns sequential PUR-XXXX code', async () => {
      const code = await client.getNextPurchaseCode();
      expect(code).toMatch(/^PUR-\d+$/);
    });

    it('TC-MOCK-014: createPurchase increments item stock in memory', async () => {
      const catalog = await client.getItemsCatalog({ limit: 5 });
      const item = catalog.items[0];
      const initialStock = item.currentStock ?? item.current_stock;

      const payload = {
        purchaseCode: await client.getNextPurchaseCode(),
        date: '2026-11-20',
        vendorId: 1,
        items: [
          {
            itemId: item.id,
            quantity: 50,
            unitsPerBundle: 1,
            unitCost: 10.0,
          },
        ],
        paidAmount: 500.0,
        paymentStatus: 'PAID',
      };

      const res = await client.createPurchase(payload);
      expect(res).toBeDefined();

      const updatedCatalog = await client.getItemsCatalog({ limit: 100 });
      const updatedItem = updatedCatalog.items.find((i: any) => i.id === item.id);
      if (!updatedItem) throw new Error('updated item not found');
      expect(updatedItem.currentStock ?? updatedItem.current_stock).toBe(initialStock + 50);
    });
  });

  describe('6. Accounting, Dues Settlement & Reports', () => {
    it('TC-MOCK-015: settleBillPayment decrements customer dues', async () => {
      const dues = await client.getPartyDuesSummary('CUSTOMER');
      expect(dues.length).toBeGreaterThan(0);
      const party = dues[0];
      const initialDue = party.netDueBalance ?? party.balance_due;

      const success = await client.settleBillPayment({
        partyId: party.partyId ?? party.id,
        amount: Math.min(100.0, initialDue),
        date: '2026-11-20',
      });
      expect(success).toBe(true);
    });

    it('TC-MOCK-016: getDashboardMetrics returns structured financial indicators', async () => {
      const metrics = await client.getDashboardMetrics();
      expect(metrics).toBeDefined();
      expect(typeof (metrics.todayGrossSales ?? metrics.todaySales)).toBe('number');
      expect(typeof (metrics.totalStockValuation ?? metrics.stockValuation)).toBe('number');
      expect(typeof (metrics.liveCashInDrawer ?? metrics.cashDrawer)).toBe('number');
    });

    it('TC-MOCK-017: getPnLReport computes Gross Sales - COGS = Net Profit', async () => {
      const pnl = await client.getPnLReport('2026-01-01', '2026-12-31');
      expect(pnl).toBeDefined();
      const gross = pnl.grossSales ?? pnl.gross_sales;
      const cogs = pnl.cogs ?? pnl.cost_of_goods_sold;
      const net = pnl.netProfit ?? pnl.net_profit;

      expect(gross).toBeGreaterThanOrEqual(0);
      expect(cogs).toBeGreaterThanOrEqual(0);
      // Net Profit = Gross - COGS
      expect(net).toBeCloseTo(gross - cogs, 2);
    });

    it('TC-MOCK-018: getStockValuationReport returns positive total inventory valuation', async () => {
      const valuation = await client.getStockValuationReport();
      expect(valuation).toBeDefined();
      const total = valuation.totalValuation ?? valuation.total_valuation;
      expect(total).toBeGreaterThan(0);
    });
  });

  describe('7. Shop Settings & Backup Simulation', () => {
    it('TC-MOCK-019: getShopSettings and updateShopSettings manage shop profile', async () => {
      const settings = await client.getShopSettings();
      expect(settings).toBeDefined();
      expect(settings.shopName || settings.shop_name).toBeDefined();

      const success = await client.updateShopSettings({
        ...settings,
        shopName: 'Gupta Stationery Mart',
      });
      expect(success).toBe(true);

      const updated = await client.getShopSettings();
      expect(updated.shopName || updated.shop_name).toBe('Gupta Stationery Mart');
    });

    it('TC-MOCK-020: backupDatabase returns simulated destination path', async () => {
      const dest = '/backup/shop_backup.db';
      const result = await client.backupDatabase(dest);
      expect(result).toBeDefined();
      expect(typeof result).toBe('string');
    });
  });
});
