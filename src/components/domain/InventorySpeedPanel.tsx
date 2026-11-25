import React, { useState } from 'react';
import { api, errorMessage } from '@/api';
import { useLiveSearch, LIST_SEARCH_DEBOUNCE_MS } from '@/hooks/useLiveSearch';
import { addDays, todayLocal } from '@/lib/dates';
import type { InventorySpeedReport, InventorySpeedRow, ReportSort, SpeedClass } from '@/types';
import { DateRangePicker } from '../common/DateRangePicker';
import { Badge } from '../ui/Badge';
import { Table, TableBody, TableCell, TableHeader, TableRow } from '../ui/Table';
import { Tabs } from '../ui/Tabs';
import { Pager } from './Pager';
import {
  nextSort,
  REPORT_PAGE_SIZE,
  ReportSearch,
  SortableHead,
  useReportItemDrawers,
} from './ReportListParts';

const EMPTY: InventorySpeedReport = {
  rows: [],
  counts: { fast: 0, normal: 0, slow: 0, dead: 0 },
  days: 1,
  totalCount: 0,
  page: 1,
  pageSize: REPORT_PAGE_SIZE,
};

type Tab = 'ALL' | 'FAST' | 'SLOW' | 'DEAD';

const BADGE: Record<SpeedClass, { label: string; variant: 'success' | 'default' | 'warning' | 'danger' }> = {
  FAST: { label: 'Fast', variant: 'success' },
  NORMAL: { label: 'Normal', variant: 'default' },
  SLOW: { label: 'Slow', variant: 'warning' },
  DEAD: { label: 'Dead stock', variant: 'danger' },
};

const fmt = (n: number, dp = 2) => String(Number(n.toFixed(dp)));

/** The last 30 days, ending today: the panel's own default period (FR-39). */
const defaultRange = () => ({ startDate: addDays(todayLocal(), -29), endDate: todayLocal() });

/**
 * FR-39: fast / slow / dead stock. Uses its own period (last 30 days by default) because a speed
 * view needs a steady window, while the page period is usually "this month". Class, units, cover
 * and counts all come from the backend (DD-56).
 */
export const InventorySpeedPanel: React.FC = () => {
  const [range, setRange] = useState(defaultRange);
  const [tab, setTab] = useState<Tab>('ALL');
  const [sort, setSort] = useState<ReportSort>({ field: 'units_out', dir: 'DESC' });
  const [page, setPage] = useState(1);
  const drawers = useReportItemDrawers();

  const live = useLiveSearch<InventorySpeedReport>({
    initial: EMPTY,
    emptyBehavior: 'fetch',
    debounceMs: LIST_SEARCH_DEBOUNCE_MS,
    deps: [range.startDate, range.endDate, tab, sort.field, sort.dir, page],
    onQueryCommit: () => setPage(1),
    fetcher: (q) =>
      api.getInventorySpeedReport({
        fromDate: range.startDate,
        toDate: range.endDate,
        class: tab === 'ALL' ? null : tab,
        search: q || null,
        sort,
        page,
        pageSize: REPORT_PAGE_SIZE,
      }),
  });
  const error = live.error ? errorMessage(live.error) : null;
  const data = live.error ? EMPTY : live.results;
  const c = data.counts;

  const onSort = (field: string) => {
    setSort((s) => nextSort(s, field, field === 'name' || field === 'cover' ? 'ASC' : 'DESC'));
    setPage(1);
  };
  const head = (field: string, label: string, align?: 'right') => (
    <SortableHead field={field} label={label} sort={sort} onSort={onSort} align={align} />
  );
  const count = (n: number) => <span className="font-mono text-[11px] text-slate-500">{n}</span>;

  return (
    <section
      data-testid="inventory-speed"
      className="p-4 rounded-lg bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 shadow-xs"
    >
      <div className="flex flex-wrap items-center justify-between gap-3 mb-2">
        <h2 className="text-xs font-bold uppercase tracking-wider text-slate-700 dark:text-slate-300">
          Inventory speed
        </h2>
        <div className="flex flex-wrap items-center gap-3">
          <DateRangePicker
            startDate={range.startDate}
            endDate={range.endDate}
            presets={['7d', '30d', 'month', 'year', 'custom']}
            onChange={(r) => {
              if (!r.startDate || !r.endDate) return;
              setRange(r);
              setPage(1);
            }}
          />
          <ReportSearch value={live.query} onChange={live.setQuery} label="Search stock" placeholder="Search items..." />
        </div>
      </div>
      <Tabs
        className="mb-2"
        activeTab={tab}
        onChange={(id) => {
          setTab(id as Tab);
          setPage(1);
        }}
        tabs={[
          { id: 'ALL', label: 'All', badge: count(c.fast + c.normal + c.slow + c.dead) },
          { id: 'FAST', label: 'Fast', badge: count(c.fast) },
          { id: 'SLOW', label: 'Slow', badge: count(c.slow) },
          { id: 'DEAD', label: 'Dead', badge: count(c.dead) },
        ]}
      />
      {error ? (
        <div role="alert" className="text-xs text-rose-600 dark:text-rose-400 py-2">
          {error}{' '}
          <button type="button" onClick={() => void live.refresh()} className="underline cursor-pointer">
            Retry
          </button>
        </div>
      ) : (
        <>
          <Table dense scroll="bounded">
            <TableHeader>
              <TableRow>
                {head('name', 'Item')}
                {head('units_out', 'Units out', 'right')}
                {head('per_day', 'Per day', 'right')}
                {head('stock', 'Stock', 'right')}
                {head('cover', 'Days of cover', 'right')}
                {head('class', 'Class')}
              </TableRow>
            </TableHeader>
            <TableBody>
              {data.rows.length === 0 && (
                <TableRow>
                  <TableCell colSpan={6} className="text-xs text-slate-400 py-3">
                    {live.isLoading ? 'Loading...' : 'Nothing to show for this period.'}
                  </TableCell>
                </TableRow>
              )}
              {data.rows.map((r: InventorySpeedRow) => (
                <TableRow
                  key={r.itemId}
                  data-testid="inventory-speed-row"
                  onClick={() => drawers.open(r)}
                  className="cursor-pointer hover:bg-slate-50 dark:hover:bg-slate-800/50"
                >
                  <TableCell>{r.name}</TableCell>
                  <TableCell align="right" mono>
                    {fmt(r.unitsOut, 3)} <span className="text-slate-400">{r.baseUnit}</span>
                  </TableCell>
                  <TableCell align="right" mono>
                    {fmt(r.unitsPerDay)}
                  </TableCell>
                  <TableCell align="right" mono>
                    {fmt(r.stock, 3)}
                  </TableCell>
                  <TableCell align="right" mono>
                    {r.daysOfCover === null ? '-' : fmt(r.daysOfCover, 1)}
                  </TableCell>
                  <TableCell>
                    <Badge size="sm" variant={BADGE[r.class].variant}>
                      {BADGE[r.class].label}
                    </Badge>
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
          <Pager page={page} total={data.totalCount} limit={REPORT_PAGE_SIZE} onPageChange={setPage} />
          <p className="mt-2 text-xs text-slate-500 dark:text-slate-400">
            {data.days} day{data.days === 1 ? '' : 's'} in this period. Units out = sold net of returns plus used by
            services.
          </p>
        </>
      )}
      {drawers.drawers}
    </section>
  );
};

export default InventorySpeedPanel;
