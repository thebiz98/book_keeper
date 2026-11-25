import React, { useEffect, useState } from 'react';
import { api, errorMessage } from '@/api';
import { todayLocal } from '@/lib/dates';
import type { PaymentSettlement } from '@/types';
import { Modal } from '../ui/Modal';
import { NumericInput } from '../ui/NumericInput';
import { Select } from '../ui/Select';
import { FormActions, enterToSubmit } from '../ui/FormActions';

export interface SettlementEditModalProps {
  /** null = closed. */
  settlement: PaymentSettlement | null;
  /** Display code of the bill the row is booked against. */
  billCode?: string;
  /** Highest allowed amount: the bill's balance plus this row's current amount. */
  /** Unused: the cap is enforced by the backend on save (DD-56). */
  maxAmount?: number;
  onClose: () => void;
  /** Called after a successful save; the caller reloads and refreshes the app metrics. */
  onSaved: () => void;
}

/** D8: correct one SETTLEMENT row (amount, date, mode, notes). */
export const SettlementEditModal: React.FC<SettlementEditModalProps> = ({
  settlement,
  billCode,
  onClose,
  onSaved,
}) => {
  const [amount, setAmount] = useState(0);
  const [date, setDate] = useState('');
  const [mode, setMode] = useState('CASH');
  const [notes, setNotes] = useState('');
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!settlement) return;
    setAmount(settlement.amountPaid);
    setDate(settlement.settlementDate);
    setMode(settlement.paymentMode ?? 'CASH');
    setNotes(settlement.notes ?? '');
    setError(null);
  }, [settlement]);

  // Format checks only; the cap against the bill's balance is the backend's (shown if save is refused).
  const problem = !(amount > 0) ? 'Amount must be greater than 0' : !date ? 'Date is required' : null;

  const save = async () => {
    if (!settlement || problem || saving) return;
    setSaving(true);
    setError(null);
    try {
      await api.updateSettlement(settlement.id, {
        amountPaid: amount,
        settlementDate: date,
        paymentMode: mode,
        notes: notes.trim() || null,
      });
      onSaved();
      onClose();
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setSaving(false);
    }
  };

  const inputCls =
    'w-full rounded-md border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-900 text-slate-900 dark:text-slate-100 text-sm px-3 py-1.5 focus:outline-none focus:ring-2 focus:ring-blue-500';

  return (
    <Modal
      isOpen={settlement !== null}
      onClose={onClose}
      title={`Edit payment${billCode ? ` - ${billCode}` : ''}`}
      description="Corrects this payment only. The bill balance and cash records follow."
      size="md"
    >
      <div className="flex flex-col gap-4 py-1" onKeyDown={enterToSubmit(save)} data-testid="settlement-edit">
        <div className="grid grid-cols-2 gap-3">
          <NumericInput
            label="Amount (₹) *"
            aria-label="Settlement amount"
            value={amount}
            onChange={setAmount}
            min={0}
            autoFocus
          />
          <div className="flex flex-col gap-1">
            <label htmlFor="edit-settle-date" className="text-xs font-medium text-slate-700 dark:text-slate-300">
              Date
            </label>
            <input
              id="edit-settle-date"
              type="date"
              value={date}
              max={todayLocal()}
              onChange={(e) => setDate(e.target.value)}
              className={inputCls}
            />
          </div>
          <Select
            label="Payment Mode"
            value={mode}
            onChange={(v) => setMode(String(v))}
            options={[
              { value: 'CASH', label: 'Cash Drawer' },
              { value: 'UPI', label: 'UPI / QR Code' },
              { value: 'BANK', label: 'Bank Transfer' },
              { value: 'CHEQUE', label: 'Cheque' },
            ]}
          />
          <div className="flex flex-col gap-1">
            <label htmlFor="edit-settle-notes" className="text-xs font-medium text-slate-700 dark:text-slate-300">
              Notes / Reference
            </label>
            <input
              id="edit-settle-notes"
              type="text"
              value={notes}
              onChange={(e) => setNotes(e.target.value)}
              className={inputCls}
            />
          </div>
        </div>
        {(problem || error) && (
          <p role="alert" className="text-xs text-rose-600 dark:text-rose-400 font-medium">
            {error ?? problem}
          </p>
        )}
        <FormActions onSave={save} onCancel={onClose} isSubmitting={saving} canSave={!problem} />
      </div>
    </Modal>
  );
};

export default SettlementEditModal;
