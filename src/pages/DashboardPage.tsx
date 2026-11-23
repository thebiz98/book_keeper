import React, { useCallback, useEffect, useRef, useState } from 'react';
import { ShoppingCart, Truck, BookOpen, SlidersHorizontal, AlertTriangle, ArrowUpRight } from 'lucide-react';
import { api, errorMessage } from '@/api';
import type { DashboardMetrics, LowStockItem, RecentTransaction, TrendDataPoint } from '@/types';
import { useAppStore } from '@/store/useAppStore';
import { CurrencyDisplay } from '@/components/common/CurrencyDisplay';
import { StockStatusBadge } from '@/components/common/StockStatusBadge';
import { PaymentStatusBadge } from '@/components/common/PaymentStatusBadge';
import { Card, CardContent } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/Table';
import { StockAdjustmentModal } from '@/components/domain/StockAdjustmentModal';
import { BillDetailDrawer } from '@/components/domain/BillDetailDrawer';
import { navigateWithParam } from '@/components/domain/navParams';

type Period = '7d' | '30d' | '1y';
const PERIODS: { id: Period; label: string }[] = [
  { id: '7d', label: '7D' },
  { id: '30d', label: '30D' },
  { id: '1y', label: '1Y' },
];

interface Section<T> {
  data: T | null;
  error: string | null;
}

interface DashData {
  metrics: Section<DashboardMetrics>;
  lowStock: Section<LowStockItem[]>;
  recent: Section<RecentTransaction[]>;
  trend: Section<TrendDataPoint[]>;
}

/** Simple SWR: last successful result per period is shown immediately while a refetch runs. */
const cache: { base: Omit<DashData, 'trend'> | null; trend: Partial<Record<Period, Section<TrendDataPoint[]>>> } = {
  base: null,
  trend: {},
};

/** Test hook: drop the in-memory cache. */
export function resetDashboardCache(): void {
  cache.base = null;
  cache.trend = {};
}

async function settle<T>(p: Promise<T>): Promise<Section<T>> {
  try {
    return { data: await p, error: null };
  } catch (e) {
    return { data: null, error: errorMessage(e) };
  }
}

const Err: React.FC<{ msg: string; onRetry: () => void }> = ({ msg, onRetry }) => (
  <div role="alert" className="text-xs text-rose-600 dark:text-rose-400 flex items-center gap-2">
    <span>{msg}</span>
    <button type="button" className="underline cursor-pointer" onClick={onRetry}>
      Retry
    </button>
  </div>
);

const Kpi: React.FC<{ label: string; value: number | null; sub?: string; colored?: boolean; testId: string }> = ({
  label,
  value,
  sub,
  colored,
  testId,
}) => (
  <Card>
    <CardContent className="p-3 flex flex-col justify-between h-full" data-testid={testId}>
      <span className="text-[11px] font-medium text-slate-500 uppercase tracking-wider">{label}</span>
      <div className="text-lg font-bold font-mono my-1">
        {value === null ? <span className="text-slate-400">-</span> : <CurrencyDisplay amount={value} size="lg" colored={colored} />}
      </div>
      {sub && <span className="text-[10px] text-slate-500 font-mono">{sub}</span>}
    </CardContent>
  </Card>
);

export const DashboardPage: React.FC = () => {
  const setActiveRoute = useAppStore((s) => s.setActiveRoute);
  const [period, setPeriod] = useState<Period>('7d');
  const [base, setBase] = useState(cache.base);
  const [trend, setTrend] = useState<Section<TrendDataPoint[]> | null>(cache.trend['7d'] ?? null);
  const [adjustOpen, setAdjustOpen] = useState(false);
  const [detail, setDetail] = useState<{ kind: 'SALE' | 'PURCHASE'; id: number } | null>(null);
  const alive = useRef(true);

  const loadBase = useCallback(async () => {
    const [metrics, lowStock, recent] = await Promise.all([
      settle(api.getDashboardMetrics()),
      settle(api.getLowStockAlerts()),
      settle(api.getRecentTransactions(10)),
    ]);
    const next = { metrics, lowStock, recent };
    cache.base = next;
    if (alive.current) setBase(next);
  }, []);

  const loadTrend = useCallback(async (p: Period) => {
    const sec = await settle(api.getSalesPurchaseTrend(p));
    cache.trend[p] = sec;
    if (alive.current) setTrend((cur) => (periodRef.current === p ? sec : cur));
  }, []);

  const periodRef = useRef<Period>(period);
  periodRef.current = period;

  useEffect(() => {
    alive.current = true;
    void loadBase();
    const onFocus = () => {
      void loadBase();
      void loadTrend(periodRef.current);
    };
    window.addEventListener('focus', onFocus);
    return () => {
      alive.current = false;
      window.removeEventListener('focus', onFocus);
    };
  }, [loadBase, loadTrend]);

  useEffect(() => {
    setTrend(cache.trend[period] ?? null);
    void loadTrend(period);
  }, [period, loadTrend]);

  const m = base?.metrics.data ?? null;
  const low = base?.lowStock.data ?? null;
  const maxAmt = Math.max(1, ...(trend?.data ?? []).map((t) => Math.max(t.salesAmount, t.purchaseAmount)));

  return (
    <div className="flex flex-col gap-4">
      <div className="flex flex-wrap items-center justify-between gap-3 bg-white dark:bg-slate-900 p-3.5 rounded-lg border border-slate-200 dark:border-slate-800 shadow-xs">
        <div>
          <h1 className="text-base font-bold text-slate-900 dark:text-slate-100">Retail Counter Cockpit</h1>
          <p className="text-xs text-slate-500">Daily turnover, cash drawer and inventory health</p>
        </div>
        <div className="flex items-center gap-2">
          <Button size="sm" onClick={() => setActiveRoute('sale')} hotkey="F2" leftIcon={<ShoppingCart className="h-3.5 w-3.5" />}>
            New Sale
          </Button>
          <Button variant="secondary" size="sm" onClick={() => setActiveRoute('purchase')} hotkey="F3" leftIcon={<Truck className="h-3.5 w-3.5" />}>
            New Purchase
          </Button>
          <Button variant="outline" size="sm" onClick={() => setActiveRoute('dues')} hotkey="F5" leftIcon={<BookOpen className="h-3.5 w-3.5" />}>
            Settle Due
          </Button>
          <Button variant="outline" size="sm" onClick={() => setAdjustOpen(true)} leftIcon={<SlidersHorizontal className="h-3.5 w-3.5" />}>
            Stock Adjustment
          </Button>
        </div>
      </div>

      {base?.metrics.error && <Err msg={`Could not load metrics: ${base.metrics.error}`} onRetry={loadBase} />}
      <div className="grid grid-cols-2 md:grid-cols-4 lg:grid-cols-7 gap-3">
        <Kpi testId="kpi-sales" label="Today's Gross Sales" value={m?.todaySalesAmount ?? null} sub={m ? `${m.todaySalesCount} bills` : undefined} />
        <Kpi testId="kpi-purchases" label="Today's Purchases" value={m?.todayPurchaseAmount ?? null} sub={m ? `${m.todayPurchaseCount} batches` : undefined} />
        <Kpi testId="kpi-profit" label="Today's Net Profit" value={m?.todayNetProfit ?? null} colored />
        <Kpi testId="kpi-valuation" label="Inventory Valuation" value={m?.totalStockValuation ?? null} />
        <Kpi testId="kpi-receivables" label="Customer Receivables" value={m?.customerReceivables ?? null} />
        <Kpi testId="kpi-payables" label="Vendor Payables" value={m?.vendorPayables ?? null} />
        <Kpi testId="kpi-cash" label="Cash in Drawer" value={m?.liveCashInDrawer ?? null} colored sub="Live physical cash" />
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-3 gap-4">
        <div className="lg:col-span-2 flex flex-col gap-4">
          <div className="p-4 rounded-lg bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 shadow-xs">
            <div className="flex items-center justify-between mb-3">
              <h2 className="text-xs font-bold uppercase tracking-wider text-slate-700 dark:text-slate-300">
                Sales vs Purchases
              </h2>
              <div className="flex rounded-md border border-slate-300 dark:border-slate-700 p-0.5 bg-slate-100 dark:bg-slate-800 text-xs">
                {PERIODS.map((p) => (
                  <button
                    key={p.id}
                    type="button"
                    data-period={p.id}
                    aria-pressed={period === p.id}
                    onClick={() => setPeriod(p.id)}
                    className={`px-2 py-0.5 rounded font-medium cursor-pointer ${
                      period === p.id ? 'bg-white dark:bg-slate-900 text-blue-600 shadow-2xs' : 'text-slate-600 dark:text-slate-400'
                    }`}
                  >
                    {p.label}
                  </button>
                ))}
              </div>
            </div>
            {trend?.error ? (
              <Err msg={`Could not load trend: ${trend.error}`} onRetry={() => loadTrend(period)} />
            ) : !trend?.data ? (
              <p className="py-6 text-center text-xs text-slate-400">Loading...</p>
            ) : (
              <div className="flex flex-col gap-1.5" data-testid="trend">
                {trend.data.map((dp) => (
                  <div key={dp.date} className="flex items-center gap-3 text-xs">
                    <span className="w-20 font-mono text-slate-500 text-[11px] shrink-0">
                      {period === '1y' ? dp.date.slice(0, 7) : dp.date}
                    </span>
                    <div className="flex-1 flex flex-col gap-0.5">
                      <div
                        style={{ width: `${Math.max(1, (dp.salesAmount / maxAmt) * 100)}%` }}
                        className="h-1.5 rounded bg-blue-600"
                        title={`Sales: ₹${dp.salesAmount}`}
                      />
                      <div
                        style={{ width: `${Math.max(1, (dp.purchaseAmount / maxAmt) * 100)}%` }}
                        className="h-1.5 rounded bg-amber-500"
                        title={`Purchases: ₹${dp.purchaseAmount}`}
                      />
                    </div>
                    <span className="w-24 text-right font-mono text-[10px]">
                      <CurrencyDisplay amount={dp.salesAmount} size="sm" />
                    </span>
                  </div>
                ))}
              </div>
            )}
          </div>

          <div className="p-4 rounded-lg bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 shadow-xs">
            <div className="flex items-center justify-between mb-3">
              <h2 className="text-xs font-bold uppercase tracking-wider text-slate-700 dark:text-slate-300">
                Recent Transactions
              </h2>
              <button
                type="button"
                onClick={() => setActiveRoute('sales-history')}
                className="text-xs font-medium text-blue-600 hover:underline flex items-center gap-1 cursor-pointer"
              >
                <span>View Full Log</span>
                <ArrowUpRight className="h-3 w-3" />
              </button>
            </div>
            {base?.recent.error ? (
              <Err msg={`Could not load recent transactions: ${base.recent.error}`} onRetry={loadBase} />
            ) : !base?.recent.data ? (
              <p className="py-4 text-center text-xs text-slate-400">Loading...</p>
            ) : base.recent.data.length === 0 ? (
              <p className="py-4 text-center text-xs text-slate-400">No transactions yet.</p>
            ) : (
              <Table dense scroll="bounded" containerClassName="max-h-[420px]" data-testid="recent">
                <TableHeader>
                  <TableRow className="border-b border-slate-100 dark:border-slate-800 text-slate-400 select-none">
                    <TableHead className="py-2">Time</TableHead>
                    <TableHead>Type</TableHead>
                    <TableHead>Code</TableHead>
                    <TableHead>Party</TableHead>
                    <TableHead className="text-right">Amount</TableHead>
                    <TableHead className="text-center">Status</TableHead>
                    <TableHead />
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {base.recent.data.map((t) => (
                    <TableRow key={`${t.kind}-${t.id}`} className="border-b border-slate-50 dark:border-slate-800/60">
                      <TableCell className="py-1.5 font-mono text-slate-500">
                        {t.date}
                        {t.createdAt ? ` ${t.createdAt.slice(11, 16)}` : ''}
                      </TableCell>
                      <TableCell>{t.kind}</TableCell>
                      <TableCell className="font-mono font-semibold">{t.code}</TableCell>
                      <TableCell>{t.partyName ?? '-'}</TableCell>
                      <TableCell className="text-right">
                        <CurrencyDisplay amount={t.amount} size="sm" />
                      </TableCell>
                      <TableCell className="text-center">{t.paymentStatus ? <PaymentStatusBadge status={t.paymentStatus} /> : null}</TableCell>
                      <TableCell className="text-right">
                        <button
                          type="button"
                          className="text-blue-600 hover:underline cursor-pointer"
                          onClick={() =>
                            t.kind === 'RETURN' ? setActiveRoute('returns') : setDetail({ kind: t.kind, id: t.id })
                          }
                        >
                          Open
                        </button>
                      </TableCell>
                    </TableRow>
                  ))}
                </TableBody>
              </Table>
            )}
          </div>
        </div>

        <div className="flex flex-col p-4 rounded-lg bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 shadow-xs">
          <div className="flex items-center justify-between mb-3">
            <div className="flex items-center gap-1.5">
              <AlertTriangle className="h-4 w-4 text-amber-500" />
              <h2 className="text-xs font-bold uppercase tracking-wider text-slate-700 dark:text-slate-300">
                Low Stock Monitor
              </h2>
            </div>
            {low && (
              <span data-testid="low-count" className="text-[10px] font-mono text-amber-600 bg-amber-50 dark:bg-amber-950/60 px-1.5 py-0.5 rounded font-bold">
                {low.length}
              </span>
            )}
          </div>
          {base?.lowStock.error ? (
            <Err msg={`Could not load low-stock items: ${base.lowStock.error}`} onRetry={loadBase} />
          ) : !low ? (
            <p className="py-8 text-center text-xs text-slate-400">Loading...</p>
          ) : low.length === 0 ? (
            <p className="py-8 text-center text-xs text-slate-400">No items below their low stock level.</p>
          ) : (
            <div className="flex flex-col divide-y divide-slate-100 dark:divide-slate-800 overflow-y-auto max-h-[520px]">
              {low.map((item) => (
                <div key={item.id} className="py-2.5 flex items-center justify-between gap-2">
                  <div className="flex flex-col truncate">
                    <span className="text-xs font-semibold truncate">{item.name}</span>
                    <span className="text-[10px] text-slate-400">{item.category ?? 'Uncategorized'}</span>
                  </div>
                  <div className="flex items-center gap-2 shrink-0">
                    <span className="text-[10px] text-slate-400 tabular-nums">
                      {item.currentStock} / {item.lowStockThreshold} {item.baseUnit}
                    </span>
                    <StockStatusBadge
                      currentStock={item.currentStock}
                      isLowStock
                      lowStockThreshold={item.lowStockThreshold}
                      baseUnit={item.baseUnit}
                    />
                    <button
                      type="button"
                      onClick={() => navigateWithParam('purchase', 'inwardItem', String(item.id))}
                      title="Record inward purchase for this item (F3)"
                      className="px-2 py-0.5 rounded text-[10px] font-semibold bg-blue-50 text-blue-700 dark:bg-blue-950 dark:text-blue-300 hover:bg-blue-100 cursor-pointer"
                    >
                      Inward (F3)
                    </button>
                  </div>
                </div>
              ))}
            </div>
          )}
        </div>
      </div>

      <StockAdjustmentModal
        isOpen={adjustOpen}
        onClose={() => setAdjustOpen(false)}
        onAdjusted={() => void loadBase()}
      />
      <BillDetailDrawer kind={detail?.kind ?? 'SALE'} billId={detail?.id ?? null} onClose={() => setDetail(null)} />
    </div>
  );
};

export default DashboardPage;
