import { describe, it, expect, beforeEach } from 'vitest';
import { render } from '../testUtils';
import { DashboardPage, resetDashboardCache } from '@/pages/DashboardPage';
import { installApi, flush, waitFor, click, bodyText } from './helpers';
import type { DashboardMetrics } from '@/types';

const metrics: DashboardMetrics = {
  todaySalesAmount: 1234,
  todaySalesCount: 7,
  todayPurchaseAmount: 500,
  todayPurchaseCount: 2,
  todayNetProfit: 321,
  totalStockValuation: 9999,
  customerReceivables: 777,
  vendorPayables: 444,
  liveCashInDrawer: 888,
};

const base = {
  getLowStockAlerts: [
    { id: 1, name: 'Pencil', category: null, baseUnit: 'PCS', currentStock: 1, defaultSalePrice: 5, mrp: null, lowStockThreshold: 10 },
    { id: 2, name: 'Eraser', category: 'Misc', baseUnit: 'PCS', currentStock: 2, defaultSalePrice: 5, mrp: null, lowStockThreshold: 5 },
  ],
  getRecentTransactions: [
    { kind: 'SALE', id: 1, code: 'SAL-1', date: '2026-02-01', createdAt: '2026-02-01T10:30:00', partyName: 'Ram', amount: 100, paymentStatus: 'PAID' },
    { kind: 'RETURN', id: 2, code: 'RET-1', date: '2026-02-01', createdAt: '2026-02-01T11:30:00', partyName: null, amount: 20, paymentStatus: null },
  ],
  getSalesPurchaseTrend: [{ date: '2026-02-01', salesAmount: 10, purchaseAmount: 5 }],
};

describe('DashboardPage', () => {
  beforeEach(() => {
    document.body.innerHTML = '';
    resetDashboardCache();
  });

  it('renders all KPIs, counts, recent feed and the uncapped low-stock list', async () => {
    const api = installApi({ ...base, getDashboardMetrics: metrics });
    render(<DashboardPage />);
    await waitFor(() => bodyText().includes('₹1,234.00'));
    for (const v of ['₹500.00', '₹321.00', '₹9,999.00', '₹777.00', '₹444.00', '₹888.00', '7 bills', '2 batches']) {
      expect(bodyText()).toContain(v);
    }
    expect(api.getRecentTransactions.mock.calls[0][0]).toBe(10);
    expect(document.querySelector('[data-testid="low-count"]')!.textContent).toBe('2');
    expect(bodyText()).toContain('1 / 10 PCS');
    expect(bodyText()).toContain('SAL-1');
    expect(bodyText()).toContain('RET-1');
    expect(bodyText()).toContain('Inward (F3)');
    expect(bodyText()).toContain('Stock Adjustment');
  });

  it('trend toggle requests 30d and 1y', async () => {
    const api = installApi({ ...base, getDashboardMetrics: metrics });
    render(<DashboardPage />);
    await waitFor(() => bodyText().includes('₹1,234.00'));
    click(document.querySelector('[data-period="30d"]')!);
    await flush();
    click(document.querySelector('[data-period="1y"]')!);
    await flush();
    expect(api.getSalesPurchaseTrend.mock.calls.map((c) => c[0])).toEqual(expect.arrayContaining(['7d', '30d', '1y']));
  });

  it('shows an error, not fake zeros, when metrics fail', async () => {
    installApi({
      ...base,
      getDashboardMetrics: async () => {
        throw { type: 'DatabaseError', details: 'disk I/O error' };
      },
    });
    render(<DashboardPage />);
    await waitFor(() => bodyText().includes('disk I/O error'));
    expect(document.querySelector('[data-testid="kpi-sales"]')!.textContent).not.toContain('₹0.00');
    expect(document.querySelector('[data-testid="kpi-sales"]')!.textContent).toContain('-');
  });
});
