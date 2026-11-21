import React, { useState, useEffect, useRef } from 'react';
import { CheckCircle2, ArrowRight, Banknote } from 'lucide-react';
import { CurrencyDisplay } from '../common/CurrencyDisplay';
import { Button } from '../ui/Button';

export interface SaleSummaryModalProps {
  isOpen: boolean;
  onClose: () => void;
  onConfirm?: () => void;
  grandTotal: number;
  invoiceCode?: string;
  customerName?: string;
  defaultCashReceived?: number;
}

export const SaleSummaryModal: React.FC<SaleSummaryModalProps> = ({
  isOpen,
  onClose,
  onConfirm,
  grandTotal = 0,
  invoiceCode,
  customerName,
  defaultCashReceived,
}) => {
  const [cashReceived, setCashReceived] = useState<number>(
    defaultCashReceived !== undefined ? defaultCashReceived : grandTotal
  );

  const cashInputRef = useRef<HTMLInputElement>(null);
  const dialogRef = useRef<HTMLDivElement>(null);
  const confirmBtnRef = useRef<HTMLButtonElement>(null);

  const onCloseRef = useRef(onClose);
  onCloseRef.current = onClose;
  const onConfirmRef = useRef(onConfirm);
  onConfirmRef.current = onConfirm;

  useEffect(() => {
    if (isOpen) {
      const initialCash =
        defaultCashReceived !== undefined ? defaultCashReceived : grandTotal;
      setCashReceived(initialCash);

      // Autofocus cash input immediately
      setTimeout(() => {
        if (cashInputRef.current) {
          cashInputRef.current.focus();
          cashInputRef.current.select();
        }
      }, 50);
    }
  }, [isOpen, defaultCashReceived, grandTotal]);

  const handleDismiss = () => {
    if (onConfirmRef.current) {
      onConfirmRef.current();
    } else {
      onCloseRef.current();
    }
  };

  const handleDismissRef = useRef(handleDismiss);
  handleDismissRef.current = handleDismiss;

  // Native DOM keyboard listeners for Escape, Enter, Space
  useEffect(() => {
    if (!isOpen) return;

    const dialogEl = dialogRef.current;
    const confirmBtnEl = confirmBtnRef.current;

    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        e.preventDefault();
        onCloseRef.current();
      } else if (e.key === 'Enter') {
        e.preventDefault();
        handleDismissRef.current();
      } else if (e.key === ' ' || e.code === 'Space') {
        const target = e.target as HTMLElement;
        if (!target || (target.tagName !== 'INPUT' && target.tagName !== 'TEXTAREA')) {
          e.preventDefault();
          handleDismissRef.current();
        }
      }
    };

    const handleBtnKeyDown = (e: KeyboardEvent) => {
      if (e.key === ' ' || e.code === 'Space' || e.key === 'Enter') {
        e.preventDefault();
        handleDismissRef.current();
      }
    };

    dialogEl?.addEventListener('keydown', handleKeyDown);
    confirmBtnEl?.addEventListener('keydown', handleBtnKeyDown);

    return () => {
      dialogEl?.removeEventListener('keydown', handleKeyDown);
      confirmBtnEl?.removeEventListener('keydown', handleBtnKeyDown);
    };
  }, [isOpen]);

  if (!isOpen) return null;

  const changeAmount = cashReceived - grandTotal;
  const isOverOrExact = changeAmount >= 0;

  return (
    <div
      className="fixed inset-0 bg-black/60 backdrop-blur-xs z-50 flex items-center justify-center p-4 overflow-y-auto"
      onClick={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <div
        ref={dialogRef}
        role="dialog"
        aria-modal="true"
        tabIndex={-1}
        className="bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 rounded-xl shadow-2xl w-full max-w-md overflow-hidden animate-in fade-in zoom-in-95 duration-150 flex flex-col focus:outline-none"
      >
        {/* Header */}
        <div className="flex items-center justify-between px-6 py-4 border-b border-slate-100 dark:border-slate-800 bg-slate-50 dark:bg-slate-900/60">
          <div className="flex items-center gap-2">
            <CheckCircle2 className="h-5 w-5 text-emerald-600 dark:text-emerald-400" />
            <h2 className="text-base font-semibold text-slate-900 dark:text-slate-100">
              Sale Complete
            </h2>
          </div>
          <div className="flex items-center gap-2 text-xs font-mono text-slate-500">
            {invoiceCode && (
              <span className="bg-slate-200 dark:bg-slate-800 px-2 py-0.5 rounded font-semibold text-slate-700 dark:text-slate-300">
                {invoiceCode}
              </span>
            )}
            {customerName && (
              <span className="font-sans text-slate-600 dark:text-slate-400">
                {customerName}
              </span>
            )}
          </div>
        </div>

        {/* Content Body */}
        <div className="p-6 flex flex-col gap-5">
          {/* Hero Grand Total */}
          <div className="flex flex-col items-center justify-center py-2 text-center bg-slate-50 dark:bg-slate-800/40 rounded-lg border border-slate-100 dark:border-slate-800">
            <span className="text-xs font-medium uppercase tracking-wider text-slate-500 dark:text-slate-400 mb-1">
              Grand Total
            </span>
            <div data-testid="grand-total" className="text-4xl font-bold font-mono text-slate-900 dark:text-slate-50">
              <CurrencyDisplay amount={grandTotal} size="hero" />
            </div>
          </div>

          {/* Cash Tendered Input */}
          <div className="flex flex-col gap-1.5">
            <label className="text-xs font-medium text-slate-700 dark:text-slate-300 flex items-center justify-between">
              <span className="flex items-center gap-1.5">
                <Banknote className="h-4 w-4 text-slate-400" />
                Cash Tendered / Received
              </span>
              <span className="text-[10px] text-slate-400 uppercase font-mono">F2 auto-focused</span>
            </label>
            <input
              ref={cashInputRef}
              data-testid="cash-received-input"
              type="number"
              name="cash"
              autoFocus
              value={cashReceived === 0 ? '' : cashReceived}
              onChange={(e) => {
                const val = parseFloat(e.target.value);
                setCashReceived(isNaN(val) ? 0 : val);
              }}
              onInput={(e) => {
                const val = parseFloat((e.target as HTMLInputElement).value);
                setCashReceived(isNaN(val) ? 0 : val);
              }}
              className="w-full rounded-lg border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-900 text-slate-900 dark:text-slate-100 text-xl font-bold font-mono px-4 py-2.5 text-right tabular-nums focus:outline-none focus:ring-2 focus:ring-blue-500"
            />
          </div>

          {/* Live Change Calculation Breakdown */}
          <div className="flex items-center justify-between p-3.5 rounded-lg border border-slate-200 dark:border-slate-800 bg-slate-50 dark:bg-slate-800/50">
            <div className="flex flex-col">
              <span className="text-xs text-slate-500 dark:text-slate-400 font-medium">
                {isOverOrExact ? 'Change to Return' : 'Shortage / Due Balance'}
              </span>
              <span className="text-sm font-semibold text-slate-700 dark:text-slate-300">
                {isOverOrExact ? 'Return to customer' : 'Customer balance short'}
              </span>
            </div>

            {isOverOrExact ? (
              <div
                data-testid="change-badge"
                className="bg-emerald-50 dark:bg-emerald-950/60 text-emerald-700 dark:text-emerald-300 border border-emerald-200 dark:border-emerald-800 px-3 py-1 rounded-md text-base font-bold font-mono tabular-nums flex items-center gap-1"
              >
                <span>₹</span>
                <span>{Math.abs(changeAmount).toFixed(2)}</span>
              </div>
            ) : (
              <div
                data-testid="shortage-badge"
                className="bg-rose-50 dark:bg-rose-950/60 text-rose-700 dark:text-rose-300 border border-rose-200 dark:border-rose-800 px-3 py-1 rounded-md text-base font-bold font-mono tabular-nums flex items-center gap-1"
              >
                <span>Short: ₹</span>
                <span>{Math.abs(changeAmount).toFixed(2)}</span>
              </div>
            )}
          </div>
        </div>

        {/* Footer */}
        <div className="px-6 py-4 bg-slate-50 dark:bg-slate-900/60 border-t border-slate-100 dark:border-slate-800 flex items-center justify-between">
          <Button variant="outline" size="sm" onClick={onClose} hotkey="Esc">
            Close
          </Button>

          <button
            ref={confirmBtnRef}
            data-testid="confirm-btn"
            type="button"
            onClick={handleDismiss}
            className="inline-flex items-center justify-center font-medium rounded-md transition-colors select-none focus:outline-none focus:ring-2 focus:ring-blue-500/50 px-3 py-1.5 text-sm gap-2 bg-blue-600 hover:bg-blue-700 text-white shadow-xs cursor-pointer"
          >
            <span>Next Customer</span>
            <kbd className="px-1 py-0.5 rounded bg-black/10 dark:bg-white/10 text-[10px] font-mono font-semibold uppercase">
              Enter
            </kbd>
            <ArrowRight className="h-4 w-4 shrink-0" />
          </button>
        </div>
      </div>
    </div>
  );
};

export default SaleSummaryModal;
