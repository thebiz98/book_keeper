import React, { useState, useEffect, useRef, useMemo } from 'react';
import { CheckCircle2, ArrowRight, Banknote, Printer } from 'lucide-react';
import { usePrintBill } from '../print/usePrintBill';
import { api, errorMessage } from '@/api';
import type { BillChargeLine } from '@/lib/billMath';
import type { BillPreview, PaymentStatus, PreviewSalePayload, SaleSummaryResponse } from '@/types';
import type { CustomerInput } from '@/types/store';
import { useBusyLock } from '@/hooks/useHotkeys';
import { usePreview } from '@/hooks/usePreview';
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
  /**
   * The bill being paid, as the preview input without a payment (customer included); null while
   * the cart is empty. The dialog adds the chosen status and cash and asks `preview_sale` (DD-56):
   * totals, change, balance and every error come from the backend.
   */
  bill: PreviewSalePayload | null;
  /** The no-payment preview of the same bill (totals, open amount); null while it is pending. */
  totals: BillPreview | null;
  /** Charge/discount lines (names only; amounts shown are the backend's) for the breakdown. */
  charges?: BillChargeLine[];
  customer: CustomerInput | null;
  /** Lets the dialog pick / create a customer when the backend asks for one. */
  onCustomerChange?: (customer: CustomerInput | null) => void;
  /** Performs the sale. Rejections are shown inside the dialog. */
  onSubmit: (values: SalePaymentValues) => Promise<SaleSummaryResponse>;
  /** Result mode: Enter / Space starts the next bill. */
  onDone: (result: SaleSummaryResponse) => void;
  /** Display-only code preview shown while paying. */
  invoiceCode?: string;
}

const STATUSES: PaymentStatus[] = ['PAID', 'PARTIAL', 'UNPAID'];

export const SaleSummaryModal: React.FC<SaleSummaryModalProps> = ({
  isOpen,
  onClose,
  bill,
  totals,
  charges,
  customer,
  onCustomerChange,
  onSubmit,
  onDone,
  invoiceCode,
}) => {
  // FR-47: the item extra charge / discount lines come from the backend preview, after the manual ones.
  const breakdown = useMemo(
    () => [...(charges ?? []), ...(totals?.adjustmentLines ?? [])],
    [charges, totals?.adjustmentLines]
  );
  const [status, setStatus] = useState<PaymentStatus>('PAID');
  const [cashStr, setCashStr] = useState('');
  /** True once the cashier typed in the cash box: the PAID prefill then stops following the total. */
  const [cashTouched, setCashTouched] = useState(false);
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<SaleSummaryResponse | null>(null);

  const cashRef = useRef<HTMLInputElement>(null);
  const doneBtnRef = useRef<HTMLButtonElement>(null);
  const submittingRef = useRef(false);
  const openAmountRef = useRef(totals?.openAmount ?? 0);
  openAmountRef.current = totals?.openAmount ?? 0;

  useBusyLock(submitting);

  // Reset only when the dialog opens: never while the cashier is typing.
  useEffect(() => {
    if (!isOpen) return;
    setStatus('PAID');
    setCashTouched(false);
    setCashStr(String(openAmountRef.current));
    setError(null);
    setResult(null);
    setSubmitting(false);
    submittingRef.current = false;
    cashRef.current?.focus();
    cashRef.current?.select();
  }, [isOpen]); // eslint-disable-line react-hooks/exhaustive-deps

  // A PAID sale prefills the cash with the amount due, which the backend reports (it can still be pending at open).
  const openAmount = totals?.openAmount;
  useEffect(() => {
    if (isOpen && status === 'PAID' && !cashTouched && openAmount != null) setCashStr(String(openAmount));
  }, [isOpen, status, cashTouched, openAmount]);

  useEffect(() => {
    if (result) doneBtnRef.current?.focus();
  }, [result]);

  const mode = result ? 'result' : 'payment';
  const cash = cashStr.trim() === '' ? NaN : parseFloat(cashStr);
  /** Format check only; every other rule is the backend's. */
  const formatError = status !== 'UNPAID' && !Number.isFinite(cash) ? 'Enter the cash received' : null;

  const payInput = useMemo<PreviewSalePayload | null>(
    () =>
      bill && isOpen && mode === 'payment' && !formatError
        ? { ...bill, paymentStatus: status, paidAmount: status === 'UNPAID' ? 0 : cash }
        : null,
    [bill, isOpen, mode, formatError, status, cash]
  );
  const preview = usePreview<PreviewSalePayload, BillPreview>({
    input: payInput,
    fetcher: (p) => api.previewSale(p, null),
  });
  const payment = preview.data?.payment ?? null;
  const previewErrors = preview.errors;
  const validation = formatError ?? previewErrors[0]?.message ?? null;
  /** Saving waits for the preview of exactly what is on screen and needs it to be clean. */
  const canSubmit = !formatError && !preview.isLoading && previewErrors.length === 0 && payment != null;
  const needsCustomer = previewErrors.some((e) => e.field === 'party_id' || e.field === 'PARTY_REQUIRED_WITH_PAYMENTS');

  const pickStatus = (next: PaymentStatus) => {
    setStatus(next);
    setError(null);
    setCashTouched(false);
    if (next === 'PAID') setCashStr(String(openAmountRef.current));
    else if (next === 'UNPAID') setCashStr('0');
    else setCashStr('');
    if (next !== 'UNPAID') setTimeout(() => cashRef.current?.focus(), 0);
  };

  const submit = async () => {
    if (mode !== 'payment' || submittingRef.current) return;
    if (!canSubmit) {
      setError(validation);
      return;
    }
    submittingRef.current = true;
    setSubmitting(true);
    setError(null);
    try {
      const res = await onSubmit({
        paymentStatus: status,
        cashReceived: status === 'UNPAID' ? 0 : cash,
      });
      setResult(res);
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      submittingRef.current = false;
      setSubmitting(false);
    }
  };
  const [printError, setPrintError] = useState<string | null>(null);
  const { printById } = usePrintBill();
  const doPrint = () => {
    if (!result) return;
    setPrintError(null);
    printById('SALE', result.saleId).catch((e) => setPrintError(errorMessage(e)));
  };
  const printRef = useRef(doPrint);
  printRef.current = doPrint;
  const showingResult = result !== null;
  useEffect(() => {
    if (!showingResult) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.ctrlKey || e.metaKey || e.altKey || e.repeat || e.key.toLowerCase() !== 'p') return;
      const tag = (e.target as HTMLElement | null)?.tagName;
      if (tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT') return;
      e.preventDefault();
      printRef.current();
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [showingResult]);

  const finish = () => {
    if (result) onDone(result);
  };


  const changeAmount = payment?.changeAmount;
  const balance = payment?.balanceDue;
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
      title={mode === 'payment' ? 'Payment' : 'Sale Complete'}
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
              disabled={submitting || !canSubmit}
              isLoading={submitting}
              hotkey="Enter"
            >
              Confirm Sale
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
            <span>Next Customer</span>
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

        {mode === 'payment' && breakdown.length > 0 && (
          <div data-testid="payment-breakdown" className="rounded-md border border-slate-200 dark:border-slate-700 px-3 py-2">
            <BillTotalsSummary
              compact
              itemsSubtotal={totals?.itemsSubtotal ?? 0}
              lines={breakdown}
              grandTotal={totals?.totalAmount ?? 0}
            />
          </div>
        )}

        <div className="flex flex-col items-center justify-center py-2 text-center bg-slate-50 dark:bg-slate-800/40 rounded-lg border border-slate-100 dark:border-slate-800">
          <span className="text-xs font-medium uppercase tracking-wider text-slate-500 dark:text-slate-400 mb-1">
            Grand Total
          </span>
          <div data-testid="grand-total" className="text-4xl font-bold font-mono text-slate-900 dark:text-slate-50">
            <CurrencyDisplay amount={result?.totalAmount ?? totals?.totalAmount ?? 0} size="hero" />
          </div>
        </div>

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

            {needsCustomer && !customer && (
              <div className="flex flex-col gap-1.5" data-testid="customer-prompt">
                <span className="text-xs text-amber-700 dark:text-amber-400 font-medium">
                  {previewErrors[0]?.message} Select a customer or press Alt+C to create a new one.
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
                    setCashTouched(true);
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
                  {changeAmount != null ? `₹${changeAmount.toFixed(2)}` : '—'}
                </div>
              ) : (
                <div
                  data-testid="balance-badge"
                  className="bg-rose-50 dark:bg-rose-950/60 text-rose-700 dark:text-rose-300 border border-rose-200 dark:border-rose-800 px-3 py-1 rounded-md text-base font-bold font-mono tabular-nums"
                >
                  {balance != null ? `₹${balance.toFixed(2)}` : '—'}
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
              <CheckCircle2 className="h-5 w-5" /> Saved as {result?.transactionCode}
            </div>
            <dl className="grid grid-cols-2 gap-y-1 font-mono">
              <dt className="text-slate-500">Paid</dt>
              <dd data-testid="result-paid" className="text-right"><CurrencyDisplay amount={result?.paidAmount ?? 0} size="sm" /></dd>
              <dt className="text-slate-500">Balance Due</dt>
              <dd data-testid="result-balance" className="text-right"><CurrencyDisplay amount={result?.balanceDue ?? 0} size="sm" /></dd>
              <dt className="text-slate-500">Change</dt>
              <dd data-testid="result-change" className="text-right"><CurrencyDisplay amount={result?.changeAmount ?? 0} size="sm" /></dd>
            </dl>
            <div className="flex items-center gap-2">
              <Button
                variant="outline"
                size="sm"
                data-testid="print-bill-btn"
                leftIcon={<Printer className="h-3.5 w-3.5" />}
                onClick={doPrint}
                hotkey="P"
              >
                Print bill
              </Button>
              {printError && (
                <span role="alert" className="text-xs text-rose-600 dark:text-rose-400">
                  {printError}
                </span>
              )}
            </div>
          </div>
        )}
      </div>
    </Modal>
  );
};

export default SaleSummaryModal;
