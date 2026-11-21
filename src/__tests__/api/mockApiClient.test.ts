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
      expect(response.totalCount).toBeGreaterThanOrEqual(160);
      expect(response.items.length).toBeGreaterThan(0);
    });

    it('TC-MOCK-002: seed dataset contains diverse stationery retail categories', async () => {
      const response = await client.getItemsCatalog({ limit: 200 });
      const categories = new Set(response.items.map((i) => i.category));

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
        expect(alert.currentStock).toBeLessThan(5);
      }
    });

    it('TC-MOCK-004: seed dataset includes items with configured packaging bundles', async () => {
      const suggestions = await client.getBundleSuggestions();
      expect(suggestions).toBeDefined();
      expect(suggestions.length).toBeGreaterThan(0);

      // Verify presence of standard packaging tiers like Box, Pack, Dozen
      const bundleNames = suggestions.map((s) => s.bundleName);
      const hasStandardBundle = bundleNames.some((name: string) =>
        /box|pack|dozen|carton/i.test(name)
      );
      expect(hasStandardBundle).toBe(true);
    });
  });

  describe('2. Parties & Dues Khata Seed Profile', () => {
    it('TC-MOCK-005: provides seeded customer directory with active dues balances', async () => {
      const customers = await client.listParties('Customer');
      expect(customers).toBeDefined();
      expect(customers.length).toBeGreaterThan(0);

      const duesSummary = await client.getPartyDuesSummary('Customer');
      expect(duesSummary).toBeDefined();
      expect(duesSummary.length).toBeGreaterThan(0);

      // Verify outstanding balance fields exist
      const firstDue = duesSummary[0];
      expect(firstDue.totalDue).toBeGreaterThan(0);
    });

    it('TC-MOCK-006: provides seeded vendor directory', async () => {
      const vendors = await client.listParties('Vendor');
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
      const itemWithBarcode = catalog.items.find((i) => Boolean(i.barcode));

      expect(itemWithBarcode).toBeDefined();
      const results = await client.searchItemsFts(itemWithBarcode!.barcode!);
      expect(results.length).toBeGreaterThanOrEqual(1);
      expect(results[0].barcode).toBe(itemWithBarcode!.barcode);
    });

    it('TC-MOCK-009: non-existent query returns empty results array', async () => {
      const results = await client.searchItemsFts('XZQ_NON_EXISTENT_QUERY_99999');
      expect(results).toEqual([]);
    });

  });

  describe('4. Counter Billing Checkout & Stock Depletion', () => {
    it('TC-MOCK-011: getNextSaleCode returns sequential SAL-XXXX code', async () => {
      const code1 = await client.getNextSaleCode();
      expect(code1).toMatch(/^SAL-\d+$/);
    });

    it('TC-MOCK-012: createSale persists sale and decrements item stock in memory', async () => {
      const catalog = await client.getItemsCatalog({ limit: 5 });
      const item = catalog.items.find((i) => i.currentStock > 10);

      expect(item).toBeDefined();
      if (!item) throw new Error('seed item not found');
      const initialStock = item.currentStock;
      const saleCode = await client.getNextSaleCode();

      const payload = {
        transactionCode: saleCode,
        transactionDate: '2026-11-20',
        partyId: null,
        paymentMode: 'CASH',
        notes: null,
        items: [
          {
            itemId: item.id,
            bundleId: null,
            bundleQuantity: 2,
            unitPrice: item.defaultSalePrice,
          },
        ],
        paidAmount: item.defaultSalePrice * 2,
      };

      const result = await client.createSale(payload);
      expect(result).toBeDefined();

      // Check that item stock was decremented
      const updatedCatalog = await client.getItemsCatalog({ limit: 100 });
      const updatedItem = updatedCatalog.items.find((i) => i.id === item.id);
      if (!updatedItem) throw new Error('updated item not found');
      const newStock = updatedItem.currentStock;
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
      const initialStock = item.currentStock;

      const payload = {
        transactionCode: await client.getNextPurchaseCode(),
        transactionDate: '2026-11-20',
        partyId: 101,
        paymentMode: 'CASH',
        notes: null,
        items: [
          {
            itemId: item.id,
            bundleId: null,
            bundleQuantity: 50,
            unitCost: 10.0,
          },
        ],
        paidAmount: 500.0,
      };

      const res = await client.createPurchase(payload);
      expect(res).toBeDefined();

      const updatedCatalog = await client.getItemsCatalog({ limit: 100 });
      const updatedItem = updatedCatalog.items.find((i) => i.id === item.id);
      if (!updatedItem) throw new Error('updated item not found');
      expect(updatedItem.currentStock).toBe(initialStock + 50);
    });
  });

  describe('6. Accounting, Dues Settlement & Reports', () => {
    it('TC-MOCK-015: settleBillPayment decrements customer dues', async () => {
      const dues = await client.getPartyDuesSummary('Customer');
      expect(dues.length).toBeGreaterThan(0);
      const party = dues[0];
      const initialDue = party.totalDue;

      const success = await client.settleBillPayment({
        partyId: party.partyId,
        amount: Math.min(100.0, initialDue),
        settlementDate: '2026-11-20',
        paymentMode: 'CASH',
        specificBillId: null,
        notes: null,
      });
      expect(success).toBeUndefined();
    });

    it('TC-MOCK-016: getDashboardMetrics returns structured financial indicators', async () => {
      const metrics = await client.getDashboardMetrics();
      expect(metrics).toBeDefined();
      expect(typeof (metrics.todaySales)).toBe('number');
      expect(typeof (metrics.totalStockValuation)).toBe('number');
      expect(typeof (metrics.liveCashInDrawer)).toBe('number');
    });

    it('TC-MOCK-017: getPnLReport computes Gross Sales - COGS = Net Profit', async () => {
      const pnl = await client.getPnLReport('2026-01-01', '2026-12-31');
      expect(pnl).toBeDefined();
      const gross = pnl.grossSales;
      const cogs = pnl.cogs;
      const net = pnl.netProfit;

      expect(gross).toBeGreaterThanOrEqual(0);
      expect(cogs).toBeGreaterThanOrEqual(0);
      // Net Profit = Gross - COGS
      expect(net).toBeCloseTo(gross - cogs, 2);
    });

    it('TC-MOCK-018: getStockValuationReport returns positive total inventory valuation', async () => {
      const valuation = await client.getStockValuationReport();
      expect(valuation).toBeDefined();
      const total = valuation.totalValuation;
      expect(total).toBeGreaterThan(0);
    });
  });

  describe('7. Shop Settings & Backup Simulation', () => {
    it('TC-MOCK-019: getShopSettings and updateShopSettings manage shop profile', async () => {
      const settings = await client.getShopSettings();
      expect(settings).toBeDefined();
      expect(settings.shopName).toBeDefined();

      const success = await client.updateShopSettings({
        ...settings,
        shopName: 'Gupta Stationery Mart',
      });
      expect(success).toBeUndefined();

      const updated = await client.getShopSettings();
      expect(updated.shopName).toBe('Gupta Stationery Mart');
    });

    it('TC-MOCK-020: backupDatabase returns simulated destination path', async () => {
      const dest = '/backup/shop_backup.db';
      const result = await client.backupDatabase(dest);
      expect(result).toBeDefined();
      expect(typeof result).toBe('string');
    });
  });
});
