import React from 'react';
import { useLiveSearch, LIST_SEARCH_DEBOUNCE_MS } from '@/hooks/useLiveSearch';
import { api, errorMessage } from '@/api';
import type { PreviewError, PurchaseHistoryRow, ReturnResult, SaleHistoryRow, TransactionType } from '@/types';
import { CurrencyDisplay } from '../common/CurrencyDisplay';

/** Tolerance for decimal quantity / price comparisons. */
export const EPS = 1e-6;

/**
 * Refund preview (DD-56): the credited/cash split and the value come from the backend's
 * `preview_*_return`; this only shows them.
 */
export const RefundPreview: React.FC<{
  result: ReturnResult | null;
  errors?: PreviewError[];
  label?: string;
}> = ({ result, errors = [], label = 'Preview' }) => (
  <div data-testid="refund-preview" className="p-2.5 rounded-md bg-blue-50/60 dark:bg-blue-950/20 border border-blue-100 dark:border-blue-900/40 text-xs">
    <span className="font-semibold">{label}: </span>
    {result ? (
      <>
        <CurrencyDisplay amount={result.creditedAmount} size="sm" /> credited ·{' '}
        <CurrencyDisplay amount={result.cashRefunded} size="sm" /> cash
        <span className="text-slate-500"> (total value </span>
        <CurrencyDisplay amount={result.totalAmount} size="sm" />
        <span className="text-slate-500">)</span>
        {(result.discountReversed ?? 0) > 0 && (
          <span data-testid="discount-reversed" className="text-slate-500">
            {' '}· Purchase discount reversed <CurrencyDisplay amount={result.discountReversed} size="sm" />
          </span>
        )}
      </>
    ) : (
      <span className="text-slate-500">—</span>
    )}
    {errors.length > 0 && (
      <p role="alert" data-testid="return-preview-error" className="mt-1 text-rose-600 dark:text-rose-400">
        {errors[0].message}
      </p>
    )}
  </div>
);
export const ReturnResultPanel: React.FC<{ result: ReturnResult; onAnother: () => void }> = ({ result, onAnother }) => (
  <div data-testid="return-result" className="flex flex-col gap-2 p-4 rounded-lg border border-emerald-200 dark:border-emerald-900 bg-emerald-50/50 dark:bg-emerald-950/20">
    <p className="text-sm font-semibold text-emerald-700 dark:text-emerald-400">
      Return {result.returnCode} recorded
    </p>
    <p className="text-sm">
      Credited to dues: <CurrencyDisplay amount={result.creditedAmount} /> · Cash:{' '}
      <CurrencyDisplay amount={result.cashRefunded} />
    </p>
    <button type="button" onClick={onAnother} className="self-start text-xs text-blue-600 underline cursor-pointer">
      Record another return
    </button>
  </div>
);


type BillRow = SaleHistoryRow | PurchaseHistoryRow;

/** Search-and-pick a bill by code or party name (server-side search). */
export const BillPicker: React.FC<{
  kind: TransactionType;
  onPick: (id: number) => void;
}> = ({ kind, onPick }) => {
  const live = useLiveSearch<BillRow[]>({
    initial: [],
    minChars: 2,
    debounceMs: LIST_SEARCH_DEBOUNCE_MS,
    deps: [kind],
    fetcher: (q) => {
      const filter = { searchQuery: q, paymentStatus: 'ALL' as const, limit: 8 };
      const req = kind === 'SALE' ? api.getSalesHistory(filter) : api.getPurchasesHistory(filter);
      return req.then((r) => r.items);
    },
  });
  const { query, setQuery } = live;
  const rows = live.error ? [] : live.results;
  const loading = live.isLoading;
  const error = live.error ? errorMessage(live.error) : null;

  return (
    <div className="flex flex-col gap-1">
      <label htmlFor={`bill-search-${kind}`} className="text-xs font-medium text-slate-700 dark:text-slate-300">
        {kind === 'SALE' ? 'Find invoice (code or customer)' : 'Find purchase bill (code, vendor or supplier ref)'}
      </label>
      <input
        id={`bill-search-${kind}`}
        type="text"
        value={query}
        onChange={(e) => setQuery(e.target.value)}
        placeholder="Type at least 2 characters..."
        className="w-full rounded-md border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-900 text-sm px-3 py-1.5"
      />
      {loading && <p className="text-xs text-slate-500">Searching...</p>}
      {error && (
        <p role="alert" className="text-xs text-rose-600 dark:text-rose-400">
          {error}
        </p>
      )}
      {rows.length > 0 && (
        <ul className="border border-slate-200 dark:border-slate-800 rounded-md divide-y divide-slate-100 dark:divide-slate-800 text-xs">
          {rows.map((r) => (
            <li key={r.id}>
              <button
                type="button"
                data-testid="bill-option"
                onClick={() => onPick(r.id)}
                className="w-full text-left px-2.5 py-1.5 hover:bg-slate-50 dark:hover:bg-slate-800 flex justify-between cursor-pointer"
              >
                <span>
                  <span className="font-mono font-semibold">{r.transactionCode}</span> · {r.transactionDate} ·{' '}
                  {r.partyName ?? (kind === 'SALE' ? 'Walk-in' : '-')}
                </span>
                <CurrencyDisplay amount={r.totalAmount} size="sm" />
              </button>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
};

export const FieldLabel: React.FC<{ htmlFor?: string; children: React.ReactNode }> = ({ htmlFor, children }) => (
  <label htmlFor={htmlFor} className="text-xs font-medium text-slate-700 dark:text-slate-300">
    {children}
  </label>
);

export const inputCls =
  'w-full rounded-md border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-900 text-sm px-3 py-1.5 focus:outline-none focus:ring-2 focus:ring-blue-500';
