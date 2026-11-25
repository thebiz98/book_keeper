import React, { useEffect, useMemo, useState } from 'react';
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

// Chart geometry (viewBox units; the SVG scales to the panel width).
const W = 720;
const H = 260;
const PAD = { l: 52, r: 16, t: 12, b: 28 };
const PLOT_W = W - PAD.l - PAD.r;
const PLOT_H = H - PAD.t - PAD.b;
const MARKER_LIMIT = 31;
const MAX_X_LABELS = 8;

/** Presentation only: axis ticks for drawing (0 .. a "nice" max, ~4 steps). Not a business figure. */
export function niceTicks(max: number, target = 4): number[] {
  const safe = Math.max(1, max);
  const raw = safe / target;
  const mag = Math.pow(10, Math.floor(Math.log10(raw)));
  const norm = raw / mag;
  const step = (norm <= 1 ? 1 : norm <= 2 ? 2 : norm <= 2.5 ? 2.5 : norm <= 5 ? 5 : 10) * mag;
  const ticks: number[] = [];
  for (let v = 0; v < safe + step; v += step) {
    ticks.push(Math.round(v * 100) / 100);
    if (v >= safe) break;
  }
  return ticks;
}

/** Compact rupee label for axis ticks: 500, 12k, 1.5L. */
export function compactRupees(v: number): string {
  const f = (n: number, unit: string) => `₹${Number.isInteger(n) ? n : n.toFixed(1)}${unit}`;
  if (v >= 1e7) return f(v / 1e7, 'Cr');
  if (v >= 1e5) return f(v / 1e5, 'L');
  if (v >= 1e3) return f(v / 1e3, 'k');
  return `₹${v}`;
}

const SERIES = [
  { key: 'sales', label: 'Sales', color: 'var(--c-sales)', get: (d: TrendDataPoint) => d.salesAmount },
  { key: 'purchases', label: 'Purchases', color: 'var(--c-purch)', get: (d: TrendDataPoint) => d.purchaseAmount },
] as const;

const TrendChart: React.FC<{ data: TrendDataPoint[]; period: Period }> = ({ data, period }) => {
  const [active, setActive] = useState<number | null>(null);
  const n = data.length;
  const label = (d: TrendDataPoint) => (period === '1y' ? d.date.slice(0, 7) : d.date);

  const { ticks, yMax } = useMemo(() => {
    const t = niceTicks(Math.max(0, ...data.map((d) => Math.max(d.salesAmount, d.purchaseAmount))));
    return { ticks: t, yMax: t[t.length - 1] };
  }, [data]);

  const x = (i: number) => PAD.l + ((i + 0.5) * PLOT_W) / n;
  const y = (v: number) => PAD.t + PLOT_H - (v / yMax) * PLOT_H;
  const labelStep = Math.ceil(n / MAX_X_LABELS);
  const showMarkers = n <= MARKER_LIMIT;

  const summary = `Sales and purchases for ${n} ${period === '1y' ? 'months' : 'days'}, from ${label(data[0])} to ${label(
    data[n - 1]
  )}`;

  const onKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'ArrowRight') {
      e.preventDefault();
      setActive((a) => (a === null ? 0 : Math.min(n - 1, a + 1)));
    } else if (e.key === 'ArrowLeft') {
      e.preventDefault();
      setActive((a) => (a === null ? n - 1 : Math.max(0, a - 1)));
    } else if (e.key === 'Escape' && active !== null) {
      setActive(null);
    }
  };

  const tip = active !== null ? data[active] : null;
  const tipLeft = active !== null ? (x(active) / W) * 100 : 0;

  return (
    <div
      tabIndex={0}
      onKeyDown={onKeyDown}
      onBlur={() => setActive(null)}
      onMouseLeave={() => setActive(null)}
      className="relative rounded-md focus:outline-none focus-visible:ring-2 focus-visible:ring-blue-500"
    >
      <svg
        viewBox={`0 0 ${W} ${H}`}
        role="img"
        aria-label={summary}
        className="block w-full h-auto select-none"
        data-testid="trend-svg"
      >
        {ticks.map((t) => (
          <g key={t} data-testid="trend-ytick">
            <line
              x1={PAD.l}
              x2={W - PAD.r}
              y1={y(t)}
              y2={y(t)}
              strokeWidth={1}
              className="stroke-slate-200 dark:stroke-slate-800"
            />
            <text
              x={PAD.l - 8}
              y={y(t)}
              textAnchor="end"
              dominantBaseline="middle"
              className="fill-slate-500 dark:fill-slate-400"
              fontSize={11}
            >
              {compactRupees(t)}
            </text>
          </g>
        ))}
        {data.map(
          (d, i) =>
            i % labelStep === 0 && (
              <text
                key={d.date}
                x={x(i)}
                y={H - 8}
                textAnchor="middle"
                className="fill-slate-500 dark:fill-slate-400"
                fontSize={11}
              >
                {label(d)}
              </text>
            )
        )}
        {active !== null && (
          <line
            data-testid="trend-crosshair"
            x1={x(active)}
            x2={x(active)}
            y1={PAD.t}
            y2={PAD.t + PLOT_H}
            strokeWidth={1}
            className="stroke-slate-400 dark:stroke-slate-500"
          />
        )}
        {SERIES.map((s) => (
          <path
            key={s.key}
            data-series={s.key}
            d={data.map((d, i) => `${i === 0 ? 'M' : 'L'}${x(i).toFixed(1)} ${y(s.get(d)).toFixed(1)}`).join(' ')}
            fill="none"
            stroke={s.color}
            strokeWidth={2}
            strokeLinejoin="round"
            strokeLinecap="round"
          />
        ))}
        {SERIES.map((s) =>
          data.map(
            (d, i) =>
              (showMarkers || i === active) && (
                <circle
                  key={`${s.key}-${d.date}`}
                  data-marker={s.key}
                  cx={x(i)}
                  cy={y(s.get(d))}
                  r={4}
                  fill={s.color}
                  strokeWidth={2}
                  className="stroke-white dark:stroke-slate-900"
                />
              )
          )
        )}
        {data.map((d, i) => (
          <rect
            key={d.date}
            data-testid="trend-hit"
            x={PAD.l + (i * PLOT_W) / n}
            y={PAD.t}
            width={PLOT_W / n}
            height={PLOT_H}
            fill="transparent"
            onMouseEnter={() => setActive(i)}
            onMouseMove={() => setActive(i)}
          />
        ))}
      </svg>
      {tip && (
        <div
          data-testid="trend-tooltip"
          role="status"
          style={{ left: `${tipLeft}%` }}
          className={`pointer-events-none absolute top-2 z-10 -translate-x-1/2 rounded-md border border-slate-200 bg-white px-2.5 py-1.5 text-[11px] shadow-md dark:border-slate-700 dark:bg-slate-900`}
        >
          <div className="mb-1 font-mono text-slate-500">{label(tip)}</div>
          {SERIES.map((s) => (
            <div key={s.key} className="flex items-center justify-between gap-4">
              <span className="flex items-center gap-1.5 text-slate-700 dark:text-slate-200">
                <span className="inline-block h-2 w-2 rounded-full" style={{ background: s.color }} />
                {s.label}
              </span>
              <CurrencyDisplay amount={s.get(tip)} size="sm" />
            </div>
          ))}
        </div>
      )}
    </div>
  );
};

const TrendTable: React.FC<{ data: TrendDataPoint[]; period: Period }> = ({ data, period }) => (
  <div className="max-h-[320px] overflow-y-auto">
    <table className="w-full text-xs" data-testid="trend-table">
      <thead className="sticky top-0 bg-white text-left text-slate-400 dark:bg-slate-900">
        <tr>
          <th className="py-1.5 font-medium">{period === '1y' ? 'Month' : 'Date'}</th>
          <th className="py-1.5 text-right font-medium">Sales</th>
          <th className="py-1.5 text-right font-medium">Purchases</th>
        </tr>
      </thead>
      <tbody>
        {data.map((d) => (
          <tr key={d.date} className="border-t border-slate-100 dark:border-slate-800">
            <td className="py-1 font-mono text-slate-500">{period === '1y' ? d.date.slice(0, 7) : d.date}</td>
            <td className="py-1 text-right">
              <CurrencyDisplay amount={d.salesAmount} size="sm" />
            </td>
            <td className="py-1 text-right">
              <CurrencyDisplay amount={d.purchaseAmount} size="sm" />
            </td>
          </tr>
        ))}
      </tbody>
    </table>
  </div>
);

/**
 * Sales vs Purchases per day (per month for 1Y), from `get_sales_purchase_trend`. Lives on
 * Financial Reports. Hand-built SVG line chart (no chart library) with a Table view as the
 * non-colour alternative. Fetches only while mounted.
 */
export const SalesPurchaseTrendPanel: React.FC = () => {
  const [period, setPeriod] = useState<Period>('7d');
  const [view, setView] = useState<'chart' | 'table'>('chart');
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
  const data = current?.data ?? null;

  const segBtn = (active: boolean) =>
    `px-2 py-0.5 rounded font-medium cursor-pointer ${
      active ? 'bg-white dark:bg-slate-900 text-blue-600 shadow-2xs' : 'text-slate-600 dark:text-slate-400'
    }`;
  const segWrap =
    'flex rounded-md border border-slate-300 dark:border-slate-700 p-0.5 bg-slate-100 dark:bg-slate-800 text-xs';

  return (
    <div
      data-testid="trend-panel"
      className="[--c-sales:#257de0] [--c-purch:#f47230] dark:[--c-sales:#338ced] dark:[--c-purch:#e76624]"
    >
      <div className="flex flex-wrap items-center justify-between gap-2 mb-3">
        <div className="flex items-center gap-4 text-[11px] text-slate-600 dark:text-slate-300">
          {SERIES.map((s) => (
            <span key={s.key} className="flex items-center gap-1.5">
              <span className="inline-block h-2.5 w-2.5 rounded-full" style={{ background: s.color }} />
              {s.label}
            </span>
          ))}
        </div>
        <div className="flex items-center gap-2">
          <div className={segWrap}>
            {(['chart', 'table'] as const).map((v) => (
              <button
                key={v}
                type="button"
                data-view={v}
                aria-pressed={view === v}
                onClick={() => setView(v)}
                className={segBtn(view === v)}
              >
                {v === 'chart' ? 'Chart' : 'Table'}
              </button>
            ))}
          </div>
          <div className={segWrap}>
            {PERIODS.map((p) => (
              <button
                key={p.id}
                type="button"
                data-period={p.id}
                aria-pressed={period === p.id}
                onClick={() => setPeriod(p.id)}
                className={segBtn(period === p.id)}
              >
                {p.label}
              </button>
            ))}
          </div>
        </div>
      </div>
      {current?.error ? (
        <div role="alert" className="text-xs text-rose-600 dark:text-rose-400 flex items-center gap-2">
          <span>Could not load trend: {current.error}</span>
          <button type="button" className="underline cursor-pointer" onClick={() => setTick((t) => t + 1)}>
            Retry
          </button>
        </div>
      ) : !data ? (
        <p className="py-6 text-center text-xs text-slate-400">Loading...</p>
      ) : data.length === 0 ? (
        <p className="py-6 text-center text-xs text-slate-400" data-testid="trend">
          No sales or purchases in this period.
        </p>
      ) : (
        <div data-testid="trend">
          {view === 'chart' ? <TrendChart data={data} period={period} /> : <TrendTable data={data} period={period} />}
        </div>
      )}
    </div>
  );
};

export default SalesPurchaseTrendPanel;
