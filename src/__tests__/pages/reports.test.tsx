import { describe, it, expect, beforeEach } from 'vitest';
import { render } from '../testUtils';
import { ReportsPage } from '@/pages/ReportsPage';
import { installApi, waitFor, bodyText } from './helpers';

describe('ReportsPage', () => {
  beforeEach(() => {
    document.body.innerHTML = '';
  });

  it('renders all P&L lines in order, all cash buckets and the category valuation', async () => {
    const api = installApi({
      getPnLReport: { grossSales: 1000, salesReturns: 100, netSales: 900, cogs: 400, grossProfit: 500, inventoryLosses: 50, netProfit: 400, totalPurchases: 700, salesDiscounts: 0, otherIncome: 0, operatingExpenses: 50, expensesByType: [{ expenseTypeId: 1, expenseTypeName: 'Printer Ink', amount: 30 }, { expenseTypeId: 2, expenseTypeName: 'Tea', amount: 20 }] },
      getCashFlowReport: { salesCashInflow: 1, customerSettlementsInflow: 2, vendorRefundsInflow: 3, purchaseCashOutflow: 4, vendorSettlementsOutflow: 5, refundsCashOutflow: 6, expensesCashOutflow: 7, netCashBalance: -16 },
      getStockValuationReport: { totalValuation: 5000, activeLotsCount: 3, categoriesBreakdown: [{ category: 'Uncategorized', itemsCount: 2, valuation: 5000 }] },
    });
    render(<ReportsPage />);
    await waitFor(() => !!document.querySelector('[data-testid="pnl"]') && !!document.querySelector('[data-testid="cashflow"]') && !!document.querySelector('[data-testid="valuation"]'));
    const ids = Array.from(document.querySelectorAll('[data-testid="pnl"] [data-testid]')).map((e) => e.getAttribute('data-testid'));
    expect(ids).toEqual(['pnl-gross-sales', 'pnl-sales-returns', 'pnl-net-sales', 'pnl-cogs', 'pnl-gross-profit', 'pnl-losses', 'pnl-opex', 'pnl-net-profit', 'pnl-purchases']);
    expect(document.querySelector('[data-testid="pnl-net-profit"]')!.textContent).toContain('₹400.00');
    expect(document.querySelector('[data-testid="pnl-opex"]')!.textContent).toContain('₹50.00');
    const breakdown = Array.from(document.querySelectorAll('[data-opex-breakdown] [data-breakdown]')).map((e) => e.textContent ?? '');
    expect(breakdown[0]).toContain('Printer Ink');
    expect(breakdown[1]).toContain('Tea');
    expect(document.querySelectorAll('[data-testid="cashflow"] [data-testid]').length).toBe(8);
    expect(document.querySelector('[data-testid="cf-expenses"]')!.textContent).toContain('₹7.00');
    expect(bodyText()).toContain('Uncategorized');
    // range defaults to the current month, in local dates
    const [from, to] = api.getPnLReport.mock.calls[0];
    expect(from).toMatch(/^\d{4}-\d{2}-01$/);
    expect(to).toMatch(/^\d{4}-\d{2}-\d{2}$/);
    expect(bodyText()).not.toContain('Export');
  });

  it('shows an error when a report fails', async () => {
    installApi({
      getPnLReport: async () => {
        throw { type: 'ValidationError', details: { field: 'from_date', message: 'bad date' } };
      },
      getCashFlowReport: { salesCashInflow: 0, customerSettlementsInflow: 0, vendorRefundsInflow: 0, purchaseCashOutflow: 0, vendorSettlementsOutflow: 0, refundsCashOutflow: 0, expensesCashOutflow: 0, netCashBalance: 0 },
      getStockValuationReport: { totalValuation: 0, activeLotsCount: 0, categoriesBreakdown: [] },
    });
    render(<ReportsPage />);
    await waitFor(() => bodyText().includes('bad date'));
  });
});
