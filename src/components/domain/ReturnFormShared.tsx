import React, { useEffect, useState } from 'react';
import { api, errorMessage } from '@/api';
import { round2 } from '@/lib/money';
import type { PartyType, PurchaseHistoryRow, ReturnResult, SaleHistoryRow, TransactionType } from '@/types';
import { CurrencyDisplay } from '../common/CurrencyDisplay';

/** Tolerance for decimal quantity / price comparisons. */
export const EPS = 1e-6;

/** Refund preview: value reduces the available balance first, the rest is cash. */
export function creditSplit(value: number, balance: number): { credited: number; cash: number } {
  const v = round2(Math.max(0, value));
  const credited = round2(Math.min(v, Math.max(0, balance)));
  return { credited, cash: round2(v - credited) };
}

export const RefundPreview: React.FC<{ value: number; balance: number | null; label?: string }> = ({
  value,
  balance,
  label = 'Preview',
}) => {
  const { credited, cash } = creditSplit(value, balance ?? 0);
  return (
    <div data-testid="refund-preview" className="p-2.5 rounded-md bg-blue-50/60 dark:bg-blue-950/20 border border-blue-100 dark:border-blue-900/40 text-xs">
      <span className="font-semibold">{label}: </span>
      <CurrencyDisplay amount={credited} size="sm" /> credited · <CurrencyDisplay amount={cash} size="sm" /> cash
      <span className="text-slate-500"> (total value </span>
      <CurrencyDisplay amount={value} size="sm" />
      <span className="text-slate-500">)</span>
    </div>
  );
};

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

/** Outstanding balance of a party (from the directory rows), null while unknown. */
export function usePartyBalance(partyType: PartyType, partyId: number | null): number | null {
  const [balance, setBalance] = useState<number | null>(null);
  useEffect(() => {
    setBalance(null);
    if (partyId === null) return;
    let live = true;
    api
      .listParties(partyType, false)
      .then((rows) => live && setBalance(rows.find((r) => r.id === partyId)?.outstandingBalance ?? 0))
      .catch(() => live && setBalance(null));
    return () => {
      live = false;
    };
  }, [partyType, partyId]);
  return balance;
}

type BillRow = SaleHistoryRow | PurchaseHistoryRow;

/** Search-and-pick a bill by code or party name (server-side search). */
export const BillPicker: React.FC<{
  kind: TransactionType;
  onPick: (id: number) => void;
}> = ({ kind, onPick }) => {
  const [query, setQuery] = useState('');
  const [rows, setRows] = useState<BillRow[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    const q = query.trim();
    if (q.length < 2) {
      setRows([]);
      setError(null);
      return;
    }
    let live = true;
    const t = setTimeout(() => {
      setLoading(true);
      setError(null);
      const filter = { searchQuery: q, paymentStatus: 'ALL' as const, limit: 8 };
      const req = kind === 'SALE' ? api.getSalesHistory(filter) : api.getPurchasesHistory(filter);
      req
        .then((r) => live && setRows(r.items))
        .catch((e) => live && setError(errorMessage(e)))
        .finally(() => live && setLoading(false));
    }, 250);
    return () => {
      live = false;
      clearTimeout(t);
    };
  }, [query, kind]);

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
