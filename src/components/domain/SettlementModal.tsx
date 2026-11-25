import React, { useState, useEffect } from 'react';
import { CheckCircle2 } from 'lucide-react';
import { api, errorMessage } from '@/api';
import { todayLocal } from '@/lib/dates';
import type { TransactionType } from '@/types';
import { Modal } from '../ui/Modal';
import { Button } from '../ui/Button';
import { NumericInput } from '../ui/NumericInput';
import { Select } from '../ui/Select';
import { CurrencyDisplay } from '../common/CurrencyDisplay';
import { BatchSettlementForm } from './BatchSettlementForm';

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

const SingleBillSettlement: React.FC<SettlementModalProps> = ({
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
  const isCustomer = transactionType === 'SALE';
  const [amount, setAmount] = useState<number>(totalDue);
  const [date, setDate] = useState<string>(todayLocal());
  const [paymentMode, setPaymentMode] = useState<string>('CASH');
  const [notes, setNotes] = useState<string>('');
  const [isLoading, setIsLoading] = useState<boolean>(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (isOpen) {
      setAmount(totalDue);
      setDate(todayLocal());
      setPaymentMode('CASH');
      setNotes('');
      setError(null);
    }
  }, [isOpen, totalDue, transactionId]);

  const validation = (): string | null => {
    if (!(amount > 0)) return 'Settlement amount must be greater than 0';
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
        amountPaid: amount,
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

  const remainingBalance = totalDue - amount; // display only

  return (
    <Modal
      isOpen={isOpen}
      onClose={onClose}
      title={`${isCustomer ? 'Record Settlement' : 'Record Vendor Payment'}${billCode ? ` - ${billCode}` : ''}`}
      description={`Settles this bill only - ${partyName}`}
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
              Bill Balance Due
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

        {error && (
          <p role="alert" className="text-xs text-rose-600 dark:text-rose-400 font-medium">
            {error}
          </p>
        )}
      </div>
    </Modal>
  );
};

/**
 * Per-bill settle (transactionId set) keeps the single-amount form; the lump sum is the
 * "Settle bills" batch form (D9).
 */
export const SettlementModal: React.FC<SettlementModalProps> = (props) => {
  if (props.transactionId != null) return <SingleBillSettlement {...props} />;
  const verb = props.transactionType === 'SALE' ? 'Receive' : 'Pay';
  return (
    <Modal
      isOpen={props.isOpen}
      onClose={props.onClose}
      title={`Settle bills - ${verb.toLowerCase()} on account`}
      description={`Oldest bills first - ${props.partyName}`}
      size="lg"
    >
      <BatchSettlementForm
        transactionType={props.transactionType}
        partyId={props.partyId}
        partyName={props.partyName}
        onClose={props.onClose}
        onSettled={props.onSettled}
      />
    </Modal>
  );
};

export default SettlementModal;
