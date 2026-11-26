import React from 'react';
import type { BillPreview, PaymentStatus } from '@/types';
import { CurrencyDisplay } from '../../common/CurrencyDisplay';

/** Paid / Partial / Unpaid control and the paid amount. Dumb: the figures shown are passed in. */
export const PaymentStatusControl: React.FC<{
  status: PaymentStatus;
  onStatus: (s: PaymentStatus) => void;
  /** Shown while the status is not PARTIAL (what the backend says will be paid). */
  paidShown: number;
  paidInput: number;
  onPaidInput: (n: number) => void;
  /** Settlements and credits already on the bill. */
  applied: number;
}> = ({ status, onStatus, paidShown, paidInput, onPaidInput, applied }) => (
  <div className="flex flex-col gap-1.5" data-testid="edit-payment">
    <div className="flex flex-wrap items-center gap-3">
      <div
        role="radiogroup"
        aria-label="Payment status"
        className="flex p-0.5 rounded-lg border border-slate-200 dark:border-slate-700 bg-slate-100 dark:bg-slate-800 text-xs"
      >
        {(['PAID', 'PARTIAL', 'UNPAID'] as const).map((st) => (
          <button
            key={st}
            type="button"
            role="radio"
            aria-checked={status === st}
            onClick={() => onStatus(st)}
            className={`px-3 py-1.5 rounded-md font-semibold transition-colors cursor-pointer ${
              status === st
                ? 'bg-white dark:bg-slate-900 text-blue-600 dark:text-blue-400 shadow-2xs'
                : 'text-slate-600 dark:text-slate-400 hover:text-slate-900'
            }`}
          >
            {st}
          </button>
        ))}
      </div>
      <div className="flex items-center gap-1.5 bg-slate-50 dark:bg-slate-800 px-2 py-1 rounded border border-slate-200 dark:border-slate-700 text-xs">
        <span className="text-slate-400">Paid: ₹</span>
        <input
          type="number"
          aria-label="Paid amount"
          min={0}
          step="any"
          disabled={status !== 'PARTIAL'}
          value={status === 'PARTIAL' ? paidInput || '' : paidShown}
          onChange={(e) => onPaidInput(parseFloat(e.target.value) || 0)}
          className="w-24 bg-transparent font-mono font-bold text-right focus:outline-none disabled:opacity-60"
        />
      </div>
    </div>
    {applied > 0 && (
      <span data-testid="already-settled" className="text-[11px] text-slate-500">
        Already settled ₹{applied.toFixed(2)}
      </span>
    )}
  </div>
);

/** Items subtotal, charges, discounts, total and balance; every number is the preview's. */
/** `payable`: a purchase bill, so a positive balance is money the shop owes and shows red (FR-49). */
export const EditTotals: React.FC<{ totals: BillPreview | null; balanceDue: number; payable?: boolean }> = ({
  totals,
  balanceDue,
  payable = false,
}) => (
  <div className="flex flex-col items-end gap-0.5 text-xs" data-testid="edit-totals">
    <div>
      Items subtotal: <CurrencyDisplay amount={totals?.itemsSubtotal ?? 0} size="sm" />
    </div>
    {(totals?.chargesTotal ?? 0) > 0 && (
      <div>
        + Charges: <CurrencyDisplay amount={totals?.chargesTotal ?? 0} size="sm" />
      </div>
    )}
    {(totals?.discountsTotal ?? 0) > 0 && (
      <div>
        − Discounts: <CurrencyDisplay amount={totals?.discountsTotal ?? 0} size="sm" />
      </div>
    )}
    <div data-testid="edit-total" className="font-semibold">
      Total: <CurrencyDisplay amount={totals?.totalAmount ?? 0} size="sm" />
    </div>
    <div data-testid="edit-balance">
      Balance: <CurrencyDisplay amount={balanceDue} size="sm" colored invertColors={payable} />
    </div>
  </div>
);

/** Errors above the Save button, plus the save failure. */
export const EditErrors: React.FC<{ messages: string[] }> = ({ messages }) =>
  messages.length === 0 ? null : (
    <ul role="alert" data-testid="edit-errors" className="text-xs text-rose-600 dark:text-rose-400 list-disc pl-4">
      {messages.map((m, i) => (
        <li key={i}>{m}</li>
      ))}
    </ul>
  );

export const inputCls =
  'bg-transparent border border-slate-300 dark:border-slate-700 rounded px-1 text-right font-mono disabled:opacity-60';
