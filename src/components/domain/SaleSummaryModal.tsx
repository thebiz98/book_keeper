import React, { useState, useEffect, useRef } from 'react';
import { CheckCircle2, ArrowRight, Banknote } from 'lucide-react';
import { errorMessage } from '@/api';
import { round2 } from '@/lib/money';
import type { BillChargeLine } from '@/lib/billMath';
import type { PaymentStatus, SaleSummaryResponse } from '@/types';
import type { CustomerInput } from '@/types/store';
import { useBusyLock } from '@/hooks/useHotkeys';
import { cn } from '@/lib/utils';
import { CurrencyDisplay } from '../common/CurrencyDisplay';
import { Button } from '../ui/Button';
import { Modal } from '../ui/Modal';
import { PartySelector } from './PartySelector';
import { BillTotalsSummary } from './BillTotalsSummary';

export interface SalePaymentValues {
  paymentStatus: PaymentStatus;
  /** Cash received from the customer (0 for UNPAID). */
  cashReceived: number;
}

export interface SaleSummaryModalProps {
  isOpen: boolean;
  /** Payment mode: Esc / Back returns to the cart without saving. */
  onClose: () => void;
  grandTotal: number;
  /** Items subtotal and charge lines (FR-19); the breakdown shows above the Grand Total when lines exist. */
  itemsSubtotal?: number;
  charges?: BillChargeLine[];
  customer: CustomerInput | null;
  /** Lets the dialog pick / create a customer when PARTIAL or UNPAID needs one. */
  onCustomerChange?: (customer: CustomerInput | null) => void;
  /** Performs the sale. Rejections are shown inside the dialog. */
  onSubmit: (values: SalePaymentValues) => Promise<SaleSummaryResponse>;
  /** Result mode: Enter / Space starts the next bill. */
  onDone: (result: SaleSummaryResponse) => void;
  /** Display-only code preview shown while paying. */
  invoiceCode?: string;
  /** FR-20: 'edit' pays against `grandTotal - appliedAmount` and reads "Update bill". */
  mode?: 'create' | 'edit';
  /** Edit: money already applied to the bill by later settlements / return credits. */
  appliedAmount?: number;
  /** Edit: the bill's old upfront amount, prefilled as the cash received. */
  initialCash?: number;
}

const STATUSES: PaymentStatus[] = ['PAID', 'PARTIAL', 'UNPAID'];

export function validatePayment(
  status: PaymentStatus,
  total: number,
  cash: number,
  hasCustomer: boolean
): string | null {
  const t = round2(total);
  if (status !== 'PAID' && !hasCustomer) {
    return `Select a customer for a ${status} sale (Alt+C to create one)`;
  }
  if (status === 'UNPAID') return null;
  if (!Number.isFinite(cash) || cash < 0) return 'Enter the cash received';
  const c = round2(cash);
  if (status === 'PAID' && c < t) return 'Cash received must cover the total for a PAID sale';
  if (status === 'PARTIAL') {
    if (c <= 0) return 'Enter the amount received for a PARTIAL sale';
    if (c >= t) return 'PARTIAL requires cash less than the total; use PAID instead';
  }
  return null;
}

export const SaleSummaryModal: React.FC<SaleSummaryModalProps> = ({
  isOpen,
  onClose,
  grandTotal,
  itemsSubtotal,
  charges,
  customer,
  onCustomerChange,
  onSubmit,
  onDone,
  invoiceCode,
  mode: billMode = 'create',
  appliedAmount = 0,
  initialCash = 0,
}) => {
  const editing = billMode === 'edit';
  /** Amount this dialog collects: the whole total, or what is still open on an edited bill. */
  const due = editing ? round2(Math.max(grandTotal - appliedAmount, 0)) : grandTotal;
  const [status, setStatus] = useState<PaymentStatus>('PAID');
  const [cashStr, setCashStr] = useState(String(due));
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<SaleSummaryResponse | null>(null);

  const cashRef = useRef<HTMLInputElement>(null);
  const doneBtnRef = useRef<HTMLButtonElement>(null);
  const submittingRef = useRef(false);
  const totalRef = useRef(due);
  totalRef.current = due;
  const initialCashRef = useRef(initialCash);
  initialCashRef.current = initialCash;

  useBusyLock(submitting);

  // Reset only when the dialog opens: never while the cashier is typing.
  useEffect(() => {
    if (!isOpen) return;
    if (editing && totalRef.current > 0) {
      // Prefill from the old upfront payment: enough = PAID, some = PARTIAL, none = UNPAID.
      const c = initialCashRef.current;
      const st: PaymentStatus = c >= totalRef.current ? 'PAID' : c > 0 ? 'PARTIAL' : 'UNPAID';
      setStatus(st);
      setCashStr(st === 'PAID' ? String(Math.max(c, totalRef.current)) : String(c));
    } else {
      setStatus('PAID');
      setCashStr(editing ? '0' : String(totalRef.current));
    }
    setError(null);
    setResult(null);
    setSubmitting(false);
    submittingRef.current = false;
    cashRef.current?.focus();
    cashRef.current?.select();
  }, [isOpen]); // eslint-disable-line react-hooks/exhaustive-deps

  useEffect(() => {
    if (result) doneBtnRef.current?.focus();
  }, [result]);

  const cash = cashStr.trim() === '' ? NaN : parseFloat(cashStr);
  const validation =
    editing && due <= 0
      ? customer == null && appliedAmount > 0
        ? 'Select a customer: later payments were recorded for this bill'
        : null
      : validatePayment(status, due, cash, customer != null);
  const mode = result ? 'result' : 'payment';

  const pickStatus = (next: PaymentStatus) => {
    setStatus(next);
    setError(null);
    if (next === 'PAID') setCashStr(String(totalRef.current));
    else if (next === 'UNPAID') setCashStr('0');
    else setCashStr('');
    if (next !== 'UNPAID') setTimeout(() => cashRef.current?.focus(), 0);
  };

  const submit = async () => {
    if (mode !== 'payment' || submittingRef.current) return;
    if (validation) {
      setError(validation);
      return;
    }
    submittingRef.current = true;
    setSubmitting(true);
    setError(null);
    try {
      const res = await onSubmit({
        paymentStatus: editing && due <= 0 ? 'PAID' : status,
        cashReceived: editing && due <= 0 ? 0 : status === 'UNPAID' ? 0 : round2(cash),
      });
      setResult(res);
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      submittingRef.current = false;
      setSubmitting(false);
    }
  };

  const finish = () => {
    if (result) onDone(result);
  };

  const changeAmount = round2(cash - due);
  const balance = status === 'UNPAID' ? round2(due) : round2(Math.max(due - (Number.isFinite(cash) ? cash : 0), 0));
  const message = error ?? validation;

  const onStatusKeyDown = (e: React.KeyboardEvent) => {
    if (e.key !== 'ArrowLeft' && e.key !== 'ArrowRight') return;
    e.preventDefault();
    const i = STATUSES.indexOf(status);
    const n = e.key === 'ArrowRight' ? (i + 1) % 3 : (i + 2) % 3;
    pickStatus(STATUSES[n]);
    // keep focus on the radio group
    setTimeout(() => {
      document.querySelector<HTMLElement>(`[data-status="${STATUSES[n]}"]`)?.focus();
    }, 0);
  };

  const onBodyKeyDown = (e: React.KeyboardEvent) => {
    if (mode === 'result' && (e.key === ' ' || e.key === 'Enter')) {
      const tag = (e.target as HTMLElement).tagName;
      if (tag !== 'BUTTON') {
        e.preventDefault();
        if (!e.repeat) finish();
      }
    }
  };

  return (
    <Modal
      isOpen={isOpen}
      onClose={mode === 'payment' ? () => { if (!submittingRef.current) onClose(); } : finish}
      title={mode === 'payment' ? (editing ? 'Update bill' : 'Payment') : editing ? 'Bill updated' : 'Sale Complete'}
      description={mode === 'payment' ? 'Nothing is saved until you confirm.' : undefined}
      size="md"
      closeOnBackdrop={false}
      initialFocusRef={mode === 'payment' ? cashRef : doneBtnRef}
      defaultConfirmAction={mode === 'payment' ? () => void submit() : finish}
      footer={
        mode === 'payment' ? (
          <>
            <Button variant="outline" size="sm" onClick={onClose} hotkey="Esc" disabled={submitting}>
              Back to Cart
            </Button>
            <Button
              data-testid="confirm-btn"
              variant="primary"
              size="sm"
              onClick={() => void submit()}
              disabled={submitting || validation != null}
              isLoading={submitting}
              hotkey="Enter"
            >
              {editing ? 'Update bill' : 'Confirm Sale'}
            </Button>
          </>
        ) : (
          <button
            ref={doneBtnRef}
            data-testid="next-bill-btn"
            type="button"
            onClick={finish}
            className="inline-flex items-center justify-center font-medium rounded-md transition-colors select-none focus:outline-none focus:ring-2 focus:ring-blue-500/50 px-3 py-1.5 text-sm gap-2 bg-blue-600 hover:bg-blue-700 text-white shadow-xs cursor-pointer"
          >
            <span>{editing ? 'Done' : 'Next Customer'}</span>
            <kbd className="px-1 py-0.5 rounded bg-black/10 dark:bg-white/10 text-[10px] font-mono font-semibold uppercase">
              Enter
            </kbd>
            <ArrowRight className="h-4 w-4 shrink-0" />
          </button>
        )
      }
    >
      <div className="flex flex-col gap-4" onKeyDown={onBodyKeyDown} data-mode={mode}>
        <div className="flex items-center justify-between text-xs font-mono text-slate-500">
          <span className="font-sans text-slate-600 dark:text-slate-400">
            {customer ? customer.name : 'Walk-in Customer'}
          </span>
          {(result?.transactionCode ?? invoiceCode) && (
            <span
              data-testid="invoice-code"
              className="bg-slate-200 dark:bg-slate-800 px-2 py-0.5 rounded font-semibold text-slate-700 dark:text-slate-300"
            >
              {result?.transactionCode ?? invoiceCode}
            </span>
          )}
        </div>

        {mode === 'payment' && charges && charges.length > 0 && (
          <div data-testid="payment-breakdown" className="rounded-md border border-slate-200 dark:border-slate-700 px-3 py-2">
            <BillTotalsSummary compact itemsSubtotal={itemsSubtotal ?? 0} lines={charges} grandTotal={grandTotal} />
          </div>
        )}

        <div className="flex flex-col items-center justify-center py-2 text-center bg-slate-50 dark:bg-slate-800/40 rounded-lg border border-slate-100 dark:border-slate-800">
          <span className="text-xs font-medium uppercase tracking-wider text-slate-500 dark:text-slate-400 mb-1">
            Grand Total
          </span>
          <div data-testid="grand-total" className="text-4xl font-bold font-mono text-slate-900 dark:text-slate-50">
            <CurrencyDisplay amount={result?.totalAmount ?? grandTotal} size="hero" />
          </div>
        </div>

        {mode === 'payment' && editing && (
          <div data-testid="edit-payment-info" className="flex items-center justify-between text-xs font-mono rounded-md border border-slate-200 dark:border-slate-700 px-3 py-2">
            <span>
              Already received later: <CurrencyDisplay amount={appliedAmount} size="sm" />
            </span>
            <span>
              Due now: <CurrencyDisplay amount={due} size="sm" />
            </span>
          </div>
        )}

        {mode === 'payment' ? (
          <>
            <div
              role="radiogroup"
              aria-label="Payment status"
              onKeyDown={onStatusKeyDown}
              className="flex p-0.5 rounded-lg border border-slate-200 dark:border-slate-700 bg-slate-100 dark:bg-slate-800 text-xs"
            >
              {STATUSES.map((s) => (
                <button
                  key={s}
                  type="button"
                  role="radio"
                  aria-checked={status === s}
                  data-status={s}
                  tabIndex={status === s ? 0 : -1}
                  disabled={submitting}
                  onClick={() => pickStatus(s)}
                  className={cn(
                    'flex-1 px-3 py-1.5 rounded-md font-semibold transition-colors cursor-pointer focus:outline-none focus:ring-2 focus:ring-blue-500/50',
                    status === s
                      ? 'bg-white dark:bg-slate-900 text-blue-600 dark:text-blue-400 shadow-2xs'
                      : 'text-slate-600 dark:text-slate-400 hover:text-slate-900'
                  )}
                >
                  {s}
                </button>
              ))}
            </div>

            {status !== 'PAID' && !customer && (
              <div className="flex flex-col gap-1.5" data-testid="customer-prompt">
                <span className="text-xs text-amber-700 dark:text-amber-400 font-medium">
                  A customer is required for {status} sales. Select one or press Alt+C to create a new customer.
                </span>
                <PartySelector
                  partyType="CUSTOMER"
                  selectedPartyId={null}
                  allowWalkIn={false}
                  hotkeyScope="modal"
                  onSelectParty={(p) =>
                    onCustomerChange?.(p ? { id: p.id, name: p.name, phone: p.phone } : null)
                  }
                />
              </div>
            )}

            <div className="flex flex-col gap-1.5">
              <label htmlFor="sale-cash-received" className="text-xs font-medium text-slate-700 dark:text-slate-300 flex items-center gap-1.5">
                <Banknote className="h-4 w-4 text-slate-400" />
                Cash Received
              </label>
              <input
                id="sale-cash-received"
                ref={cashRef}
                data-testid="cash-received-input"
                type="text"
                inputMode="decimal"
                name="cash"
                autoComplete="off"
                disabled={status === 'UNPAID' || submitting}
                value={cashStr}
                onFocus={(e) => e.currentTarget.select()}
                onChange={(e) => {
                  if (/^\d*\.?\d*$/.test(e.target.value)) {
                    setCashStr(e.target.value);
                    setError(null);
                  }
                }}
                onKeyDown={(e) => {
                  if (e.key === 'Enter' && !e.ctrlKey && !e.metaKey) {
                    e.preventDefault();
                    if (!e.repeat) void submit();
                  }
                }}
                aria-invalid={validation != null && status !== 'UNPAID' ? true : undefined}
                className="w-full rounded-lg border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-900 text-slate-900 dark:text-slate-100 text-xl font-bold font-mono px-4 py-2.5 text-right tabular-nums focus:outline-none focus:ring-2 focus:ring-blue-500 disabled:opacity-50"
              />
            </div>

            <div className="flex items-center justify-between p-3.5 rounded-lg border border-slate-200 dark:border-slate-800 bg-slate-50 dark:bg-slate-800/50">
              <span className="text-xs text-slate-500 dark:text-slate-400 font-medium">
                {status === 'PAID' ? 'Change to Return' : 'Balance Due'}
              </span>
              {status === 'PAID' ? (
                <div
                  data-testid="change-badge"
                  className="bg-emerald-50 dark:bg-emerald-950/60 text-emerald-700 dark:text-emerald-300 border border-emerald-200 dark:border-emerald-800 px-3 py-1 rounded-md text-base font-bold font-mono tabular-nums"
                >
                  ₹{Number.isFinite(changeAmount) ? Math.max(changeAmount, 0).toFixed(2) : '0.00'}
                </div>
              ) : (
                <div
                  data-testid="balance-badge"
                  className="bg-rose-50 dark:bg-rose-950/60 text-rose-700 dark:text-rose-300 border border-rose-200 dark:border-rose-800 px-3 py-1 rounded-md text-base font-bold font-mono tabular-nums"
                >
                  ₹{balance.toFixed(2)}
                </div>
              )}
            </div>

            {message && (
              <p role="alert" data-testid="payment-error" className="text-xs font-medium text-rose-600 dark:text-rose-400">
                {message}
              </p>
            )}
          </>
        ) : (
          <div data-testid="sale-result" className="flex flex-col gap-2 text-sm">
            <div className="flex items-center gap-2 text-emerald-700 dark:text-emerald-400 font-semibold">
              <CheckCircle2 className="h-5 w-5" /> {editing ? 'Updated' : 'Saved as'} {result?.transactionCode}
            </div>
            <dl className="grid grid-cols-2 gap-y-1 font-mono">
              <dt className="text-slate-500">Paid</dt>
              <dd data-testid="result-paid" className="text-right"><CurrencyDisplay amount={result?.paidAmount ?? 0} size="sm" /></dd>
              <dt className="text-slate-500">Balance Due</dt>
              <dd data-testid="result-balance" className="text-right"><CurrencyDisplay amount={result?.balanceDue ?? 0} size="sm" /></dd>
              <dt className="text-slate-500">Change</dt>
              <dd data-testid="result-change" className="text-right"><CurrencyDisplay amount={result?.changeAmount ?? 0} size="sm" /></dd>
            </dl>
          </div>
        )}
      </div>
    </Modal>
  );
};

export default SaleSummaryModal;
