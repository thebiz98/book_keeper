import { describe, it, expect, beforeEach } from 'vitest';
import { render } from '../testUtils';
import { ItemProfitPanel } from '@/components/domain/ItemProfitPanel';
import { InventorySpeedPanel } from '@/components/domain/InventorySpeedPanel';
import type { InventorySpeedReport, ItemProfitReport } from '@/types';
import { installApi, waitFor, bodyText, click, flush, setInput } from './helpers';

const profitReport = (over: Partial<ItemProfitReport> = {}): ItemProfitReport => ({
  rows: [
    { itemId: 1, name: 'Paper', baseUnit: 'SHEET', qtySold: 12, revenue: 120, cost: 60, profit: 60, marginPct: 50 },
    { itemId: 2, name: 'Pen', baseUnit: 'PCS', qtySold: 2, revenue: 20, cost: 25, profit: -5, marginPct: -25 },
  ],
  totals: { revenue: 140, cost: 85, profit: 55, marginPct: 39.29 },
  reconciliation: { itemsProfit: 55, charges: 10, replacementCost: 3, billsProfit: 62 },
  totalCount: 2,
  page: 1,
  pageSize: 20,
  ...over,
});

const speedReport = (over: Partial<InventorySpeedReport> = {}): InventorySpeedReport => ({
  rows: [
    { itemId: 1, name: 'Paper', baseUnit: 'SHEET', unitsOut: 30, unitsPerDay: 1, stock: 60, daysOfCover: 60, class: 'FAST' },
    { itemId: 2, name: 'Dust', baseUnit: 'PCS', unitsOut: 0, unitsPerDay: 0, stock: 5, daysOfCover: null, class: 'DEAD' },
  ],
  counts: { fast: 1, normal: 0, slow: 0, dead: 1 },
  days: 30,
  totalCount: 2,
  page: 1,
  pageSize: 20,
  ...over,
});

const catalogItem = (itemId: number, name: string) => ({
  itemId, name, category: null, barcode: null, baseUnit: 'PCS', itemType: 'PHYSICAL', mrp: null, defaultSalePrice: 5,
  currentStock: 7, isLowStock: false, lowStockThreshold: 0, bundles: [], matchedBundleId: null,
});

describe('ItemProfitPanel (FR-38)', () => {
  beforeEach(() => {
    document.body.innerHTML = '';
  });

  it('shows rows, totals and the reconciliation from the backend', async () => {
    const api = installApi({ getItemProfitReport: profitReport() });
    render(<ItemProfitPanel fromDate="2026-11-01" toDate="2026-11-25" />);
    await waitFor(() => document.querySelectorAll('[data-testid="item-profit-row"]').length === 2);
    const rows = Array.from(document.querySelectorAll('[data-testid="item-profit-row"]')).map((r) => r.textContent ?? '');
    expect(rows[0]).toContain('Paper');
    expect(rows[0]).toContain('50.0%');
    expect(rows[1]).toContain('-25.0%');
    const totals = document.querySelector('[data-testid="item-profit-totals"]')!.textContent!;
    expect(totals).toContain('140.00');
    expect(totals).toContain('39.3%');
    const rec = document.querySelector('[data-testid="item-profit-reconciliation"]')!.textContent!;
    expect(rec).toContain('55.00');
    expect(rec).toContain('62.00');
    expect(api.getItemProfitReport.mock.calls[0][0]).toMatchObject({
      fromDate: '2026-11-01', toDate: '2026-11-25', search: null, sort: { field: 'profit', dir: 'DESC' }, page: 1,
    });
  });

  it('sorts by a header click and searches through the backend', async () => {
    const api = installApi({ getItemProfitReport: profitReport() });
    render(<ItemProfitPanel fromDate="2026-11-01" toDate="2026-11-25" />);
    await waitFor(() => document.querySelectorAll('[data-testid="item-profit-row"]').length === 2);
    click(document.querySelector('[data-testid="sort-name"]')!);
    await flush();
    expect(api.getItemProfitReport.mock.calls.at(-1)![0].sort).toEqual({ field: 'name', dir: 'ASC' });
    click(document.querySelector('[data-testid="sort-name"]')!);
    await flush();
    expect(api.getItemProfitReport.mock.calls.at(-1)![0].sort).toEqual({ field: 'name', dir: 'DESC' });
    setInput(document.querySelector('input[aria-label="Search items"]')!, 'pap');
    await waitFor(() => api.getItemProfitReport.mock.calls.at(-1)![0].search === 'pap');
  });

  it('shows the error with Retry', async () => {
    let fail = true;
    installApi({
      getItemProfitReport: async () => {
        if (fail) throw { type: 'ValidationError', details: { field: 'from_date', message: 'bad range' } };
        return profitReport();
      },
    });
    render(<ItemProfitPanel fromDate="2026-11-01" toDate="2026-11-25" />);
    await waitFor(() => bodyText().includes('bad range'));
    fail = false;
    click(Array.from(document.querySelectorAll('button')).find((b) => b.textContent === 'Retry')!);
    await waitFor(() => document.querySelectorAll('[data-testid="item-profit-row"]').length === 2);
  });

  it('opens Item details on a row click', async () => {
    const api = installApi({
      getItemProfitReport: profitReport(),
      getItem: catalogItem(1, 'Paper'),
      getItemLots: [],
      getItemSales: { items: [], total: 0, page: 1, limit: 10 },
      getItemPurchases: { items: [], total: 0, page: 1, limit: 10 },
    });
    render(<ItemProfitPanel fromDate="2026-11-01" toDate="2026-11-25" />);
    await waitFor(() => document.querySelectorAll('[data-testid="item-profit-row"]').length === 2);
    click(document.querySelector('[data-testid="item-profit-row"]')!);
    await waitFor(() => bodyText().includes('Sale Price'));
    expect(api.getItem).toHaveBeenCalledWith(1);
  });
});

describe('InventorySpeedPanel (FR-39)', () => {
  beforeEach(() => {
    document.body.innerHTML = '';
  });

  it('defaults to the last 30 days and shows classes, cover and tab counts', async () => {
    const api = installApi({ getInventorySpeedReport: speedReport() });
    render(<InventorySpeedPanel />);
    await waitFor(() => document.querySelectorAll('[data-testid="inventory-speed-row"]').length === 2);
    const f = api.getInventorySpeedReport.mock.calls[0][0];
    expect(f.class).toBeNull();
    expect(f.fromDate).toMatch(/^\d{4}-\d{2}-\d{2}$/);
    const rows = Array.from(document.querySelectorAll('[data-testid="inventory-speed-row"]')).map((r) => r.textContent ?? '');
    expect(rows[0]).toContain('Fast');
    expect(rows[0]).toContain('60');
    expect(rows[1]).toContain('Dead stock');
    expect(rows[1]).toContain('-');
    expect(bodyText()).toContain('30 days in this period');
    const tabs = Array.from(document.querySelectorAll('button')).map((b) => b.textContent ?? '');
    expect(tabs).toContain('All2');
    expect(tabs).toContain('Dead1');
  });

  it('sends the class when a tab is clicked and sorts through the backend', async () => {
    const api = installApi({ getInventorySpeedReport: speedReport() });
    render(<InventorySpeedPanel />);
    await waitFor(() => document.querySelectorAll('[data-testid="inventory-speed-row"]').length === 2);
    click(Array.from(document.querySelectorAll('button')).find((b) => (b.textContent ?? '').startsWith('Dead'))!);
    await waitFor(() => api.getInventorySpeedReport.mock.calls.at(-1)![0].class === 'DEAD');
    click(document.querySelector('[data-testid="sort-stock"]')!);
    await waitFor(() => api.getInventorySpeedReport.mock.calls.at(-1)![0].sort.field === 'stock');
    expect(api.getInventorySpeedReport.mock.calls.at(-1)![0].sort.dir).toBe('DESC');
  });

  it('shows the error with Retry and opens Item details on a row click', async () => {
    let fail = true;
    installApi({
      getInventorySpeedReport: async () => {
        if (fail) throw new Error('boom');
        return speedReport();
      },
      getItem: catalogItem(1, 'Paper'),
      getItemLots: [],
      getItemSales: { items: [], total: 0, page: 1, limit: 10 },
      getItemPurchases: { items: [], total: 0, page: 1, limit: 10 },
    });
    render(<InventorySpeedPanel />);
    await waitFor(() => bodyText().includes('boom'));
    fail = false;
    click(Array.from(document.querySelectorAll('button')).find((b) => b.textContent === 'Retry')!);
    await waitFor(() => document.querySelectorAll('[data-testid="inventory-speed-row"]').length === 2);
    click(document.querySelector('[data-testid="inventory-speed-row"]')!);
    await waitFor(() => bodyText().includes('Sale Price'));
  });
});
