import React, { useEffect, useState } from 'react';
import { api, errorMessage } from '@/api';
import { round2 } from '@/lib/money';
import { todayLocal } from '@/lib/dates';
import type { TransactionType } from '@/types';
import { NumericInput } from '../ui/NumericInput';
import { Select } from '../ui/Select';
import { FormActions, enterToSubmit } from '../ui/FormActions';
import { CurrencyDisplay } from '../common/CurrencyDisplay';
import { allocateFifo, fetchOpenBills, type OpenBill } from './openBills';

export interface BatchRow {
  checked: boolean;
  amount: number;
}
export type BatchRows = Record<number, BatchRow>;

/** Amount -> rows: oldest-first fill, bills that get nothing are unticked. */
export function rowsFromAmount(bills: OpenBill[], amount: number): BatchRows {
  const applied = new Map(allocateFifo(bills, amount).map((r) => [r.bill.id, r.applied]));
  const rows: BatchRows = {};
  for (const b of bills) {
    const a = applied.get(b.id) ?? 0;
    rows[b.id] = { checked: a > 0, amount: a };
  }
  return rows;
}

/** Rows -> amount: the sum of the ticked rows. */
export function amountFromRows(rows: BatchRows): number {
  return round2(Object.values(rows).reduce((s, r) => s + (r.checked ? r.amount : 0), 0));
}

export interface BatchSettlementFormProps {
  transactionType: TransactionType;
  partyId: number;
  partyName: string;
  onClose: () => void;
  onSettled?: (amount: number) => void;
}

/**
 * "Settle bills" (D9). Two-way sync without effects: typing the Amount re-runs the oldest-first
 * fill; ticking or editing a row (capped at its balance) rewrites the Amount. Each handler
 * updates both sides itself, so there is no update loop.
 */
export const BatchSettlementForm: React.FC<BatchSettlementFormProps> = ({
  transactionType,
  partyId,
  partyName,
  onClose,
  onSettled,
}) => {
  const isCustomer = transactionType === 'SALE';
  const [bills, setBills] = useState<OpenBill[]>([]);
  const [loaded, setLoaded] = useState(false);
  const [billsError, setBillsError] = useState<string | null>(null);
  const [rows, setRows] = useState<BatchRows>({});
  const [amount, setAmount] = useState(0);
  const [date, setDate] = useState(todayLocal());
  const [paymentMode, setPaymentMode] = useState('CASH');
  const [notes, setNotes] = useState('');
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let live = true;
    fetchOpenBills(transactionType, partyId)
      .then((b) => {
        if (!live) return;
        const full = round2(b.reduce((s, x) => s + x.balanceDue, 0));
        setBills(b);
        setAmount(full);
        setRows(rowsFromAmount(b, full));
        setLoaded(true);
      })
      .catch((e) => live && setBillsError(errorMessage(e)));
    return () => {
      live = false;
    };
  }, [transactionType, partyId]);

  const outstanding = round2(bills.reduce((s, b) => s + b.balanceDue, 0));

  const onAmount = (v: number) => {
    setAmount(v);
    setRows(rowsFromAmount(bills, v));
  };
  const onRow = (bill: OpenBill, patch: Partial<BatchRow>) => {
    const cur = rows[bill.id] ?? { checked: false, amount: 0 };
    let next = { ...cur, ...patch };
    if (patch.amount !== undefined) {
      next.amount = round2(Math.min(Math.max(0, patch.amount), bill.balanceDue));
      next.checked = next.amount > 0;
    } else if (patch.checked !== undefined) {
      next = patch.checked ? { checked: true, amount: bill.balanceDue } : { checked: false, amount: 0 };
    }
    const nextRows = { ...rows, [bill.id]: next };
    setRows(nextRows);
    setAmount(amountFromRows(nextRows));
  };

  const ticked = bills.filter((b) => rows[b.id]?.checked);
  const rowInvalid = ticked.some((b) => {
    const a = rows[b.id].amount;
    return !(a > 0) || round2(a) > round2(b.balanceDue);
  });
  const overTotal = round2(amount) > outstanding;
  const message = overTotal
    ? `Amount cannot exceed the total outstanding (₹${outstanding.toFixed(2)})`
    : rowInvalid
      ? 'Every ticked bill needs an amount above 0 and within its balance'
      : null;
  const total = amountFromRows(rows);
  const canSave = loaded && total > 0 && !rowInvalid && !overTotal && !!date && ticked.length <= 200;

  const save = async () => {
    if (!canSave || saving) return;
    setSaving(true);
    setError(null);
    try {
      await api.settleBillsBatch({
        partyId,
        transactionType,
        settlementDate: date,
        paymentMode,
        notes: notes.trim() || null,
        allocations: ticked.map((b) => ({ transactionId: b.id, amountPaid: round2(rows[b.id].amount) })),
      });
      onSettled?.(total);
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
    <div className="flex flex-col gap-4 py-1" onKeyDown={enterToSubmit(save)} data-testid="batch-settlement">
      <div className="flex items-center justify-between p-3 rounded-lg bg-slate-50 dark:bg-slate-800/60 border border-slate-200 dark:border-slate-800">
        <div>
          <span className="text-xs text-slate-500 dark:text-slate-400">
            Total Outstanding - {partyName}
          </span>
          <div className="text-xl font-bold font-mono">
            <CurrencyDisplay amount={outstanding} size="xl" colored />
          </div>
        </div>
        <div className="text-right">
          <span className="text-xs text-slate-500 dark:text-slate-400">Remaining After Payment</span>
          <div className="text-xl font-bold font-mono">
            <CurrencyDisplay amount={Math.max(0, round2(outstanding - total))} size="xl" />
          </div>
        </div>
      </div>

      <div className="grid grid-cols-2 gap-3">
        <NumericInput
          label={`${isCustomer ? 'Receive' : 'Pay'} Amount (₹) *`}
          aria-label="Settlement amount"
          value={amount}
          onChange={onAmount}
          min={0}
          autoFocus
        />
        <div className="flex flex-col gap-1">
          <label htmlFor="batch-date" className="text-xs font-medium text-slate-700 dark:text-slate-300">
            Settlement Date
          </label>
          <input
            id="batch-date"
            type="date"
            value={date}
            max={todayLocal()}
            onChange={(e) => setDate(e.target.value)}
            className={inputCls}
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
          <label htmlFor="batch-notes" className="text-xs font-medium text-slate-700 dark:text-slate-300">
            Notes / Reference
          </label>
          <input
            id="batch-notes"
            type="text"
            placeholder="e.g. UPI Ref / receipt no."
            value={notes}
            onChange={(e) => setNotes(e.target.value)}
            className={inputCls}
          />
        </div>
      </div>

      <div data-testid="batch-rows" className="rounded border border-slate-200 dark:border-slate-800 text-xs max-h-64 overflow-auto">
        {billsError ? (
          <p role="alert" className="p-2.5 text-rose-600 dark:text-rose-400">
            Could not load open bills: {billsError}
          </p>
        ) : !loaded ? (
          <p className="p-2.5 text-slate-500">Loading open bills...</p>
        ) : bills.length === 0 ? (
          <p className="p-2.5 text-slate-500">No open bills.</p>
        ) : (
          <table className="w-full">
            <thead className="bg-slate-50 dark:bg-slate-800/60 text-slate-500 sticky top-0">
              <tr className="text-left">
                <th className="px-2 py-1 w-8"></th>
                <th className="py-1">Code</th>
                <th>Date</th>
                <th className="text-right">Total</th>
                <th className="text-right">Balance</th>
                <th className="text-right pr-2 w-32">Amount</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-slate-100 dark:divide-slate-800">
              {bills.map((b) => {
                const r = rows[b.id] ?? { checked: false, amount: 0 };
                return (
                  <tr key={b.id} data-testid={`batch-row-${b.id}`}>
                    <td className="px-2 py-1">
                      <input
                        type="checkbox"
                        aria-label={`Settle ${b.transactionCode}`}
                        checked={r.checked}
                        onChange={(e) => onRow(b, { checked: e.target.checked })}
                      />
                    </td>
                    <td className="font-mono">{b.transactionCode}</td>
                    <td className="font-mono text-slate-500">{b.transactionDate}</td>
                    <td className="text-right font-mono">
                      <CurrencyDisplay amount={b.totalAmount} size="sm" />
                    </td>
                    <td className="text-right font-mono">
                      <CurrencyDisplay amount={b.balanceDue} size="sm" />
                    </td>
                    <td className="py-1 pr-2">
                      <NumericInput
                        aria-label={`Amount for ${b.transactionCode}`}
                        value={r.amount}
                        onChange={(v) => onRow(b, { amount: v })}
                        min={0}
                        max={b.balanceDue}
                      />
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        )}
      </div>

      {(message || error) && (
        <p role="alert" className="text-xs text-rose-600 dark:text-rose-400 font-medium">
          {error ?? message}
        </p>
      )}

      <FormActions
        onSave={save}
        onCancel={onClose}
        saveLabel="Settle bills"
        isSubmitting={saving}
        canSave={canSave}
      />
    </div>
  );
};

export default BatchSettlementForm;
