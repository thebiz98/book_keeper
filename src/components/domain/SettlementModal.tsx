import React, { useState, useEffect } from 'react';
import { CreditCard, CheckCircle2 } from 'lucide-react';
import { api, errorMessage } from '@/api';
import { round2 } from '@/lib/money';
import { todayLocal } from '@/lib/dates';
import type { TransactionType } from '@/types';
import { Modal } from '../ui/Modal';
import { Button } from '../ui/Button';
import { NumericInput } from '../ui/NumericInput';
import { Select } from '../ui/Select';
import { CurrencyDisplay } from '../common/CurrencyDisplay';
import { allocateFifo, fetchOpenBills, type OpenBill } from './openBills';

export interface SettlementModalProps {
  isOpen: boolean;
  onClose: () => void;
  partyId: number;
  partyName: string;
  /** Outstanding amount: the party total (lump sum) or the bill balance (single bill). */
  totalDue: number;
  /** SALE for customers (receive), PURCHASE for vendors (pay). */
  transactionType: TransactionType;
  /** Set => settle that bill only. null/undefined => lump sum across open bills, oldest first. */
  transactionId?: number | null;
  /** Display code of the single bill. */
  billCode?: string;
  onSettled?: (amount: number) => void;
}

export const SettlementModal: React.FC<SettlementModalProps> = ({
  isOpen,
  onClose,
  partyId,
  partyName,
  totalDue,
  transactionType,
  transactionId = null,
  billCode,
  onSettled,
}) => {
  const singleBill = transactionId !== null;
  const isCustomer = transactionType === 'SALE';
  const [amount, setAmount] = useState<number>(totalDue);
  const [date, setDate] = useState<string>(todayLocal());
  const [paymentMode, setPaymentMode] = useState<string>('CASH');
  const [notes, setNotes] = useState<string>('');
  const [isLoading, setIsLoading] = useState<boolean>(false);
  const [error, setError] = useState<string | null>(null);
  const [bills, setBills] = useState<OpenBill[]>([]);
  const [billsError, setBillsError] = useState<string | null>(null);

  useEffect(() => {
    if (isOpen) {
      setAmount(totalDue);
      setDate(todayLocal());
      setPaymentMode('CASH');
      setNotes('');
      setError(null);
    }
  }, [isOpen, totalDue, transactionId]);

  useEffect(() => {
    if (!isOpen || singleBill) return;
    let live = true;
    setBillsError(null);
    fetchOpenBills(transactionType, partyId)
      .then((b) => live && setBills(b))
      .catch((e) => {
        if (live) {
          setBills([]);
          setBillsError(errorMessage(e));
        }
      });
    return () => {
      live = false;
    };
  }, [isOpen, singleBill, transactionType, partyId]);

  const validation = (): string | null => {
    if (!(amount > 0)) return 'Settlement amount must be greater than 0';
    if (round2(amount) > round2(totalDue)) {
      return `Settlement amount cannot exceed the outstanding balance (₹${round2(totalDue).toFixed(2)})`;
    }
    if (!date) return 'Settlement date is required';
    return null;
  };

  const handleSettle = async () => {
    const v = validation();
    if (v) {
      setError(v);
      return;
    }
    setIsLoading(true);
    setError(null);
    try {
      await api.settleBillPayment({
        partyId,
        transactionType,
        transactionId,
        amountPaid: round2(amount),
        paymentMode,
        notes: notes.trim() || null,
        settlementDate: date,
      });
      onSettled?.(amount);
      onClose();
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setIsLoading(false);
    }
  };

  const remainingBalance = Math.max(0, round2(totalDue - amount));
  const split = singleBill ? [] : allocateFifo(bills, amount);
  const verb = isCustomer ? 'Receive' : 'Pay';

  return (
    <Modal
      isOpen={isOpen}
      onClose={onClose}
      title={
        singleBill
          ? `${isCustomer ? 'Record Settlement' : 'Record Vendor Payment'}${billCode ? ` - ${billCode}` : ''}`
          : `${verb} on account`
      }
      description={singleBill ? `Settles this bill only - ${partyName}` : `Oldest bills first - ${partyName}`}
      size="md"
      footer={
        <>
          <Button variant="outline" size="sm" onClick={onClose} hotkey="Esc">
            Cancel
          </Button>
          <Button
            variant="primary"
            size="sm"
            onClick={handleSettle}
            isLoading={isLoading}
            leftIcon={<CheckCircle2 className="h-4 w-4" />}
          >
            Record Settlement
          </Button>
        </>
      }
    >
      <div className="flex flex-col gap-4 py-1">
        <div className="flex items-center justify-between p-3 rounded-lg bg-slate-50 dark:bg-slate-800/60 border border-slate-200 dark:border-slate-800">
          <div>
            <span className="text-xs text-slate-500 dark:text-slate-400">
              {singleBill ? 'Bill Balance Due' : 'Total Outstanding'}
            </span>
            <div className="text-xl font-bold font-mono">
              <CurrencyDisplay amount={totalDue} size="xl" colored />
            </div>
          </div>
          <div className="text-right">
            <span className="text-xs text-slate-500 dark:text-slate-400">Remaining After Payment</span>
            <div className="text-xl font-bold font-mono">
              <CurrencyDisplay amount={remainingBalance} size="xl" />
            </div>
          </div>
        </div>

        <div className="grid grid-cols-2 gap-3">
          <NumericInput
            label="Settlement Amount (₹) *"
            aria-label="Settlement amount"
            value={amount}
            onChange={setAmount}
            min={0}
            autoFocus
          />
          <div className="flex flex-col gap-1">
            <label htmlFor="settle-date" className="text-xs font-medium text-slate-700 dark:text-slate-300">
              Settlement Date
            </label>
            <input
              id="settle-date"
              type="date"
              value={date}
              max={todayLocal()}
              onChange={(e) => setDate(e.target.value)}
              className="w-full rounded-md border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-900 text-slate-900 dark:text-slate-100 text-sm px-3 py-1.5 focus:outline-none focus:ring-2 focus:ring-blue-500"
            />
          </div>
          <Select
            label="Payment Mode"
            value={paymentMode}
            onChange={(val) => setPaymentMode(String(val))}
            options={[
              { value: 'CASH', label: 'Cash Drawer' },
              { value: 'UPI', label: 'UPI / QR Code' },
              { value: 'BANK', label: 'Bank Transfer' },
              { value: 'CHEQUE', label: 'Cheque' },
            ]}
          />
          <div className="flex flex-col gap-1">
            <label htmlFor="settle-notes" className="text-xs font-medium text-slate-700 dark:text-slate-300">
              Notes / Reference
            </label>
            <input
              id="settle-notes"
              type="text"
              placeholder="e.g. UPI Ref / receipt no."
              value={notes}
              onChange={(e) => setNotes(e.target.value)}
              className="w-full rounded-md border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-900 text-slate-900 dark:text-slate-100 text-sm px-3 py-1.5 focus:outline-none focus:ring-2 focus:ring-blue-500"
            />
          </div>
        </div>

        {!singleBill && (
          <div data-testid="fifo-preview" className="rounded border border-slate-200 dark:border-slate-800 text-xs">
            <div className="px-2.5 py-1.5 bg-slate-50 dark:bg-slate-800/60 font-semibold text-slate-600 dark:text-slate-300 flex items-center gap-1.5">
              <CreditCard className="h-3.5 w-3.5" /> Split preview (oldest bill first)
            </div>
            {billsError ? (
              <p role="alert" className="p-2.5 text-rose-600 dark:text-rose-400">
                Could not load open bills: {billsError}
              </p>
            ) : split.length === 0 ? (
              <p className="p-2.5 text-slate-500">Enter an amount to see how it is applied.</p>
            ) : (
              <ul className="divide-y divide-slate-100 dark:divide-slate-800">
                {split.map((r) => (
                  <li key={r.bill.id} className="px-2.5 py-1.5 flex items-center justify-between font-mono">
                    <span>
                      {r.bill.transactionCode} <span className="text-slate-400">({r.bill.transactionDate})</span>
                    </span>
                    <span>
                      <CurrencyDisplay amount={r.applied} size="sm" /> of{' '}
                      <CurrencyDisplay amount={r.bill.balanceDue} size="sm" />
                      {r.remainingAfter === 0 ? ' - settled' : ''}
                    </span>
                  </li>
                ))}
              </ul>
            )}
          </div>
        )}

        {error && (
          <p role="alert" className="text-xs text-rose-600 dark:text-rose-400 font-medium">
            {error}
          </p>
        )}
      </div>
    </Modal>
  );
};

export default SettlementModal;
