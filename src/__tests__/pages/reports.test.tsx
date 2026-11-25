import { describe, it, expect, beforeEach } from 'vitest';
import { render } from '../testUtils';
import { ReportsPage } from '@/pages/ReportsPage';
import { installApi, waitFor, bodyText, click, flush } from './helpers';

describe('ReportsPage', () => {
  beforeEach(() => {
    document.body.innerHTML = '';
  });

  it('renders all P&L lines in order, all cash buckets and the category valuation', async () => {
    const api = installApi({
      getPnLReport: { grossSales: 1000, salesReturns: 100, netSales: 900, cogs: 400, grossProfit: 500, inventoryLosses: 50, netProfit: 400, totalPurchases: 700, salesDiscounts: 0, otherIncome: 0, saleChargesIncome: 0, purchaseDiscountsIncome: 0, purchaseChargesExpense: 0, operatingExpenses: 50, expensesByType: [{ expenseTypeId: 1, expenseTypeName: 'Printer Ink', amount: 30 }, { expenseTypeId: 2, expenseTypeName: 'Tea', amount: 20 }] },
      getCashFlowReport: { salesCashInflow: 1, customerSettlementsInflow: 2, vendorRefundsInflow: 3, purchaseCashOutflow: 4, vendorSettlementsOutflow: 5, refundsCashOutflow: 6, expensesCashOutflow: 7, totalInflow: 61, totalOutflow: 77, netCashBalance: -16 },
      getStockValuationReport: { totalValuation: 5000, activeLotsCount: 3, categoriesBreakdown: [{ category: 'Uncategorized', itemsCount: 2, valuation: 5000 }] },
    });
    render(<ReportsPage />);
    await waitFor(() => !!document.querySelector('[data-testid="pnl"]') && !!document.querySelector('[data-testid="cashflow"]') && !!document.querySelector('[data-testid="valuation"]'));
    const ids = Array.from(document.querySelectorAll('[data-testid="pnl"] [data-testid]')).map((e) => e.getAttribute('data-testid'));
    expect(ids).toEqual(['pnl-gross-sales', 'pnl-sales-discounts', 'pnl-sales-returns', 'pnl-net-sales', 'pnl-cogs', 'pnl-gross-profit', 'pnl-other-income', 'pnl-losses', 'pnl-opex', 'pnl-net-profit', 'pnl-purchases']);
    expect(document.querySelector('[data-testid="pnl-net-profit"]')!.textContent).toContain('₹400.00');
    expect(document.querySelector('[data-testid="pnl-opex"]')!.textContent).toContain('₹50.00');
    const breakdown = Array.from(document.querySelectorAll('[data-opex-breakdown] [data-breakdown]')).map((e) => e.textContent ?? '');
    expect(breakdown[0]).toContain('Printer Ink');
    expect(breakdown[1]).toContain('Tea');
    expect(document.querySelectorAll('[data-testid="cashflow"] [data-testid]').length).toBe(10);
    expect(document.querySelector('[data-testid="cf-expenses"]')!.textContent).toContain('₹7.00');
    // DD-56: the totals are the backend's, shown as returned (not re-summed in the UI).
    expect(document.querySelector('[data-testid="cf-total-inflow"]')!.textContent).toContain('₹61.00');
    expect(document.querySelector('[data-testid="cf-total-outflow"]')!.textContent).toContain('₹77.00');
    expect(bodyText()).toContain('Uncategorized');
    // range defaults to the current month, in local dates
    const [from, to] = api.getPnLReport.mock.calls[0];
    expect(from).toMatch(/^\d{4}-\d{2}-01$/);
    expect(to).toMatch(/^\d{4}-\d{2}-\d{2}$/);
    expect(bodyText()).not.toContain('Export');
  });

  it('renders sales discounts, other income sub-lines and purchase charges (FR-19)', async () => {
    installApi({
      getPnLReport: { grossSales: 1000, salesReturns: 0, netSales: 980, cogs: 400, grossProfit: 580, inventoryLosses: 0, netProfit: 620, totalPurchases: 700, salesDiscounts: 20, otherIncome: 60, saleChargesIncome: 50, purchaseDiscountsIncome: 10, purchaseChargesExpense: 40, operatingExpenses: 40, expensesByType: [] },
      getCashFlowReport: { salesCashInflow: 0, customerSettlementsInflow: 0, vendorRefundsInflow: 0, purchaseCashOutflow: 0, vendorSettlementsOutflow: 0, refundsCashOutflow: 0, expensesCashOutflow: 0, netCashBalance: 0 },
      getStockValuationReport: { totalValuation: 0, activeLotsCount: 0, categoriesBreakdown: [] },
    });
    render(<ReportsPage />);
    await waitFor(() => !!document.querySelector('[data-testid="pnl"]'));
    const t = (id: string) => document.querySelector('[data-testid="' + id + '"]')!.textContent ?? '';
    expect(t('pnl-sales-discounts')).toContain('20.00');
    expect(t('pnl-other-income')).toContain('60.00');
    expect(t('pnl-sale-charges')).toContain('50.00');
    expect(t('pnl-purchase-discounts')).toContain('10.00');
    expect(t('pnl-purchase-charges')).toContain('40.00');
  });

  it('shows an error when a report fails', async () => {
    installApi({
      getPnLReport: async () => {
        throw { type: 'ValidationError', details: { field: 'from_date', message: 'bad date' } };
      },
      getCashFlowReport: { salesCashInflow: 0, customerSettlementsInflow: 0, vendorRefundsInflow: 0, purchaseCashOutflow: 0, vendorSettlementsOutflow: 0, refundsCashOutflow: 0, expensesCashOutflow: 0, totalInflow: 0, totalOutflow: 0, netCashBalance: 0 },
      getStockValuationReport: { totalValuation: 0, activeLotsCount: 0, categoriesBreakdown: [] },
    });
    render(<ReportsPage />);
    await waitFor(() => bodyText().includes('bad date'));
  });
  it('shows the Sales vs Purchases trend only when asked, with 7D / 30D / 1Y', async () => {
    const api = installApi({
      getPnLReport: { grossSales: 0, salesReturns: 0, netSales: 0, cogs: 0, grossProfit: 0, inventoryLosses: 0, netProfit: 0, totalPurchases: 0, salesDiscounts: 0, otherIncome: 0, saleChargesIncome: 0, purchaseDiscountsIncome: 0, purchaseChargesExpense: 0, operatingExpenses: 0, expensesByType: [] },
      getCashFlowReport: { salesCashInflow: 0, customerSettlementsInflow: 0, vendorRefundsInflow: 0, purchaseCashOutflow: 0, vendorSettlementsOutflow: 0, refundsCashOutflow: 0, expensesCashOutflow: 0, totalInflow: 0, totalOutflow: 0, netCashBalance: 0 },
      getStockValuationReport: { totalValuation: 0, activeLotsCount: 0, categoriesBreakdown: [] },
      getSalesPurchaseTrend: [{ date: '2026-02-01', salesAmount: 10, purchaseAmount: 5 }],
    });
    render(<ReportsPage />);
    await waitFor(() => !!document.querySelector('[data-testid="pnl"]'));
    expect(document.querySelector('[data-testid="trend"]')).toBeNull();
    expect(api.getSalesPurchaseTrend).not.toHaveBeenCalled();

    click(document.querySelector('[data-testid="trend-toggle"]')!);
    await waitFor(() => !!document.querySelector('[data-testid="trend"]'));
    expect(document.querySelector('[data-testid="trend"]')!.textContent).toContain('2026-02-01');
    click(document.querySelector('[data-period="30d"]')!);
    await flush();
    click(document.querySelector('[data-period="1y"]')!);
    await flush();
    expect(api.getSalesPurchaseTrend.mock.calls.map((c) => c[0])).toEqual(['7d', '30d', '1y']);
  });
});
