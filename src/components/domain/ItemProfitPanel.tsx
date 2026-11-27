import React, { useState } from 'react';
import { api, errorMessage } from '@/api';
import { useLiveSearch, LIST_SEARCH_DEBOUNCE_MS } from '@/hooks/useLiveSearch';
import type { ItemProfitReport, ItemProfitRow, ReportSort } from '@/types';
import { CurrencyDisplay } from '../common/CurrencyDisplay';
import { Table, TableBody, TableCell, TableFooter, TableHead, TableHeader, TableRow } from '../ui/Table';
import { Pager } from './Pager';
import {
  nextSort,
  REPORT_PAGE_SIZE,
  ReportSearch,
  SortableHead,
  useReportItemDrawers,
} from './ReportListParts';

const EMPTY: ItemProfitReport = {
  rows: [],
  totals: { revenue: 0, cost: 0, profit: 0, marginPct: null },
  reconciliation: { itemsProfit: 0, charges: 0, replacementCost: 0, billsProfit: 0, pnlGrossProfit: 0 },
  totalCount: 0,
  page: 1,
  pageSize: REPORT_PAGE_SIZE,
};

const TEXT_FIELDS = new Set(['name']);
const fmtQty = (n: number) => String(Number(n.toFixed(3)));
const fmtPct = (n: number | null) => (n === null ? '-' : `${n.toFixed(1)}%`);

export interface ItemProfitPanelProps {
  fromDate: string;
  toDate: string;
}

/** FR-38: item-wise profit for the Reports period. Everything shown comes from one backend query. */
export const ItemProfitPanel: React.FC<ItemProfitPanelProps> = ({ fromDate, toDate }) => {
  const [sort, setSort] = useState<ReportSort>({ field: 'profit', dir: 'DESC' });
  const [page, setPage] = useState(1);
  const drawers = useReportItemDrawers();

  const live = useLiveSearch<ItemProfitReport>({
    initial: EMPTY,
    emptyBehavior: 'fetch',
    debounceMs: LIST_SEARCH_DEBOUNCE_MS,
    deps: [fromDate, toDate, sort.field, sort.dir, page],
    onQueryCommit: () => setPage(1),
    fetcher: (q) =>
      api.getItemProfitReport({ fromDate, toDate, search: q || null, sort, page, pageSize: REPORT_PAGE_SIZE }),
  });
  const error = live.error ? errorMessage(live.error) : null;
  const data = live.error ? EMPTY : live.results;
  const rec = data.reconciliation;

  const onSort = (field: string) => {
    setSort((s) => nextSort(s, field, TEXT_FIELDS.has(field) ? 'ASC' : 'DESC'));
    setPage(1);
  };
  const head = (field: string, label: string, align?: 'right') => (
    <SortableHead field={field} label={label} sort={sort} onSort={onSort} align={align} />
  );

  return (
    <section
      data-testid="item-profit"
      className="p-4 rounded-lg bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 shadow-xs"
    >
      <div className="flex flex-wrap items-center justify-between gap-3 mb-2">
        <h2 className="text-xs font-bold uppercase tracking-wider text-slate-700 dark:text-slate-300">Item profit</h2>
        <ReportSearch value={live.query} onChange={live.setQuery} label="Search items" placeholder="Search items..." />
      </div>
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
                {head('qty', 'Qty', 'right')}
                {head('revenue', 'Revenue', 'right')}
                <TableHead align="right">Returns</TableHead>
                <TableHead align="right">Returned cost</TableHead>
                {head('cost', 'Cost', 'right')}
                {head('profit', 'Profit', 'right')}
                {head('margin', 'Margin', 'right')}
              </TableRow>
            </TableHeader>
            <TableBody>
              {data.rows.length === 0 && (
                <TableRow>
                  <TableCell colSpan={8} className="text-xs text-slate-400 py-3">
                    {live.isLoading ? 'Loading...' : 'No sales or returns in this period.'}
                  </TableCell>
                </TableRow>
              )}
              {data.rows.map((r: ItemProfitRow) => (
                <TableRow
                  key={r.itemId}
                  data-testid="item-profit-row"
                  onClick={() => drawers.open(r)}
                  className="cursor-pointer hover:bg-slate-50 dark:hover:bg-slate-800/50"
                >
                  <TableCell>{r.name}</TableCell>
                  <TableCell align="right" mono>
                    {fmtQty(r.qtySold)} <span className="text-slate-400">{r.baseUnit}</span>
                  </TableCell>
                  <TableCell align="right">
                    <CurrencyDisplay amount={r.revenue} size="sm" />
                  </TableCell>
                  <TableCell align="right" data-testid="item-profit-returns">
                    <CurrencyDisplay amount={r.returnsValue ?? 0} size="sm" />
                  </TableCell>
                  <TableCell align="right" data-testid="item-profit-returned-cost">
                    <CurrencyDisplay amount={r.returnedCost ?? 0} size="sm" />
                  </TableCell>
                  <TableCell align="right">
                    <CurrencyDisplay amount={r.cost} size="sm" />
                  </TableCell>
                  <TableCell align="right">
                    <CurrencyDisplay amount={r.profit} size="sm" colored />
                  </TableCell>
                  <TableCell align="right" mono>
                    {fmtPct(r.marginPct)}
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
            <TableFooter>
              <TableRow data-testid="item-profit-totals" className="font-semibold">
                <TableCell>Total ({data.totalCount} items)</TableCell>
                <TableCell />
                <TableCell align="right">
                  <CurrencyDisplay amount={data.totals.revenue} size="sm" />
                </TableCell>
                <TableCell />
                <TableCell />
                <TableCell align="right">
                  <CurrencyDisplay amount={data.totals.cost} size="sm" />
                </TableCell>
                <TableCell align="right">
                  <CurrencyDisplay amount={data.totals.profit} size="sm" colored />
                </TableCell>
                <TableCell align="right" mono>
                  {fmtPct(data.totals.marginPct)}
                </TableCell>
              </TableRow>
            </TableFooter>
          </Table>
          <Pager page={page} total={data.totalCount} limit={REPORT_PAGE_SIZE} onPageChange={setPage} />
          <p data-testid="item-profit-reconciliation" className="mt-2 text-xs text-slate-500 dark:text-slate-400">
            Items profit <CurrencyDisplay amount={rec.itemsProfit} size="sm" /> = P&amp;L gross profit{' '}
            <CurrencyDisplay amount={rec.pnlGrossProfit ?? rec.itemsProfit} size="sm" /> (sales by bill date, returns by
            return date; scrapped returns are Inventory Losses). Bills dated in the period: profit{' '}
            <CurrencyDisplay amount={rec.billsProfit} size="sm" />, with charges{' '}
            <CurrencyDisplay amount={rec.charges} size="sm" /> and all their returns.
          </p>
        </>
      )}
      {drawers.drawers}
    </section>
  );
};

export default ItemProfitPanel;
