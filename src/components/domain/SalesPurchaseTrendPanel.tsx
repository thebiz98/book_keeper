import React, { useEffect, useState } from 'react';
import { api, errorMessage } from '@/api';
import type { TrendDataPoint } from '@/types';
import { CurrencyDisplay } from '@/components/common/CurrencyDisplay';

type Period = '7d' | '30d' | '1y';
const PERIODS: { id: Period; label: string }[] = [
  { id: '7d', label: '7D' },
  { id: '30d', label: '30D' },
  { id: '1y', label: '1Y' },
];

interface TrendState {
  period: Period;
  data: TrendDataPoint[] | null;
  error: string | null;
}

/**
 * Sales vs Purchases per day (per month for 1Y), from `get_sales_purchase_trend`. Lives on
 * Financial Reports (moved off the dashboard, owner 2026-11-25). Fetches only while mounted.
 */
export const SalesPurchaseTrendPanel: React.FC = () => {
  const [period, setPeriod] = useState<Period>('7d');
  const [state, setState] = useState<TrendState | null>(null);
  const [tick, setTick] = useState(0);

  useEffect(() => {
    let live = true;
    api.getSalesPurchaseTrend(period).then(
      (data) => live && setState({ period, data, error: null }),
      (e: unknown) => live && setState({ period, data: null, error: errorMessage(e) })
    );
    return () => {
      live = false;
    };
  }, [period, tick]);

  const current = state?.period === period ? state : null;
  const maxAmt = Math.max(1, ...(current?.data ?? []).map((t) => Math.max(t.salesAmount, t.purchaseAmount)));

  return (
    <div data-testid="trend-panel">
      <div className="flex flex-wrap items-center justify-between gap-2 mb-3">
        <div className="flex items-center gap-3 text-[11px] text-slate-500">
          <span className="flex items-center gap-1">
            <span className="inline-block h-1.5 w-3 rounded bg-blue-600" /> Sales
          </span>
          <span className="flex items-center gap-1">
            <span className="inline-block h-1.5 w-3 rounded bg-amber-500" /> Purchases
          </span>
        </div>
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
      {current?.error ? (
        <div role="alert" className="text-xs text-rose-600 dark:text-rose-400 flex items-center gap-2">
          <span>Could not load trend: {current.error}</span>
          <button type="button" className="underline cursor-pointer" onClick={() => setTick((t) => t + 1)}>
            Retry
          </button>
        </div>
      ) : !current?.data ? (
        <p className="py-6 text-center text-xs text-slate-400">Loading...</p>
      ) : (
        <div className="flex flex-col gap-1.5 max-h-[420px] overflow-y-auto" data-testid="trend">
          {current.data.map((dp) => (
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
              <span className="w-24 text-right font-mono text-[10px] flex flex-col">
                <CurrencyDisplay amount={dp.salesAmount} size="sm" />
                <span className="text-amber-600 dark:text-amber-400">
                  <CurrencyDisplay amount={dp.purchaseAmount} size="sm" />
                </span>
              </span>
            </div>
          ))}
        </div>
      )}
    </div>
  );
};

export default SalesPurchaseTrendPanel;
