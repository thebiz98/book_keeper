import React, { useCallback, useEffect, useState } from 'react';
import { Search, Pencil, Trash2 } from 'lucide-react';
import { api, errorMessage } from '@/api';
import { startOfMonth, todayLocal } from '@/lib/dates';
import { useLiveSearch, LIST_SEARCH_DEBOUNCE_MS } from '@/hooks/useLiveSearch';
import type { Expense, ExpenseListResponse, ExpenseType } from '@/types';
import { CurrencyDisplay } from '../common/CurrencyDisplay';
import { ConfirmDialog } from '../common/ConfirmDialog';
import { DateRangePicker } from '../common/DateRangePicker';
import { Button } from '../ui/Button';
import { Select } from '../ui/Select';
import { DataTable, type DataTableColumn } from './DataTable';
import { refreshAppMetrics } from './navParams';

export const EXPENSE_PAGE_SIZE = 25;
const EMPTY: ExpenseListResponse = { items: [], total: 0, page: 1, limit: EXPENSE_PAGE_SIZE, periodTotal: 0 };

export interface ExpenseListProps {
  /** Bump to refetch (after a save elsewhere). */
  reloadKey: number;
  onEdit: (e: Expense) => void;
  /** Called after a delete so the page can refresh the code preview. */
  onDeleted: (e: Expense) => void;
}

/** Expenses list (FR-18): date range (default this month), type filter, live search, period total. */
export const ExpenseList: React.FC<ExpenseListProps> = ({ reloadKey, onEdit, onDeleted }) => {
  const [range, setRange] = useState({ startDate: startOfMonth(), endDate: todayLocal() });
  const [typeId, setTypeId] = useState(0);
  const [page, setPage] = useState(1);
  const [types, setTypes] = useState<ExpenseType[]>([]);
  const [deleting, setDeleting] = useState<Expense | null>(null);
  const [deleteError, setDeleteError] = useState<string | null>(null);

  useEffect(() => {
    let live = true;
    api
      .listExpenseTypes()
      .then((t) => live && setTypes(t))
      .catch(() => undefined);
    return () => {
      live = false;
    };
  }, [reloadKey]);

  const live = useLiveSearch<ExpenseListResponse>({
    initial: EMPTY,
    emptyBehavior: 'fetch',
    debounceMs: LIST_SEARCH_DEBOUNCE_MS,
    deps: [range.startDate, range.endDate, typeId, page, reloadKey],
    onQueryCommit: () => setPage(1),
    fetcher: (q) =>
      api.listExpenses({
        startDate: range.startDate || null,
        endDate: range.endDate || null,
        expenseTypeId: typeId || null,
        searchQuery: q || null,
        page,
        limit: EXPENSE_PAGE_SIZE,
      }),
  });
  const error = live.error ? errorMessage(live.error) : null;
  const data = live.error ? EMPTY : live.results;
  const refresh = useCallback(() => void live.refresh(), [live.refresh]);

  const confirmDelete = async () => {
    if (!deleting) return;
    try {
      await api.deleteExpense(deleting.id);
      const gone = deleting;
      setDeleting(null);
      setDeleteError(null);
      refreshAppMetrics();
      onDeleted(gone);
      refresh();
    } catch (e) {
      setDeleteError(errorMessage(e));
    }
  };

  const columns: DataTableColumn<Expense>[] = [
    {
      key: 'code',
      header: 'Code',
      mono: true,
      render: (r) => <span className="font-semibold text-blue-600 dark:text-blue-400">{r.expenseCode}</span>,
    },
    { key: 'date', header: 'Date', render: (r) => <span className="text-xs text-slate-500">{r.expenseDate}</span> },
    { key: 'type', header: 'Type', render: (r) => r.expenseTypeName },
    { key: 'notes', header: 'Notes', render: (r) => <span className="text-xs text-slate-500">{r.notes ?? ''}</span> },
    {
      key: 'amount',
      header: 'Amount',
      align: 'right',
      render: (r) => <CurrencyDisplay amount={r.amount} size="sm" />,
    },
    {
      key: 'actions',
      header: '',
      align: 'right',
      render: (r) => (
        <div className="flex justify-end gap-1">
          <Button
            variant="ghost"
            size="xs"
            aria-label={`Edit ${r.expenseCode}`}
            data-testid={`expense-edit-${r.id}`}
            onClick={() => onEdit(r)}
            leftIcon={<Pencil className="h-3 w-3" />}
          >
            Edit
          </Button>
          <Button
            variant="ghost"
            size="xs"
            aria-label={`Delete ${r.expenseCode}`}
            data-testid={`expense-delete-${r.id}`}
            onClick={() => {
              setDeleteError(null);
              setDeleting(r);
            }}
            leftIcon={<Trash2 className="h-3 w-3" />}
          >
            Delete
          </Button>
        </div>
      ),
    },
  ];

  return (
    <div className="flex flex-col gap-3 flex-1 min-h-0">
      <div className="shrink-0 flex flex-wrap items-center gap-3 bg-white dark:bg-slate-900 p-3 rounded-lg border border-slate-200 dark:border-slate-800">
        <div className="relative flex-1 min-w-[180px]">
          <Search className="absolute left-2.5 top-2.5 text-slate-400" size={16} />
          <input
            type="text"
            aria-label="Search expenses"
            data-testid="expense-search"
            placeholder="Search code, type or notes..."
            value={live.query}
            onChange={(e) => live.setQuery(e.target.value)}
            className="w-full pl-8 pr-3 py-1.5 text-sm bg-slate-50 dark:bg-slate-800 border border-slate-300 dark:border-slate-700 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500"
          />
        </div>
        <Select<number>
          aria-label="Filter by type"
          data-testid="expense-type-filter"
          value={typeId}
          onChange={(v) => {
            setTypeId(Number(v));
            setPage(1);
          }}
          options={[{ value: 0, label: 'All types' }, ...types.map((t) => ({ value: t.id, label: t.name }))]}
        />
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
        className="flex-1 min-h-[200px]"
        columns={columns}
        items={data.items}
        rowKey={(r) => r.id}
        isLoading={live.isLoading}
        error={error}
        onRetry={refresh}
        emptyMessage="No expenses match the filters."
        total={data.total}
        page={page}
        limit={EXPENSE_PAGE_SIZE}
        onPageChange={setPage}
      />

      <div className="shrink-0 flex items-center justify-between text-sm px-1">
        <span className="text-slate-500">
          {data.total} {data.total === 1 ? 'expense' : 'expenses'}
        </span>
        <span data-testid="expense-period-total" className="font-semibold">
          Period total: <CurrencyDisplay amount={data.periodTotal} />
        </span>
      </div>

      <ConfirmDialog
        isOpen={deleting !== null}
        variant="danger"
        title="Delete expense"
        message={
          deleting
            ? `Delete ${deleting.expenseCode} (${deleting.expenseTypeName}, ₹${deleting.amount.toFixed(2)})? The amount returns to the cash drawer of ${deleting.expenseDate}.${deleteError ? ` ${deleteError}` : ''}`
            : ''
        }
        confirmText="Delete"
        onConfirm={() => void confirmDelete()}
        onCancel={() => setDeleting(null)}
      />
    </div>
  );
};

export default ExpenseList;
