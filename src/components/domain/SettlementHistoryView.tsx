import React, { useCallback, useRef, useState } from 'react';
import { Pencil, Search, Trash2 } from 'lucide-react';
import { api, errorMessage } from '@/api';
import { useLiveSearch, LIST_SEARCH_DEBOUNCE_MS } from '@/hooks/useLiveSearch';
import { useGridKeyboardNav } from '@/hooks/useGridKeyboardNav';
import { startOfMonth, todayLocal } from '@/lib/dates';
import type { PaymentSettlement, SettlementHistoryPage, SettlementHistoryRow, TransactionType } from '@/types';
import { CurrencyDisplay } from '../common/CurrencyDisplay';
import { DateRangePicker } from '../common/DateRangePicker';
import { ConfirmDialog } from '../common/ConfirmDialog';
import { Select } from '../ui/Select';
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '../ui/Table';
import { SettlementEditModal } from './SettlementEditModal';
import { BillDetailDrawer } from './BillDetailDrawer';
import { Pager } from './Pager';
import { refreshAppMetrics } from './navParams';

export const SETTLEMENT_HISTORY_PAGE_SIZE = 25;

type Side = 'ALL' | TransactionType;

const SIDE_OPTIONS: { value: Side; label: string }[] = [
  { value: 'ALL', label: 'All' },
  { value: 'SALE', label: 'Received from customers' },
  { value: 'PURCHASE', label: 'Paid to vendors' },
];

const EMPTY_PAGE: SettlementHistoryPage = { rows: [], totalReceived: 0, totalPaid: 0, totalCount: 0 };

const ICON_BTN =
  'p-1 rounded text-slate-500 hover:bg-slate-100 dark:hover:bg-slate-800 transition-colors cursor-pointer';

const asLedgerRow = (r: SettlementHistoryRow): PaymentSettlement => ({
  id: r.settlementId,
  settlementDate: r.settlementDate,
  partyId: r.partyId,
  kind: 'SETTLEMENT',
  transactionType: r.transactionType,
  transactionId: r.transactionId,
  returnId: null,
  amountPaid: r.amount,
  paymentMode: r.paymentMode,
  notes: r.notes,
  createdAt: '',
});

export interface SettlementHistoryViewProps {
  /** Called after an edit or delete so the dues tabs reload (header cash refreshes here). */
  onChanged?: () => void;
}

/** FR-42: every later due payment across all parties, newest first, with Edit / Delete per row. */
export const SettlementHistoryView: React.FC<SettlementHistoryViewProps> = ({ onChanged }) => {
  const [range, setRange] = useState(() => ({ startDate: startOfMonth(), endDate: todayLocal() }));
  const [side, setSide] = useState<Side>('ALL');
  const [page, setPage] = useState(1);
  const [bill, setBill] = useState<SettlementHistoryRow | null>(null);
  const [editing, setEditing] = useState<SettlementHistoryRow | null>(null);
  const [deleting, setDeleting] = useState<SettlementHistoryRow | null>(null);
  const [deleteError, setDeleteError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const searchRef = useRef<HTMLInputElement>(null);

  const live = useLiveSearch<SettlementHistoryPage>({
    initial: EMPTY_PAGE,
    emptyBehavior: 'fetch',
    debounceMs: LIST_SEARCH_DEBOUNCE_MS,
    deps: [range.startDate, range.endDate, side, page],
    onQueryCommit: () => setPage(1),
    fetcher: (q) =>
      api.getSettlementHistory({
        fromDate: range.startDate,
        toDate: range.endDate,
        side: side === 'ALL' ? null : side,
        partySearch: q || null,
        page,
        pageSize: SETTLEMENT_HISTORY_PAGE_SIZE,
      }),
  });
  const error = live.error ? errorMessage(live.error) : null;
  const data = live.error ? EMPTY_PAGE : live.results;
  const rows = data.rows;
  const reload = useCallback(() => void live.refresh(), [live.refresh]);

  const afterChange = () => {
    reload();
    refreshAppMetrics();
    onChanged?.();
  };

  const nav = useGridKeyboardNav<SettlementHistoryRow>({
    items: rows,
    getId: (r) => r.settlementId,
    enabled: !bill && !editing && !deleting && !live.isLoading && !error,
    onOpen: setBill,
    letters: {
      e: setEditing,
      delete: (r) => {
        setDeleteError(null);
        setDeleting(r);
      },
    },
    onLeaveTop: () => searchRef.current?.focus(),
  });

  const askDelete = (r: SettlementHistoryRow) => {
    setDeleteError(null);
    setDeleting(r);
  };

  const confirmDelete = async () => {
    if (!deleting || busy) return;
    setBusy(true);
    setDeleteError(null);
    try {
      await api.deleteSettlement(deleting.settlementId);
      setDeleting(null);
      afterChange();
    } catch (e) {
      setDeleteError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="flex flex-col gap-3 flex-1 min-h-0" data-testid="settlement-history">
      <div className="shrink-0 flex flex-wrap items-center gap-3 bg-white dark:bg-slate-900 p-3 rounded-lg border border-slate-200 dark:border-slate-800">
        <DateRangePicker
          startDate={range.startDate}
          endDate={range.endDate}
          onChange={(r) => {
            if (!r.startDate || !r.endDate) return;
            setRange(r);
            setPage(1);
          }}
        />
        <div className="w-56">
          <Select<Side>
            aria-label="Side"
            options={SIDE_OPTIONS}
            value={side}
            onChange={(v) => {
              setSide(v);
              setPage(1);
            }}
          />
        </div>
        <div className="relative flex-1 min-w-[200px] max-w-sm">
          <Search className="absolute left-2.5 top-2.5 text-slate-400" size={16} />
          <input
            ref={searchRef}
            type="text"
            aria-label="Search party"
            placeholder="Search party..."
            value={live.query}
            onChange={(e) => live.setQuery(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === 'ArrowDown') {
                e.preventDefault();
                (e.target as HTMLInputElement).blur();
                nav.enterGrid();
              }
            }}
            className="w-full pl-8 pr-3 py-1.5 text-sm bg-slate-50 dark:bg-slate-800 border border-slate-300 dark:border-slate-700 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500"
          />
        </div>
      </div>

      <div
        data-testid="settlement-summary"
        className="shrink-0 flex flex-wrap items-center gap-x-6 gap-y-1 px-3 text-xs text-slate-600 dark:text-slate-400"
      >
        <span>
          Received <CurrencyDisplay amount={data.totalReceived} size="sm" className="font-semibold" />
        </span>
        <span>
          Paid <CurrencyDisplay amount={data.totalPaid} size="sm" className="font-semibold" />
        </span>
        <span className="font-mono">{data.totalCount} settlements</span>
      </div>

      <div className="flex-1 min-h-[240px] min-h-0 flex flex-col rounded-lg border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 overflow-hidden">
        {error ? (
          <div role="alert" className="p-4 text-sm text-rose-600 dark:text-rose-400">
            {error}{' '}
            <button type="button" className="underline cursor-pointer" onClick={reload}>
              Retry
            </button>
          </div>
        ) : (
          <Table dense scroll="fill">
            <TableHeader>
              <TableRow>
                <TableHead>Date</TableHead>
                <TableHead>Party</TableHead>
                <TableHead>Bill</TableHead>
                <TableHead>Side</TableHead>
                <TableHead align="right">Amount</TableHead>
                <TableHead>Notes</TableHead>
                <TableHead align="center">Actions</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {rows.length === 0 ? (
                <TableRow>
                  <TableCell colSpan={7} className="py-8 text-center text-slate-500" data-testid="settlement-empty">
                    {live.isLoading ? 'Loading...' : 'No settlements match the filters.'}
                  </TableCell>
                </TableRow>
              ) : (
                rows.map((r) => {
                  const received = r.transactionType === 'SALE';
                  return (
                    <TableRow
                      key={r.settlementId}
                      data-testid="settlement-row"
                      data-row-id={r.settlementId}
                      selected={nav.activeId === r.settlementId}
                      aria-selected={nav.activeId === r.settlementId}
                      className="cursor-pointer"
                      onClick={(e) => {
                        if ((e.target as HTMLElement).closest('button, input, select, textarea, a')) return;
                        nav.setActiveId(r.settlementId);
                      }}
                    >
                      <TableCell mono className="whitespace-nowrap">
                        {r.settlementDate}
                      </TableCell>
                      <TableCell>{r.partyName || '-'}</TableCell>
                      <TableCell>
                        <button
                          type="button"
                          aria-label={`Open bill ${r.transactionCode}`}
                          className="font-mono font-semibold text-blue-600 dark:text-blue-400 hover:underline cursor-pointer"
                          onClick={() => setBill(r)}
                        >
                          {r.isOpeningBalance ? (
                            <span data-testid="opening-label" className="font-sans">
                              Opening balance
                            </span>
                          ) : (
                            r.transactionCode
                          )}
                        </button>
                      </TableCell>
                      <TableCell>
                        <span
                          data-testid="side-badge"
                          className={`inline-block rounded px-1.5 py-0.5 text-[11px] font-semibold ${
                            received
                              ? 'bg-emerald-50 text-emerald-700 dark:bg-emerald-950 dark:text-emerald-300'
                              : 'bg-amber-50 text-amber-700 dark:bg-amber-950 dark:text-amber-300'
                          }`}
                        >
                          {received ? 'Received' : 'Paid'}
                        </span>
                      </TableCell>
                      <TableCell align="right" mono>
                        <CurrencyDisplay amount={r.amount} size="sm" />
                      </TableCell>
                      <TableCell className="max-w-[220px] truncate" title={r.notes ?? undefined}>
                        {r.notes ?? ''}
                      </TableCell>
                      <TableCell align="center">
                        <div className="flex items-center justify-center gap-0.5">
                          <button
                            type="button"
                            aria-label={`Edit settlement ${r.transactionCode}`}
                            title="Edit (E)"
                            className={`${ICON_BTN} hover:text-blue-600`}
                            onClick={() => setEditing(r)}
                          >
                            <Pencil className="h-3.5 w-3.5" />
                          </button>
                          <button
                            type="button"
                            aria-label={`Delete settlement ${r.transactionCode}`}
                            title="Delete (Del)"
                            className={`${ICON_BTN} hover:text-rose-600`}
                            onClick={() => askDelete(r)}
                          >
                            <Trash2 className="h-3.5 w-3.5" />
                          </button>
                        </div>
                      </TableCell>
                    </TableRow>
                  );
                })
              )}
            </TableBody>
          </Table>
        )}
        <Pager page={page} total={data.totalCount} limit={SETTLEMENT_HISTORY_PAGE_SIZE} onPageChange={setPage} />
      </div>

      <BillDetailDrawer
        kind={bill?.transactionType ?? 'SALE'}
        billId={bill?.transactionId ?? null}
        onClose={() => setBill(null)}
        onChanged={afterChange}
      />

      <SettlementEditModal
        settlement={editing ? asLedgerRow(editing) : null}
        billCode={editing?.transactionCode}
        onClose={() => setEditing(null)}
        onSaved={() => {
          setEditing(null);
          afterChange();
        }}
      />

      <ConfirmDialog
        isOpen={deleting !== null}
        variant="danger"
        title="Delete settlement"
        message={
          deleting
            ? `Delete the settlement of ₹${deleting.amount.toFixed(2)} on ${
                deleting.isOpeningBalance ? 'the opening balance' : deleting.transactionCode
              }? The bill's balance goes back up.`
            : ''
        }
        confirmText="Delete"
        onConfirm={() => void confirmDelete()}
        onCancel={() => setDeleting(null)}
      >
        {deleteError && (
          <p role="alert" className="text-xs text-rose-600 dark:text-rose-400">
            {deleteError}
          </p>
        )}
      </ConfirmDialog>
    </div>
  );
};

export default SettlementHistoryView;
