import React, { useCallback, useEffect, useState } from 'react';
import { BarChart3 } from 'lucide-react';
import { api, errorMessage } from '@/api';
import { startOfMonth, todayLocal } from '@/lib/dates';
import type { CashFlowReport, PnLReport, StockValuationReport } from '@/types';
import { CurrencyDisplay } from '@/components/common/CurrencyDisplay';
import { DateRangePicker } from '@/components/common/DateRangePicker';
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/Table';

interface Section<T> {
  data: T | null;
  error: string | null;
  loading: boolean;
}
const empty = <T,>(): Section<T> => ({ data: null, error: null, loading: true });

function useReport<T>(fetcher: () => Promise<T>, deps: unknown[]): [Section<T>, () => void] {
  const [state, setState] = useState<Section<T>>(empty<T>());
  const [tick, setTick] = useState(0);
  useEffect(() => {
    let live = true;
    setState({ data: null, error: null, loading: true });
    fetcher()
      .then((data) => live && setState({ data, error: null, loading: false }))
      .catch((e) => live && setState({ data: null, error: errorMessage(e), loading: false }));
    return () => {
      live = false;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [...deps, tick]);
  return [state, () => setTick((t) => t + 1)];
}

const Line: React.FC<{ label: string; amount: number; strong?: boolean; testId?: string; negative?: boolean }> = ({
  label,
  amount,
  strong,
  testId,
  negative,
}) => (
  <div
    data-testid={testId}
    className={`flex items-center justify-between py-1.5 text-sm ${strong ? 'font-bold ' : ''}`}
  >
    <span className={strong ? '' : 'text-slate-600 dark:text-slate-300'}>{label}</span>
    <span className="font-mono">
      {negative ? '- ' : ''}
      <CurrencyDisplay amount={amount} colored={strong} />
    </span>
  </div>
);

const Panel: React.FC<{ title: string; state: Section<unknown>; onRetry: () => void; children: React.ReactNode }> = ({
  title,
  state,
  onRetry,
  children,
}) => (
  <section className="p-4 rounded-lg bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 shadow-xs">
    <h2 className="text-xs font-bold uppercase tracking-wider text-slate-700 dark:text-slate-300 mb-2">{title}</h2>
    {state.loading ? (
      <p className="text-xs text-slate-400 py-4">Loading...</p>
    ) : state.error ? (
      <div role="alert" className="text-xs text-rose-600 dark:text-rose-400 py-2">
        {state.error}{' '}
        <button type="button" onClick={onRetry} className="underline cursor-pointer">
          Retry
        </button>
      </div>
    ) : (
      children
    )}
  </section>
);

export const ReportsPage: React.FC = () => {
  const [range, setRange] = useState({ startDate: startOfMonth(), endDate: todayLocal() });
  const [pnl, retryPnl] = useReport<PnLReport>(() => api.getPnLReport(range.startDate, range.endDate), [range.startDate, range.endDate]);
  const [cash, retryCash] = useReport<CashFlowReport>(() => api.getCashFlowReport(range.startDate, range.endDate), [range.startDate, range.endDate]);
  const [val, retryVal] = useReport<StockValuationReport>(() => api.getStockValuationReport(), []);
  const onRange = useCallback((r: { startDate: string; endDate: string }) => setRange(r), []);

  const p = pnl.data;
  const c = cash.data;
  const inflow = c?.totalInflow ?? 0;
  const outflow = c?.totalOutflow ?? 0;

  return (
    <div className="flex flex-col gap-4">
      <div className="flex flex-wrap items-center justify-between gap-3 bg-white dark:bg-slate-900 p-4 rounded-lg border border-slate-200 dark:border-slate-800 shadow-xs">
        <h1 className="text-xl font-bold flex items-center gap-2">
          <BarChart3 className="text-blue-600 dark:text-blue-400" size={24} />
          Financial Reports
        </h1>
        <DateRangePicker startDate={range.startDate} endDate={range.endDate} onChange={onRange} />
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-2 gap-4">
        <Panel title="Profit & Loss" state={pnl} onRetry={retryPnl}>
          {p && (
            <div data-testid="pnl">
              <Line testId="pnl-gross-sales" label="Gross Sales" amount={p.grossSales} />
              <Line testId="pnl-sales-discounts" label="Less: Sales Discounts" amount={p.salesDiscounts} negative />
              <Line testId="pnl-sales-returns" label="Less: Sales Returns" amount={p.salesReturns} negative />
              <Line testId="pnl-net-sales" label="Net Sales" amount={p.netSales} strong />
              <Line testId="pnl-cogs" label="Less: FIFO COGS (net of returns)" amount={p.cogs} negative />
              <Line testId="pnl-gross-profit" label="Gross Profit" amount={p.grossProfit} strong />
              <Line testId="pnl-other-income" label="Add: Other Income" amount={p.otherIncome} />
              {p.otherIncome > 0 && (
                <ul className="ml-4 mb-1 text-xs text-slate-500">
                  <li data-testid="pnl-sale-charges" className="flex items-center justify-between py-0.5">
                    <span>Charges collected on sales</span>
                    <CurrencyDisplay amount={p.saleChargesIncome} size="sm" />
                  </li>
                  <li data-testid="pnl-purchase-discounts" className="flex items-center justify-between py-0.5">
                    <span>Discounts received on purchases</span>
                    <CurrencyDisplay amount={p.purchaseDiscountsIncome} size="sm" />
                  </li>
                </ul>
              )}
              <Line testId="pnl-losses" label="Less: Inventory Losses" amount={p.inventoryLosses} negative />
              <Line testId="pnl-opex" label="Less: Operating Expenses" amount={p.operatingExpenses} negative />
              {p.purchaseChargesExpense > 0 && (
                <ul className="ml-4 mb-1 text-xs text-slate-500">
                  <li data-testid="pnl-purchase-charges" className="flex items-center justify-between py-0.5">
                    <span>Purchase charges paid</span>
                    <CurrencyDisplay amount={p.purchaseChargesExpense} size="sm" />
                  </li>
                </ul>
              )}
              {p.expensesByType.length > 0 && (
                <ul data-opex-breakdown className="ml-4 mb-1 text-xs text-slate-500">
                  {p.expensesByType.map((t) => (
                    <li key={t.expenseTypeId} data-breakdown className="flex items-center justify-between py-0.5">
                      <span>{t.expenseTypeName}</span>
                      <span className="font-mono">
                        <CurrencyDisplay amount={t.amount} size="sm" />
                      </span>
                    </li>
                  ))}
                </ul>
              )}
              <Line testId="pnl-net-profit" label="Net Profit" amount={p.netProfit} strong />
              <div className="mt-2 pt-2 border-t border-dashed border-slate-200 dark:border-slate-700">
                <Line testId="pnl-purchases" label="Total Purchases (net of vendor returns)" amount={p.totalPurchases} />
              </div>
            </div>
          )}
        </Panel>

        <Panel title="Cash Flow Register" state={cash} onRetry={retryCash}>
          {c && (
            <div data-testid="cashflow">
              <p className="text-[11px] font-semibold uppercase text-slate-400">Inflows</p>
              <Line testId="cf-sales" label="Sale upfront cash" amount={c.salesCashInflow} />
              <Line testId="cf-cust" label="Customer settlements" amount={c.customerSettlementsInflow} />
              <Line testId="cf-vref" label="Vendor refunds received" amount={c.vendorRefundsInflow} />
              <Line testId="cf-total-inflow" label="Total inflow" amount={inflow} strong />
              <p className="text-[11px] font-semibold uppercase text-slate-400 mt-2">Outflows</p>
              <Line testId="cf-purch" label="Purchase upfront cash" amount={c.purchaseCashOutflow} />
              <Line testId="cf-vset" label="Vendor settlements" amount={c.vendorSettlementsOutflow} />
              <Line testId="cf-refunds" label="Customer refunds paid" amount={c.refundsCashOutflow} />
              <Line testId="cf-expenses" label="Store expenses" amount={c.expensesCashOutflow} />
              <Line testId="cf-total-outflow" label="Total outflow" amount={outflow} strong />
              <Line testId="cf-net" label="Net Cash Balance" amount={c.netCashBalance} strong />
            </div>
          )}
        </Panel>
      </div>

      <Panel title="FIFO Inventory Valuation (current)" state={val} onRetry={retryVal}>
        {val.data && (
          <div data-testid="valuation">
            <div className="flex gap-6 text-sm mb-3">
              <span>
                Total Stock Valuation: <CurrencyDisplay amount={val.data.totalValuation} colored />
              </span>
              <span>Active lots: {val.data.activeLotsCount}</span>
            </div>
            {val.data.categoriesBreakdown.length === 0 ? (
              <p className="text-xs text-slate-500">No stock on hand.</p>
            ) : (
              <Table dense scroll="bounded">
                <TableHeader>
                  <TableRow className="text-left text-slate-400">
                    <TableHead className="py-1">Category</TableHead>
                    <TableHead className="text-right">Items</TableHead>
                    <TableHead className="text-right">Valuation</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {val.data.categoriesBreakdown.map((cat) => (
                    <TableRow key={cat.category} >
                      <TableCell className="py-1">{cat.category}</TableCell>
                      <TableCell className="text-right font-mono">{cat.itemsCount}</TableCell>
                      <TableCell className="text-right">
                        <CurrencyDisplay amount={cat.valuation} size="sm" />
                      </TableCell>
                    </TableRow>
                  ))}
                </TableBody>
              </Table>
            )}
          </div>
        )}
      </Panel>
    </div>
  );
};

export default ReportsPage;
