import { describe, it, expect, beforeEach } from 'vitest';
import { render } from '../testUtils';
import { DashboardPage, resetDashboardCache } from '@/pages/DashboardPage';
import { CreditDuesPage } from '@/pages/CreditDuesPage';
import { EditTotals } from '@/components/domain/billEdit/EditParts';
import type { DashboardMetrics, PartyDueSummary } from '@/types';
import { installApi, flush, waitFor, click, buttonByText, bodyText } from './helpers';

const GREEN = 'text-emerald-600';
const RED = 'text-rose-600';

/** The CurrencyDisplay span inside `root` whose text shows `amount`. */
const amountSpan = (root: ParentNode, amount: RegExp) =>
  Array.from(root.querySelectorAll('span.font-mono')).find((s) => amount.test(s.textContent ?? '')) as HTMLElement;

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

const party = (partyType: 'CUSTOMER' | 'VENDOR'): PartyDueSummary => ({
  partyId: 7,
  partyName: 'Sharma Traders',
  phone: '999',
  partyType,
  unsettledBillsCount: 1,
  earliestBillDate: '2026-01-01',
  totalInvoiced: 500,
  totalPaid: 200,
  balanceDue: 300,
});

describe('Dues colours everywhere (FR-49)', () => {
  beforeEach(() => {
    document.body.innerHTML = '';
    resetDashboardCache();
  });

  it('Dashboard Dues card: receivables green, payables red', async () => {
    installApi({
      getDashboardMetrics: metrics,
      getLowStockAlerts: [],
      getRecentTransactions: [],
      getSalesPurchaseTrend: [],
    });
    render(<DashboardPage />);
    await waitFor(() => bodyText().includes('₹444.00'));
    const rec = document.querySelector('[data-testid="kpi-receivables"]')!;
    const pay = document.querySelector('[data-testid="kpi-payables"]')!;
    expect(amountSpan(rec, /777\.00/).className).toContain(GREEN);
    expect(amountSpan(pay, /444\.00/).className).toContain(RED);
  });

  it('Dues page: customer total and row green, vendor total and row red', async () => {
    installApi({
      getPartyDuesSummary: async (t: 'CUSTOMER' | 'VENDOR') => [party(t)],
    });
    render(<CreditDuesPage />);
    await waitFor(() => bodyText().includes('Sharma Traders'));
    let spans = Array.from(document.querySelectorAll('span.font-mono')).filter((s) => /300\.00/.test(s.textContent ?? ''));
    expect(spans.length).toBeGreaterThanOrEqual(2); // tab total + party row
    for (const s of spans) expect(s.className).toContain(GREEN);

    click(buttonByText(/Vendor Payables/));
    await flush();
    await waitFor(() => bodyText().includes('Total Payables Due'));
    spans = Array.from(document.querySelectorAll('span.font-mono')).filter((s) => /300\.00/.test(s.textContent ?? ''));
    expect(spans.length).toBeGreaterThanOrEqual(2);
    for (const s of spans) expect(s.className).toContain(RED);
  });

  it('bill edit balance: sale green, purchase red', () => {
    render(<EditTotals totals={null} balanceDue={120} />);
    expect(amountSpan(document.querySelector('[data-testid="edit-balance"]')!, /120\.00/).className).toContain(GREEN);
    document.body.innerHTML = '';
    render(<EditTotals totals={null} balanceDue={120} payable />);
    expect(amountSpan(document.querySelector('[data-testid="edit-balance"]')!, /120\.00/).className).toContain(RED);
  });
});
