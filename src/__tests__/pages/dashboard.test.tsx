import { describe, it, expect, beforeEach } from 'vitest';
import { render } from '../testUtils';
import { DashboardPage, resetDashboardCache } from '@/pages/DashboardPage';
import { installApi, waitFor, click, bodyText } from './helpers';
import type { DashboardMetrics } from '@/types';
import { useAppStore } from '@/store/useAppStore';

const metrics: DashboardMetrics = {
  todaySalesAmount: 1234,
  todaySalesCount: 7,
  todayPurchaseAmount: 500,
  todayPurchaseCount: 2,
  totalStockValuation: 9999,
  customerReceivables: 777,
  vendorPayables: 444,
  cashInHand: 888,
  todayExpensesAmount: 350,
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
    for (const v of ['₹500.00', '₹9,999.00', '₹777.00', '₹444.00', '₹888.00', '7 bills', '2 batches']) {
      expect(bodyText()).toContain(v);
    }
    expect(document.querySelector('[data-testid="kpi-expenses"]')!.textContent).toContain('₹350.00');
    expect(api.getRecentTransactions.mock.calls[0][0]).toBe(10);
    expect(document.querySelector('[data-testid="low-count"]')!.textContent).toBe('2');
    expect(bodyText()).toContain('1 / 10 PCS');
    expect(bodyText()).toContain('SAL-1');
    expect(bodyText()).toContain('RET-1');
    expect(bodyText()).toContain('Inward (F3)');
    expect(bodyText()).toContain('Stock Adjustment');
  });

  it('has no Sales vs Purchases chart (moved to Financial Reports)', async () => {
    const api = installApi({ ...base, getDashboardMetrics: metrics });
    render(<DashboardPage />);
    await waitFor(() => bodyText().includes('₹1,234.00'));
    expect(document.querySelector('[data-testid="trend"]')).toBeNull();
    expect(bodyText()).not.toContain('Sales vs Purchases');
    expect(api.getSalesPurchaseTrend).not.toHaveBeenCalled();
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
    expect(document.querySelector('[data-testid="kpi-sales"]')!.textContent).toContain('–');
  });

  it('groups the figures into three cards with no profit tile', async () => {
    installApi({ ...base, getDashboardMetrics: metrics });
    render(<DashboardPage />);
    await waitFor(() => bodyText().includes('₹1,234.00'));
    const card = (id: string) => document.querySelector(`[data-testid="card-${id}"]`)!;
    const inCard = (id: string) => Array.from(card(id).querySelectorAll('[data-testid^="kpi-"]')).map((e) => e.getAttribute('data-testid'));
    expect(document.querySelectorAll('[data-testid^="card-"]')).toHaveLength(3);
    expect(inCard('today')).toEqual(['kpi-sales', 'kpi-purchases']);
    expect(inCard('dues')).toEqual(['kpi-receivables', 'kpi-payables']);
    expect(inCard('cash')).toEqual(['kpi-cash', 'kpi-expenses', 'kpi-valuation']);
    expect(document.querySelector('[data-testid="kpi-profit"]')).toBeNull();
    expect(bodyText()).not.toMatch(/net profit/i);
    expect(card('dues').textContent).toContain('customers owe you');
    expect(card('dues').textContent).toContain('you owe vendors');
    expect(document.querySelector('[data-testid="kpi-cash"]')!.textContent).toContain('Cash in hand');
    expect(document.querySelector('[data-testid="kpi-cash"]')!.textContent).toContain('₹888.00');
    // Rows are real buttons (keyboard focusable).
    expect(document.querySelector('[data-testid="kpi-sales"]')!.tagName).toBe('BUTTON');
  });

  it('each row opens the page its figure comes from', async () => {
    installApi({ ...base, getDashboardMetrics: metrics });
    render(<DashboardPage />);
    await waitFor(() => bodyText().includes('₹1,234.00'));
    const expected: Record<string, string> = {
      'kpi-sales': 'sales-history',
      'kpi-purchases': 'purchase-history',
      'kpi-receivables': 'dues',
      'kpi-payables': 'dues',
      'kpi-expenses': 'expenses',
      'kpi-valuation': 'reports',
      'kpi-cash': 'reports',
    };
    for (const [id, route] of Object.entries(expected)) {
      useAppStore.setState({ activeRoute: 'dashboard' });
      click(document.querySelector(`[data-testid="${id}"]`)!);
      expect(useAppStore.getState().activeRoute, id).toBe(route);
    }
  });
});
