import { describe, it, expect } from 'vitest';
import { MockApiClient } from '@/api/mock/mockClient';
import type { ApiClient } from '@/types';
import { isAppError } from '@/api/errors';

// Contract-shape test (DD-27): the mock is a thin fixture client, so we only
// assert that every ApiClient method exists and returns the right top-level keys.

const keys = (o: unknown) => Object.keys(o as object).sort();
const has = (o: unknown, ks: string[]) => expect(keys(o)).toEqual(expect.arrayContaining(ks));

const payload = <T>(p: Partial<T>) => p as T;

describe('MockApiClient contract shapes', () => {
  const c: ApiClient = new MockApiClient();

  it('implements every ApiClient method', () => {
    const methods = [
      'getNextSaleCode', 'createSale', 'getSalesHistory', 'getSaleDetails',
      'getNextPurchaseCode', 'createPurchase', 'getPurchasesHistory', 'getPurchaseDetails',
      'searchItemsFts', 'getItemsCatalog', 'listCategories', 'createItem', 'updateItem',
      'getBundleSuggestions', 'createBundle', 'updateBundle', 'deleteBundle',
      'getPartyDuesSummary', 'settleBillPayment', 'getPartyLedger',
      'createParty', 'updateParty', 'getParty', 'listParties', 'searchParties',
      'getNextReturnCode', 'createCustomerReturn', 'createVendorReturn', 'getReturnDetails',
      'listReturns', 'createStockAdjustment', 'getItemLots', 'listStockAdjustments',
      'getDashboardMetrics', 'getLowStockAlerts', 'getSalesPurchaseTrend', 'getRecentTransactions',
      'getPnLReport', 'getStockValuationReport', 'getCashFlowReport',
      'getShopSettings', 'updateShopSettings', 'backupDatabase', 'verifyDatabaseIntegrity',
      'validateItemsImport', 'executeItemsImport',
    ];
    for (const m of methods) expect(typeof (c as unknown as Record<string, unknown>)[m]).toBe('function');
  });

  it('catalog and search return the catalog shape', async () => {
    const page = await c.getItemsCatalog({ limit: 5 });
    expect(keys(page)).toEqual(['items', 'limit', 'page', 'total']);
    has(page.items[0], ['itemId', 'name', 'bundles', 'matchedBundleId', 'isLowStock', 'itemType']);
    const hits = await c.searchItemsFts('pen');
    has(hits[0], ['itemId', 'bundles', 'matchedBundleId']);
    expect(Array.isArray(await c.listCategories())).toBe(true);
  });

  it('createItem without itemType defaults to PHYSICAL (FR-21)', async () => {
    const id = await c.createItem({
      name: 'Type Probe', category: null, barcode: null, baseUnit: 'PCS', mrp: null,
      defaultSalePrice: 1, initialStock: null, initialCost: null, bundles: null,
    });
    const page = await c.getItemsCatalog({ search: 'Type Probe' });
    const row = page.items.find((i) => i.itemId === id);
    expect(row?.itemType).toBe('PHYSICAL');
  });

  it('updateItemLowStockThreshold returns the row with the new level', async () => {
    const page = await c.getItemsCatalog({ limit: 1 });
    const id = page.items[0].itemId;
    const row = await c.updateItemLowStockThreshold(id, 12);
    expect(row.itemId).toBe(id);
    expect(row.lowStockThreshold).toBe(12);
    await expect(c.updateItemLowStockThreshold(99999999, 1)).rejects.toSatisfy(isAppError);
  });

  it('simulates one typed error path', async () => {
    await expect(c.searchItemsFts('__error__')).rejects.toSatisfy(isAppError);
  });

  it('sale, purchase and history shapes', async () => {
    const sale = await c.createSale(
      payload<Parameters<ApiClient['createSale']>[0]>({
        transactionDate: '2026-11-21', partyId: null, paymentStatus: 'PAID', paidAmount: 100,
        paymentMode: 'CASH', notes: null,
        items: [{ itemId: 1001, bundleId: null, bundleQuantity: 1, unitPrice: 50 }],
      })
    );
    has(sale, ['saleId', 'transactionCode', 'totalAmount', 'paidAmount', 'balanceDue', 'changeAmount', 'paymentStatus']);
    const hist = await c.getSalesHistory();
    expect(keys(hist)).toEqual(['items', 'limit', 'page', 'total']);
    has(hist.items[0], ['id', 'transactionCode', 'lineCount', 'partyName', 'balanceDue']);
    has(await c.getSaleDetails(sale.saleId), ['sale', 'party', 'items', 'settlements']);

    const pur = await c.createPurchase(
      payload<Parameters<ApiClient['createPurchase']>[0]>({
        transactionDate: '2026-11-21', partyId: null, supplierInvoiceRef: 'X1', paymentStatus: 'PAID',
        paidAmount: 10, paymentMode: null, notes: null,
        items: [{ itemId: 1001, bundleId: null, bundleQuantity: 1, unitCost: 10 }],
      })
    );
    expect(keys(pur)).toEqual(['purchaseId', 'transactionCode']);
    const ph = await c.getPurchasesHistory();
    has(ph.items[0], ['supplierInvoiceRef']);
    has(await c.getPurchaseDetails(pur.purchaseId), ['purchase', 'party', 'items', 'settlements']);
  });

  it('parties and dues shapes', async () => {
    has((await c.listParties('CUSTOMER'))[0], ['id', 'name', 'outstandingBalance', 'transactionCount']);
    has((await c.getPartyDuesSummary('CUSTOMER'))[0], [
      'partyId', 'unsettledBillsCount', 'earliestBillDate', 'totalInvoiced', 'totalPaid', 'balanceDue',
    ]);
    has(await c.getPartyLedger(1), ['party', 'currentBalance', 'bills', 'payments', 'returns', 'entries']);
    has(
      await c.settleBillPayment(
        payload<Parameters<ApiClient['settleBillPayment']>[0]>({ partyId: 1, amountPaid: 1, transactionId: null })
      ),
      ['settlementIds', 'billsTouched']
    );
  });

  it('returns, adjustments, lots', async () => {
    const r = await c.createCustomerReturn(
      payload<Parameters<ApiClient['createCustomerReturn']>[0]>({
        returnDate: '2026-11-21', partyId: null, settlementType: 'REFUND', notes: null,
        items: [], replacementItems: [],
      })
    );
    expect(keys(r)).toEqual(['cashRefunded', 'creditedAmount', 'returnCode', 'returnId']);
    has(await c.getReturnDetails(r.returnId), ['returnTx', 'party', 'items', 'allocations', 'ledger']);
    expect(keys(await c.listReturns())).toEqual(['items', 'limit', 'page', 'total']);
    const adj = await c.createStockAdjustment(
      payload<Parameters<ApiClient['createStockAdjustment']>[0]>({
        adjustmentDate: '2026-11-21', itemId: 1001, quantity: -1, reason: 'DAMAGED', unitCost: null, notes: null,
      })
    );
    expect(keys(adj)).toEqual(['adjustmentId', 'lossValue']);
    expect(keys(await c.listStockAdjustments())).toEqual(['items', 'limit', 'page', 'total']);
    has((await c.getItemLots(1001))[0], ['purchaseItemId', 'remainingQuantity', 'unitCost', 'vendorName']);
  });

  it('report shapes', async () => {
    has(await c.getDashboardMetrics(), ['todaySalesAmount', 'todaySalesCount', 'liveCashInDrawer', 'vendorPayables']);
    has((await c.getLowStockAlerts())[0], ['id', 'currentStock', 'baseUnit']);
    expect((await c.getSalesPurchaseTrend('7d')).length).toBe(7);
    has((await c.getSalesPurchaseTrend('1y'))[0], ['date', 'salesAmount', 'purchaseAmount']);
    has(await c.getPnLReport('2026-11-01', '2026-11-21'), ['grossSales', 'salesReturns', 'netSales', 'inventoryLosses', 'totalPurchases']);
    has(await c.getStockValuationReport(), ['totalValuation', 'activeLotsCount', 'categoriesBreakdown']);
    has(await c.getCashFlowReport('2026-11-01', '2026-11-21'), ['salesCashInflow', 'vendorRefundsInflow', 'netCashBalance']);
    expect(Array.isArray(await c.getRecentTransactions(5))).toBe(true);
  });

  it('settings, import, bundles', async () => {
    has(await c.getShopSettings(), ['shopName', 'taxId']);
    expect(await c.verifyDatabaseIntegrity()).toBe(true);
    has(await c.validateItemsImport('x.xlsx'), ['validCount', 'rowsWithErrors', 'duplicates']);
    has(await c.executeItemsImport('x.xlsx', 'SKIP'), ['created', 'updated', 'skipped', 'stockLotsCreated']);
    const b = await c.createBundle({
      itemId: 1001, bundleName: 'Test', unitsPerBundle: 3, mrp: null, defaultBundlePrice: null, barcode: null,
    });
    has(b, ['id', 'itemId', 'bundleName']);
    has(await c.updateBundle(b.id, { bundleName: 'T2', unitsPerBundle: 4, mrp: null, defaultBundlePrice: null, barcode: null }), ['id']);
    await c.deleteBundle(b.id);
  });
});
