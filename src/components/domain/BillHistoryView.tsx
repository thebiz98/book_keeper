import React, { useCallback, useEffect, useRef, useState } from 'react';
import { useLiveSearch, LIST_SEARCH_DEBOUNCE_MS } from '@/hooks/useLiveSearch';
import { Search, Eye, Pencil, HandCoins, Undo2, SlidersHorizontal } from 'lucide-react';
import { api, errorMessage } from '@/api';
import { useAppStore } from '@/store/useAppStore';
import type {
  HistoryFilter,
  PaymentStatusFilter,
  PurchaseHistoryRow,
  SaleHistoryRow,
  SalesHistoryTotals,
  TransactionType,
} from '@/types';
import { CurrencyDisplay } from '../common/CurrencyDisplay';
import { PaymentStatusBadge } from '../common/PaymentStatusBadge';
import { DateRangePicker } from '../common/DateRangePicker';
import { DataTable, type DataTableColumn } from './DataTable';
import { SettlementModal } from './SettlementModal';
import { BillDetailDrawer } from './BillDetailDrawer';
import { navigateWithParam, refreshAppMetrics } from './navParams';
import { formatTime } from '@/lib/dates';

type Row = SaleHistoryRow | PurchaseHistoryRow;

export const HISTORY_PAGE_SIZE = 25;
const STATUSES: PaymentStatusFilter[] = ['ALL', 'PAID', 'PARTIAL', 'UNPAID'];

export interface BillHistoryViewProps {
  kind: TransactionType;
}

const ICON_BTN =
  'p-1 rounded text-slate-500 hover:bg-slate-100 dark:hover:bg-slate-800 transition-colors cursor-pointer';

interface RowActionProps {
  label: string;
  onClick: () => void;
  hover: string;
  children: React.ReactNode;
}

const RowAction: React.FC<RowActionProps> = ({ label, onClick, hover, children }) => (
  <button type="button" aria-label={label} title={label} onClick={onClick} className={`${ICON_BTN} ${hover}`}>
    {children}
  </button>
);

/** Column menu (sales only): the "Show profit" checkbox (FR-34). */
const ColumnMenu: React.FC<{ showProfit: boolean; onChange: (on: boolean) => void }> = ({ showProfit, onChange }) => {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!open) return;
    const onDown = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false);
    };
    const onKey = (e: KeyboardEvent) => e.key === 'Escape' && setOpen(false);
    document.addEventListener('mousedown', onDown);
    document.addEventListener('keydown', onKey);
    return () => {
      document.removeEventListener('mousedown', onDown);
      document.removeEventListener('keydown', onKey);
    };
  }, [open]);
  return (
    <div ref={ref} className="relative">
      <button
        type="button"
        aria-label="Columns"
        title="Columns"
        aria-expanded={open}
        data-testid="column-menu"
        onClick={() => setOpen((o) => !o)}
        className={`${ICON_BTN} hover:text-blue-600 border border-slate-200 dark:border-slate-700 p-1.5`}
      >
        <SlidersHorizontal className="h-4 w-4" />
      </button>
      {open && (
        <div className="absolute right-0 top-full mt-1 z-20 min-w-[150px] rounded-md border border-slate-200 dark:border-slate-700 bg-white dark:bg-slate-900 shadow-md p-2">
          <label className="flex items-center gap-2 text-xs font-medium text-slate-700 dark:text-slate-200 cursor-pointer select-none">
            <input
              type="checkbox"
              aria-label="Show profit"
              checked={showProfit}
              onChange={(e) => onChange(e.target.checked)}
            />
            Show profit
          </label>
        </div>
      )}
    </div>
  );
};

/** Shared server-side sales / purchase history ledger (Pages 7 and 8). */
export const BillHistoryView: React.FC<BillHistoryViewProps> = ({ kind }) => {
  const isSale = kind === 'SALE';
  const [range, setRange] = useState({ startDate: '', endDate: '' });
  const [status, setStatus] = useState<PaymentStatusFilter>('ALL');
  const [page, setPage] = useState(1);
  const [detail, setDetail] = useState<{ id: number; mode: 'view' | 'edit' } | null>(null);
  const [settling, setSettling] = useState<Row | null>(null);
  const storeShowProfit = useAppStore((s) => s.showProfit);
  const setShowProfit = useAppStore((s) => s.setShowProfit);
  const showProfit = isSale && storeShowProfit;

  const live = useLiveSearch<{ items: Row[]; total: number; totals?: SalesHistoryTotals | null }>({
    initial: { items: [], total: 0 },
    emptyBehavior: 'fetch',
    debounceMs: LIST_SEARCH_DEBOUNCE_MS,
    deps: [range.startDate, range.endDate, status, page, isSale, showProfit],
    onQueryCommit: () => setPage(1),
    fetcher: (q) => {
      const filter: HistoryFilter = {
        startDate: range.startDate || null,
        endDate: range.endDate || null,
        paymentStatus: status,
        searchQuery: q || null,
        page,
        limit: HISTORY_PAGE_SIZE,
        ...(showProfit ? { includeProfit: true } : {}),
        ...(isSale ? { includeTotals: true } : {}),
      };
      return isSale ? api.getSalesHistory(filter) : api.getPurchasesHistory(filter);
    },
  });
  const loading = live.isLoading;
  const error = live.error ? errorMessage(live.error) : null;
  const rows: Row[] = live.error ? [] : live.results.items;
  const total = live.error ? 0 : live.results.total;
  const totals = isSale && !live.error ? live.results.totals ?? null : null;
  const searchInput = live.query;
  const setSearchInput = live.setQuery;
  const load = useCallback(() => void live.refresh(), [live.refresh]);

  const partyLabel = isSale ? 'Customer' : 'Vendor';
  const tight = 'px-2';
  const columns: DataTableColumn<Row>[] = [
    {
      key: 'code',
      header: isSale ? 'Invoice Code' : 'Purchase Code',
      mono: true,
      className: `${tight} whitespace-nowrap`,
      render: (r) => <span className="font-semibold text-blue-600 dark:text-blue-400">{r.transactionCode}</span>,
    },
    {
      key: 'date',
      header: isSale ? 'Date & Time' : 'Date',
      className: `${tight} whitespace-nowrap`,
      render: (r) => (
        <span className="text-xs text-slate-500">
          {r.transactionDate}
          {isSale && r.createdAt ? ` ${formatTime(r.createdAt)}` : ''}
        </span>
      ),
    },
    {
      key: 'party',
      header: partyLabel,
      className: tight,
      render: (r) => r.partyName ?? (isSale ? 'Walk-in Customer' : '-'),
    },
    ...(isSale
      ? [
          {
            key: 'lines',
            header: 'Items',
            align: 'center' as const,
            className: 'w-14 px-1 text-center',
            render: (r: Row) => r.lineCount,
          },
        ]
      : [
          {
            key: 'ref',
            header: 'Supplier Bill Ref',
            className: tight,
            render: (r: Row) => (r as PurchaseHistoryRow).supplierInvoiceRef ?? '-',
          },
        ]),
    {
      key: 'adj',
      header: 'Chg / Disc',
      align: 'right',
      className: tight,
      render: (r) =>
        r.chargesTotal || r.discountsTotal ? (
          <span className="flex flex-col items-end font-mono text-xs leading-tight" data-testid="adj-cell">
            {r.chargesTotal > 0 && <span className="whitespace-nowrap">{`+₹${r.chargesTotal.toFixed(2)}`}</span>}
            {r.discountsTotal > 0 && <span className="whitespace-nowrap">{`−₹${r.discountsTotal.toFixed(2)}`}</span>}
          </span>
        ) : (
          '—'
        ),
    },
    {
      key: 'total',
      header: 'Total',
      align: 'right',
      className: tight,
      render: (r) => <CurrencyDisplay amount={r.totalAmount} />,
    },
    {
      key: 'paid',
      header: 'Paid',
      align: 'right',
      className: tight,
      render: (r) => <CurrencyDisplay amount={r.paidAmount} />,
    },
    {
      key: 'due',
      header: 'Balance Due',
      align: 'right',
      className: tight,
      render: (r) => <CurrencyDisplay amount={r.balanceDue} colored invertColors={!isSale} />,
    },
    ...(showProfit
      ? [
          {
            key: 'profit',
            header: 'Profit',
            align: 'right' as const,
            className: tight,
            render: (r: Row) => {
              const p = (r as SaleHistoryRow).profit;
              return p == null ? '-' : <CurrencyDisplay amount={p} colored />;
            },
          },
        ]
      : []),
    {
      key: 'status',
      header: 'Status',
      align: 'center',
      className: 'px-1',
      render: (r) => <PaymentStatusBadge status={r.paymentStatus} size="sm" />,
    },
    {
      key: 'actions',
      header: 'Actions',
      align: 'center',
      className: 'px-1',
      render: (r) => (
        <div className="flex items-center justify-center gap-0.5" onClick={(e) => e.stopPropagation()}>
          <RowAction label={`View ${r.transactionCode}`} hover="hover:text-blue-600" onClick={() => setDetail({ id: r.id, mode: 'view' })}>
            <Eye className="h-3.5 w-3.5" />
          </RowAction>
          <RowAction label={`Edit ${r.transactionCode}`} hover="hover:text-blue-600" onClick={() => setDetail({ id: r.id, mode: 'edit' })}>
            <Pencil className="h-3.5 w-3.5" />
          </RowAction>
          {r.balanceDue > 0 && r.partyId !== null && (
            <RowAction label={`Settle ${r.transactionCode}`} hover="hover:text-emerald-600" onClick={() => setSettling(r)}>
              <HandCoins className="h-3.5 w-3.5" />
            </RowAction>
          )}
          <RowAction
            label={`Return ${r.transactionCode}`}
            hover="hover:text-amber-600"
            onClick={() => navigateWithParam('returns', 'returnBill', `${kind}:${r.id}`)}
          >
            <Undo2 className="h-3.5 w-3.5" />
          </RowAction>
        </div>
      ),
    },
  ];

  const footer = totals
    ? {
        code: `Total (${totals.billCount} ${totals.billCount === 1 ? 'bill' : 'bills'})`,
        total: <CurrencyDisplay amount={totals.totalAmount} />,
        paid: <CurrencyDisplay amount={totals.paidAmount} />,
        due: <CurrencyDisplay amount={totals.balanceDue} colored />,
        ...(showProfit && totals.profit != null
          ? {
              profit: (
                <span title="Bill profit of every bill in this list (returns of these bills included). The Reports P&L counts returns by return date.">
                  <CurrencyDisplay amount={totals.profit} colored />
                </span>
              ),
            }
          : {}),
      }
    : undefined;

  return (
    <div className="flex flex-col gap-4 flex-1 min-h-0">
      <div className="shrink-0 flex flex-wrap items-center gap-3 bg-white dark:bg-slate-900 p-3 rounded-lg border border-slate-200 dark:border-slate-800">
        <div className="relative flex-1 min-w-[200px]">
          <Search className="absolute left-2.5 top-2.5 text-slate-400" size={16} />
          <input
            type="text"
            aria-label="Search"
            placeholder={isSale ? 'Search invoice code or customer...' : 'Search purchase code, vendor or supplier bill ref...'}
            value={searchInput}
            onChange={(e) => setSearchInput(e.target.value)}
            className="w-full pl-8 pr-3 py-1.5 text-sm bg-slate-50 dark:bg-slate-800 border border-slate-300 dark:border-slate-700 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500"
          />
        </div>
        <div className="flex items-center gap-1 bg-slate-100 dark:bg-slate-800 p-1 rounded-md text-xs font-medium">
          {STATUSES.map((st) => (
            <button
              key={st}
              type="button"
              data-status={st}
              onClick={() => {
                setStatus(st);
                setPage(1);
              }}
              className={`px-2.5 py-1 rounded transition-colors cursor-pointer ${
                status === st
                  ? 'bg-white dark:bg-slate-900 text-blue-600 dark:text-blue-400 font-semibold shadow-2xs'
                  : 'text-slate-600 dark:text-slate-400 hover:text-slate-900'
              }`}
            >
              {st}
            </button>
          ))}
        </div>
        <DateRangePicker
          startDate={range.startDate}
          endDate={range.endDate}
          presets={['all', 'today', 'week', 'month', 'year', 'custom']}
          onChange={(r) => {
            setRange(r);
            setPage(1);
          }}
        />
        {isSale && <ColumnMenu showProfit={storeShowProfit} onChange={setShowProfit} />}
      </div>

      <DataTable
        scroll="fill"
        className="flex-1 min-h-[240px]"
        columns={columns}
        items={rows}
        rowKey={(r) => r.id}
        isLoading={loading}
        error={error}
        onRetry={load}
        emptyMessage={isSale ? 'No sales invoices match the filters.' : 'No purchases match the filters.'}
        onRowClick={(r) => setDetail({ id: r.id, mode: 'view' })}
        footer={footer}
        footerTestId="history-totals"
        total={total}
        page={page}
        limit={HISTORY_PAGE_SIZE}
        onPageChange={setPage}
      />

      <BillDetailDrawer
        kind={kind}
        billId={detail?.id ?? null}
        initialMode={detail?.mode}
        showProfit={showProfit}
        onClose={() => setDetail(null)}
        onChanged={load}
      />

      {settling && settling.partyId !== null && (
        <SettlementModal
          isOpen
          onClose={() => setSettling(null)}
          partyId={settling.partyId}
          partyName={settling.partyName ?? ''}
          totalDue={settling.balanceDue}
          transactionType={kind}
          transactionId={settling.id}
          billCode={settling.transactionCode}
          onSettled={() => {
            setSettling(null);
            refreshAppMetrics();
            load();
          }}
        />
      )}
    </div>
  );
};

export default BillHistoryView;
