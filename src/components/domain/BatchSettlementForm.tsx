import React, { useEffect, useMemo, useState } from 'react';
import { api, errorMessage } from '@/api';
import { usePreview } from '@/hooks/usePreview';
import { todayLocal } from '@/lib/dates';
import type { SettleAllocation, TransactionType } from '@/types';
import { NumericInput } from '../ui/NumericInput';
import { Select } from '../ui/Select';
import { FormActions, enterToSubmit } from '../ui/FormActions';
import { CurrencyDisplay } from '../common/CurrencyDisplay';
import { fetchOpenBills, type OpenBill } from './openBills';

export interface BatchRow {
  checked: boolean;
  amount: number;
}
export type BatchRows = Record<number, BatchRow>;

/** The backend's split (`preview_settlement_allocation`) as rows; bills it gives nothing are unticked. */
export function rowsFromAllocations(bills: OpenBill[], allocations: SettleAllocation[]): BatchRows {
  const applied = new Map(allocations.map((a) => [a.transactionId, a.amountPaid]));
  const rows: BatchRows = {};
  for (const b of bills) {
    const a = applied.get(b.id) ?? 0;
    rows[b.id] = { checked: a > 0, amount: a };
  }
  return rows;
}

/** Rows -> amount: the sum of the ticked rows the user entered (display; the backend validates on save). */
export function amountFromRows(rows: BatchRows): number {
  return Object.values(rows).reduce((s, r) => s + (r.checked ? r.amount : 0), 0);
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
        // Start by settling every open bill in full.
        const all: BatchRows = {};
        for (const x of b) all[x.id] = { checked: true, amount: x.balanceDue };
        setBills(b);
        setAmount(amountFromRows(all));
        setRows(all);
        setLoaded(true);
      })
      .catch((e) => live && setBillsError(errorMessage(e)));
    return () => {
      live = false;
    };
  }, [transactionType, partyId]);

  const outstanding = bills.reduce((s, b) => s + b.balanceDue, 0); // display

  /** Set while the Amount box drives the rows; typing it asks the backend for the oldest-first split. */
  const [typed, setTyped] = useState<number | null>(null);
  const splitInput = useMemo(() => (typed !== null && typed > 0 && loaded ? { partyId, transactionType, amount: typed } : null), [typed, loaded, partyId, transactionType]);
  const split = usePreview<{ partyId: number; transactionType: TransactionType; amount: number }, SettleAllocation[]>({
    input: splitInput,
    fetcher: (i) => api.previewSettlementAllocation(i.partyId, i.transactionType, i.amount),
  });
  useEffect(() => {
    if (typed !== null && split.data) setRows(rowsFromAllocations(bills, split.data));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [split.data]);

  const onAmount = (v: number) => {
    setAmount(v);
    setTyped(v);
    if (!(v > 0)) setRows(rowsFromAllocations(bills, []));
  };
  const onRow = (bill: OpenBill, patch: Partial<BatchRow>) => {
    const cur = rows[bill.id] ?? { checked: false, amount: 0 };
    let next = { ...cur, ...patch };
    if (patch.amount !== undefined) {
      next.checked = patch.amount > 0;
    } else if (patch.checked !== undefined) {
      next = patch.checked ? { checked: true, amount: bill.balanceDue } : { checked: false, amount: 0 };
    }
    const nextRows = { ...rows, [bill.id]: next };
    setTyped(null);
    setRows(nextRows);
    setAmount(amountFromRows(nextRows));
  };

  const ticked = bills.filter((b) => rows[b.id]?.checked);
  const total = amountFromRows(rows);
  const rowInvalid = ticked.some((b) => !(rows[b.id].amount > 0)); // format check
  const message = typed !== null ? (split.errors[0]?.message ?? null) : rowInvalid ? 'Every ticked bill needs an amount above 0' : null;
  const canSave = loaded && total > 0 && !rowInvalid && !split.isLoading && split.errors.length === 0 && !!date;

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
        allocations: ticked.map((b) => ({ transactionId: b.id, amountPaid: rows[b.id].amount })),
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
            <CurrencyDisplay amount={outstanding - total} size="xl" />
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
                    <td className="font-mono">
                      {b.transactionCode}
                      {b.isOpeningBalance && (
                        <span data-testid="opening-label" className="ml-1.5 font-sans text-[11px] text-slate-500">
                          Opening balance
                        </span>
                      )}
                    </td>
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
