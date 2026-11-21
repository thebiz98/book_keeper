import React, { useState, useEffect } from 'react';
import { CreditCard, CheckCircle2 } from 'lucide-react';
import { api } from '@/api';
import { Modal } from '../ui/Modal';
import { Button } from '../ui/Button';
import { NumericInput } from '../ui/NumericInput';
import { Select } from '../ui/Select';
import { CurrencyDisplay } from '../common/CurrencyDisplay';

export interface SettlementModalProps {
  isOpen: boolean;
  onClose: () => void;
  partyId: number;
  partyName: string;
  totalDue: number;
  onSettled?: (amount: number) => void;
}

export const SettlementModal: React.FC<SettlementModalProps> = ({
  isOpen,
  onClose,
  partyId,
  partyName,
  totalDue,
  onSettled,
}) => {
  const [amount, setAmount] = useState<number>(totalDue);
  const [paymentMode, setPaymentMode] = useState<string>('CASH');
  const [notes, setNotes] = useState<string>('');
  const [isLoading, setIsLoading] = useState<boolean>(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (isOpen) {
      setAmount(totalDue);
      setPaymentMode('CASH');
      setNotes('');
      setError(null);
    }
  }, [isOpen, totalDue]);

  const handleSettle = async () => {
    if (amount <= 0) {
      setError('Settlement amount must be greater than 0');
      return;
    }
    if (amount > totalDue) {
      setError(`Settlement amount cannot exceed total outstanding due (₹${totalDue})`);
      return;
    }

    setIsLoading(true);
    setError(null);

    try {
      await api.settleBillPayment({
        partyId,
        amount,
        paymentMode,
        notes: notes.trim() || null,
        specificBillId: null,
        settlementDate: new Date().toISOString().split('T')[0],
      });
      onSettled?.(amount);
      onClose();
    } catch (err: any) {
      setError(err?.message || 'Failed to settle debt payment');
    } finally {
      setIsLoading(false);
    }
  };

  const remainingBalance = Math.max(0, totalDue - amount);

  return (
    <Modal
      isOpen={isOpen}
      onClose={onClose}
      title="Settle Credit Khata Balance"
      description={`FIFO payment allocation for ${partyName}`}
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
            hotkey="Ctrl+Enter"
            leftIcon={<CheckCircle2 className="h-4 w-4" />}
          >
            Record Settlement
          </Button>
        </>
      }
    >
      <div className="flex flex-col gap-4 py-1">
        {/* Outstanding Dues Summary Header */}
        <div className="flex items-center justify-between p-3 rounded-lg bg-slate-50 dark:bg-slate-800/60 border border-slate-200 dark:border-slate-800">
          <div>
            <span className="text-xs text-slate-500 dark:text-slate-400">Total Outstanding Due</span>
            <div className="text-xl font-bold font-mono text-rose-600 dark:text-rose-400">
              <CurrencyDisplay amount={totalDue} size="xl" colored />
            </div>
          </div>
          <div className="text-right">
            <span className="text-xs text-slate-500 dark:text-slate-400">Remaining After Payment</span>
            <div className="text-xl font-bold font-mono text-slate-900 dark:text-slate-100">
              <CurrencyDisplay amount={remainingBalance} size="xl" />
            </div>
          </div>
        </div>

        {/* Inputs */}
        <div className="grid grid-cols-2 gap-3">
          <NumericInput
            label="Payment Amount (₹) *"
            value={amount}
            onChange={setAmount}
            min={1}
            max={totalDue}
            autoFocus
          />

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
        </div>

        <div className="flex flex-col gap-1">
          <label className="text-xs font-medium text-slate-700 dark:text-slate-300">
            Notes / Reference
          </label>
          <input
            type="text"
            placeholder="e.g. UPI Ref / Cash receipt details..."
            value={notes}
            onChange={(e) => setNotes(e.target.value)}
            className="w-full rounded-md border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-900 text-slate-900 dark:text-slate-100 text-sm px-3 py-1.5 focus:outline-none focus:ring-2 focus:ring-blue-500"
          />
        </div>

        {/* FIFO allocation explanation note */}
        <div className="flex items-start gap-2 p-2.5 rounded bg-blue-50/50 dark:bg-blue-950/20 border border-blue-100 dark:border-blue-900/40 text-[11px] text-blue-700 dark:text-blue-300">
          <CreditCard className="h-4 w-4 shrink-0 mt-0.5" />
          <span>
            Payment will automatically extinguish earliest unpaid invoices first according to FIFO
            principles.
          </span>
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

export default SettlementModal;
