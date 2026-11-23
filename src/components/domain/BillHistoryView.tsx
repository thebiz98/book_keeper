import React, { useCallback, useState } from 'react';
import { useLiveSearch, LIST_SEARCH_DEBOUNCE_MS } from '@/hooks/useLiveSearch';
import { Search, Eye, DollarSign, Undo2 } from 'lucide-react';
import { api, errorMessage } from '@/api';
import type {
  HistoryFilter,
  PaymentStatusFilter,
  PurchaseHistoryRow,
  SaleHistoryRow,
  TransactionType,
} from '@/types';
import { CurrencyDisplay } from '../common/CurrencyDisplay';
import { PaymentStatusBadge } from '../common/PaymentStatusBadge';
import { DateRangePicker } from '../common/DateRangePicker';
import { Button } from '../ui/Button';
import { DataTable, type DataTableColumn } from './DataTable';
import { SettlementModal } from './SettlementModal';
import { BillDetailDrawer } from './BillDetailDrawer';
import { navigateWithParam, refreshAppMetrics } from './navParams';

type Row = SaleHistoryRow | PurchaseHistoryRow;

export const HISTORY_PAGE_SIZE = 25;
const STATUSES: PaymentStatusFilter[] = ['ALL', 'PAID', 'PARTIAL', 'UNPAID'];

export interface BillHistoryViewProps {
  kind: TransactionType;
}

/** Shared server-side sales / purchase history ledger (Pages 7 and 8). */
export const BillHistoryView: React.FC<BillHistoryViewProps> = ({ kind }) => {
  const isSale = kind === 'SALE';
  const [range, setRange] = useState({ startDate: '', endDate: '' });
  const [status, setStatus] = useState<PaymentStatusFilter>('ALL');
  const [page, setPage] = useState(1);
  const [detailId, setDetailId] = useState<number | null>(null);
  const [settling, setSettling] = useState<Row | null>(null);

  const live = useLiveSearch<{ items: Row[]; total: number }>({
    initial: { items: [], total: 0 },
    emptyBehavior: 'fetch',
    debounceMs: LIST_SEARCH_DEBOUNCE_MS,
    deps: [range.startDate, range.endDate, status, page, isSale],
    onQueryCommit: () => setPage(1),
    fetcher: (q) => {
      const filter: HistoryFilter = {
        startDate: range.startDate || null,
        endDate: range.endDate || null,
        paymentStatus: status,
        searchQuery: q || null,
        page,
        limit: HISTORY_PAGE_SIZE,
      };
      return isSale ? api.getSalesHistory(filter) : api.getPurchasesHistory(filter);
    },
  });
  const loading = live.isLoading;
  const error = live.error ? errorMessage(live.error) : null;
  const rows: Row[] = live.error ? [] : live.results.items;
  const total = live.error ? 0 : live.results.total;
  const searchInput = live.query;
  const setSearchInput = live.setQuery;
  const load = useCallback(() => void live.refresh(), [live.refresh]);

  const partyLabel = isSale ? 'Customer' : 'Vendor';
  const columns: DataTableColumn<Row>[] = [
    {
      key: 'code',
      header: isSale ? 'Invoice Code' : 'Purchase Code',
      mono: true,
      render: (r) => <span className="font-semibold text-blue-600 dark:text-blue-400">{r.transactionCode}</span>,
    },
    {
      key: 'date',
      header: isSale ? 'Date & Time' : 'Date',
      render: (r) => (
        <span className="text-xs text-slate-500">
          {r.transactionDate}
          {isSale && r.createdAt ? ` ${r.createdAt.slice(11, 16)}` : ''}
        </span>
      ),
    },
    {
      key: 'party',
      header: partyLabel,
      render: (r) => r.partyName ?? (isSale ? 'Walk-in Customer' : '-'),
    },
    ...(isSale
      ? [{ key: 'lines', header: 'Items', align: 'center' as const, render: (r: Row) => r.lineCount }]
      : [
          {
            key: 'ref',
            header: 'Supplier Bill Ref',
            render: (r: Row) => (r as PurchaseHistoryRow).supplierInvoiceRef ?? '-',
          },
        ]),
    {
      key: 'adj',
      header: 'Chg / Disc',
      align: 'right',
      render: (r) =>
        r.chargesTotal || r.discountsTotal ? (
          <span className="font-mono text-xs" data-testid="adj-cell">
            +₹{r.chargesTotal.toFixed(2)} / −₹{r.discountsTotal.toFixed(2)}
          </span>
        ) : (
          '—'
        ),
    },
    { key: 'total', header: 'Total', align: 'right', render: (r) => <CurrencyDisplay amount={r.totalAmount} /> },
    { key: 'paid', header: 'Paid', align: 'right', render: (r) => <CurrencyDisplay amount={r.paidAmount} /> },
    {
      key: 'due',
      header: 'Balance Due',
      align: 'right',
      render: (r) => <CurrencyDisplay amount={r.balanceDue} colored />,
    },
    { key: 'status', header: 'Status', align: 'center', render: (r) => <PaymentStatusBadge status={r.paymentStatus} /> },
    {
      key: 'actions',
      header: 'Actions',
      align: 'right',
      render: (r) => (
        <div className="flex items-center justify-end gap-1.5" onClick={(e) => e.stopPropagation()}>
          <Button variant="ghost" size="xs" leftIcon={<Eye size={12} />} onClick={() => setDetailId(r.id)}>
            View
          </Button>
          {r.balanceDue > 0 && r.partyId !== null && (
            <Button variant="secondary" size="xs" leftIcon={<DollarSign size={12} />} onClick={() => setSettling(r)}>
              Settle
            </Button>
          )}
          <Button
            variant="ghost"
            size="xs"
            leftIcon={<Undo2 size={12} />}
            onClick={() => navigateWithParam('returns', 'returnBill', `${kind}:${r.id}`)}
          >
            Return
          </Button>
        </div>
      ),
    },
  ];

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
        onRowClick={(r) => setDetailId(r.id)}
        total={total}
        page={page}
        limit={HISTORY_PAGE_SIZE}
        onPageChange={setPage}
      />

      <BillDetailDrawer kind={kind} billId={detailId} onClose={() => setDetailId(null)} onChanged={load} />

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
