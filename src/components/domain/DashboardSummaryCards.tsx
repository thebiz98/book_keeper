import React from 'react';
import { CalendarDays, Scale, Landmark, ChevronRight } from 'lucide-react';
import type { DashboardMetrics } from '@/types';
import { CurrencyDisplay } from '@/components/common/CurrencyDisplay';
import { cn } from '@/lib/utils';

/**
 * Dashboard summary (FR-32, DD-61): three grouped cards, each a list of linked rows
 * `label + caption ... amount`. Every figure comes from the backend metrics (DD-56);
 * this component only lays them out and names where each row leads.
 */

interface RowSpec {
  testId: string;
  label: string;
  caption?: string;
  value: number | null;
  route: string;
  /** The single most prominent figure of a card. */
  hero?: boolean;
  colored?: boolean;
}

interface CardSpec {
  id: string;
  title: string;
  icon: React.ReactNode;
  /** Icon chip colours (light / dark). */
  chip: string;
  rows: RowSpec[];
}

const Row: React.FC<{ row: RowSpec; onOpen: (route: string) => void }> = ({ row, onOpen }) => (
  <li>
    <button
      type="button"
      data-testid={row.testId}
      onClick={() => onOpen(row.route)}
      className={cn(
        'group w-full flex items-center justify-between gap-3 px-4 text-left cursor-pointer',
        'transition-colors hover:bg-slate-50 dark:hover:bg-slate-800/60',
        'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-blue-500',
        row.hero ? 'py-4 bg-slate-50/70 dark:bg-slate-800/30' : 'py-3'
      )}
    >
      <span className="min-w-0 flex flex-col">
        <span
          className={cn(
            'truncate',
            row.hero
              ? 'text-sm font-semibold text-slate-900 dark:text-slate-100'
              : 'text-[13px] font-medium text-slate-700 dark:text-slate-200'
          )}
        >
          {row.label}
        </span>
        {row.caption && (
          <span className="truncate text-[11px] leading-4 text-slate-500 dark:text-slate-400">{row.caption}</span>
        )}
      </span>
      <span className="flex shrink-0 items-center gap-1.5">
        {row.value === null ? (
          <span className="font-mono text-sm text-slate-400 dark:text-slate-500" aria-label="Loading">
            –
          </span>
        ) : (
          <CurrencyDisplay
            amount={row.value}
            size={row.hero ? 'xl' : 'md'}
            colored={row.colored}
            className={cn(row.hero ? 'text-2xl font-bold leading-8' : 'font-semibold')}
          />
        )}
        <ChevronRight
          aria-hidden="true"
          className="h-4 w-4 shrink-0 text-slate-300 opacity-0 transition-opacity group-hover:opacity-100 group-focus-visible:opacity-100 dark:text-slate-600"
        />
      </span>
    </button>
  </li>
);

export type SummaryGroup = 'cash' | 'today-dues' | 'all';

export const DashboardSummaryCards: React.FC<{
  metrics: DashboardMetrics | null;
  onNavigate: (route: string) => void;
  /** Which cards to draw: the full-width cash row, the Today/Dues pair, or all three. */
  group?: SummaryGroup;
}> = ({ metrics: m, onNavigate, group = 'all' }) => {
  const cards: CardSpec[] = [
    {
      id: 'today',
      title: 'Today',
      icon: <CalendarDays className="h-4 w-4" />,
      chip: 'bg-blue-50 text-blue-600 dark:bg-blue-950/60 dark:text-blue-400',
      rows: [
        {
          testId: 'kpi-sales',
          label: 'Sales',
          caption: m ? `${m.todaySalesCount} ${m.todaySalesCount === 1 ? 'bill' : 'bills'}` : undefined,
          value: m?.todaySalesAmount ?? null,
          route: 'sales-history',
        },
        {
          testId: 'kpi-purchases',
          label: 'Purchases',
          caption: m ? `${m.todayPurchaseCount} ${m.todayPurchaseCount === 1 ? 'batch' : 'batches'}` : undefined,
          value: m?.todayPurchaseAmount ?? null,
          route: 'purchase-history',
        },
      ],
    },
    {
      id: 'dues',
      title: 'Dues',
      icon: <Scale className="h-4 w-4" />,
      chip: 'bg-amber-50 text-amber-600 dark:bg-amber-950/60 dark:text-amber-400',
      rows: [
        {
          testId: 'kpi-receivables',
          label: 'Receivables',
          caption: 'customers owe you',
          value: m?.customerReceivables ?? null,
          route: 'dues',
        },
        {
          testId: 'kpi-payables',
          label: 'Payables',
          caption: 'you owe vendors',
          value: m?.vendorPayables ?? null,
          route: 'dues',
        },
      ],
    },
    {
      id: 'cash',
      title: 'Cash & Stock',
      icon: <Landmark className="h-4 w-4" />,
      chip: 'bg-emerald-50 text-emerald-600 dark:bg-emerald-950/60 dark:text-emerald-400',
      rows: [
        {
          testId: 'kpi-cash',
          label: 'Cash in hand',
          caption: 'running total, never resets',
          value: m?.cashInHand ?? null,
          route: 'reports',
          hero: true,
          colored: true,
        },
        {
          testId: 'kpi-expenses',
          label: "Today's expenses",
          value: m?.todayExpensesAmount ?? null,
          route: 'expenses',
        },
        {
          testId: 'kpi-valuation',
          label: 'Inventory valuation',
          caption: 'stock at cost',
          value: m?.totalStockValuation ?? null,
          route: 'inventory',
        },
      ],
    },
  ];

  const shown = cards.filter((c) => (group === 'all' ? true : group === 'cash' ? c.id === 'cash' : c.id !== 'cash'));
  const gridCols = group === 'cash' ? '' : group === 'today-dues' ? 'md:grid-cols-2' : 'md:grid-cols-3';

  return (
    <div className={cn('grid grid-cols-1 gap-3', gridCols)} data-testid="summary-cards">
      {shown.map((c) => (
        <section
          key={c.id}
          data-testid={`card-${c.id}`}
          aria-labelledby={`card-${c.id}-title`}
          className="flex flex-col overflow-hidden rounded-xl border border-slate-200 bg-white shadow-xs dark:border-slate-800 dark:bg-slate-900"
        >
          <header className="flex items-center gap-2.5 border-b border-slate-100 px-4 py-2.5 dark:border-slate-800">
            <span className={cn('flex h-7 w-7 items-center justify-center rounded-md', c.chip)}>{c.icon}</span>
            <h2
              id={`card-${c.id}-title`}
              className="text-[11px] font-bold uppercase tracking-wider text-slate-500 dark:text-slate-400"
            >
              {c.title}
            </h2>
          </header>
          <ul
            className={cn(
              'flex-1 divide-slate-100 dark:divide-slate-800',
              c.id === 'cash'
                ? 'grid grid-cols-1 divide-y md:grid-cols-3 md:divide-x md:divide-y-0'
                : 'flex flex-col divide-y'
            )}
          >
            {c.rows.map((r) => (
              <Row key={r.testId} row={r} onOpen={onNavigate} />
            ))}
          </ul>
        </section>
      ))}
    </div>
  );
};

export default DashboardSummaryCards;
