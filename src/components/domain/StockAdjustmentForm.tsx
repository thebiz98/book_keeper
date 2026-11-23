import React, { useEffect, useState } from 'react';
import { api, errorMessage } from '@/api';
import { round2 } from '@/lib/money';
import { todayLocal } from '@/lib/dates';
import type { AdjustmentReason, StockAdjustmentResult } from '@/types';
import { latestLotCost } from './lotCost';
import { Button } from '../ui/Button';
import { NumericInput } from '../ui/NumericInput';
import { Select } from '../ui/Select';
import { CurrencyDisplay } from '../common/CurrencyDisplay';
import { ItemSearchTypeahead } from './ItemSearchTypeahead';

export interface AdjustmentItem {
  itemId: number;
  name: string;
  currentStock: number;
  baseUnit: string;
}

export const ADJUSTMENT_REASONS: { value: AdjustmentReason; label: string }[] = [
  { value: 'DAMAGED', label: 'Damaged' },
  { value: 'EXPIRED', label: 'Expired' },
  { value: 'COUNT_CORRECTION', label: 'Count correction' },
  { value: 'THEFT_LOST', label: 'Theft / lost' },
  { value: 'UNRECORDED_STOCK', label: 'Unrecorded stock (no purchase)' },
];

export interface StockAdjustmentFormProps {
  /** Pre-selected item (e.g. from Inventory). When omitted the user searches for one. */
  item?: AdjustmentItem | null;
  onAdjusted?: (result: StockAdjustmentResult) => void;
  onCancel?: () => void;
}

export const StockAdjustmentForm: React.FC<StockAdjustmentFormProps> = ({ item: initial = null, onAdjusted, onCancel }) => {
  const [item, setItem] = useState<AdjustmentItem | null>(initial);
  const [date, setDate] = useState(todayLocal());
  const [direction, setDirection] = useState<'DECREASE' | 'INCREASE'>('DECREASE');
  const [quantity, setQuantity] = useState(1);
  const [reason, setReason] = useState<AdjustmentReason>('DAMAGED');
  const [unitCost, setUnitCost] = useState(0);
  const [hasLots, setHasLots] = useState<boolean | null>(null);
  const [lotsError, setLotsError] = useState<string | null>(null);
  const [notes, setNotes] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<StockAdjustmentResult | null>(null);
  const [addedValue, setAddedValue] = useState<number | null>(null);

  useEffect(() => {
    setItem(initial);
  }, [initial?.itemId]); // eslint-disable-line react-hooks/exhaustive-deps

  useEffect(() => {
    setHasLots(null);
    setLotsError(null);
    setUnitCost(0);
    if (!item) return;
    let live = true;
    api
      .getItemLots(item.itemId)
      .then((lots) => {
        if (!live) return;
        setHasLots(lots.length > 0);
        const c = latestLotCost(lots);
        if (c !== null) setUnitCost(c);
      })
      .catch((e) => {
        if (!live) return;
        setHasLots(false);
        setLotsError(errorMessage(e));
      });
    return () => {
      live = false;
    };
  }, [item?.itemId]); // eslint-disable-line react-hooks/exhaustive-deps

  const increase = direction === 'INCREASE';
  const delta = increase ? quantity : -quantity;

  const submit = async () => {
    if (!item) return setError('Select an item first');
    if (!(quantity > 0)) return setError('Quantity must be greater than 0');
    if (!date) return setError('Adjustment date is required');
    if (item.currentStock + delta < 0) {
      return setError(`Resulting stock cannot be negative (current ${item.currentStock} ${item.baseUnit})`);
    }
    if (increase && !(unitCost > 0)) return setError('Unit cost is required for stock increases');
    setBusy(true);
    setError(null);
    try {
      const res = await api.createStockAdjustment({
        adjustmentDate: date,
        itemId: item.itemId,
        quantity: delta,
        reason,
        unitCost: increase ? round2(unitCost) : null,
        notes: notes.trim() || null,
      });
      setAddedValue(increase ? round2(quantity * unitCost) : null);
      setResult(res);
      onAdjusted?.(res);
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  };

  const reset = () => {
    setResult(null);
    setItem(initial);
    setQuantity(1);
    setNotes('');
    setError(null);
  };

  if (result) {
    return (
      <div className="flex flex-col gap-3" data-testid="adjustment-result">
        <p className="text-sm font-semibold text-emerald-700 dark:text-emerald-400">
          Adjustment #{result.adjustmentId} recorded.
        </p>
        {addedValue !== null ? (
          <p className="text-sm">
            Stock value added: <CurrencyDisplay amount={addedValue} colored />
          </p>
        ) : (
          <p className="text-sm">
            Write-off value: <CurrencyDisplay amount={result.lossValue} colored />
          </p>
        )}
        <div className="flex gap-2">
          <Button size="sm" variant="outline" onClick={reset}>
            Adjust another
          </Button>
          {onCancel && (
            <Button size="sm" onClick={onCancel}>
              Close
            </Button>
          )}
        </div>
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-3">
      {item ? (
        <div className="flex items-center justify-between p-2.5 rounded-md bg-slate-50 dark:bg-slate-800 text-sm">
          <div>
            <div className="font-semibold">{item.name}</div>
            <div className="text-xs text-slate-500">
              Current stock: {item.currentStock} {item.baseUnit}
              {item.currentStock + delta !== item.currentStock && ` -> ${round2(item.currentStock + delta)}`}
            </div>
          </div>
          {!initial && (
            <Button size="xs" variant="ghost" onClick={() => setItem(null)}>
              Change
            </Button>
          )}
        </div>
      ) : (
        <div className="flex flex-col gap-1">
          <span className="text-xs font-medium text-slate-700 dark:text-slate-300">Item (name or barcode) *</span>
          <ItemSearchTypeahead
            autoFocus
            placeholder="Search item to adjust..."
            onSelectItem={(r) =>
              setItem({ itemId: r.itemId, name: r.name, currentStock: r.currentStock, baseUnit: r.baseUnit })
            }
          />
        </div>
      )}

      <div className="grid grid-cols-2 gap-3">
        <div className="flex flex-col gap-1">
          <label htmlFor="adj-date" className="text-xs font-medium text-slate-700 dark:text-slate-300">
            Adjustment Date
          </label>
          <input
            id="adj-date"
            type="date"
            value={date}
            max={todayLocal()}
            onChange={(e) => setDate(e.target.value)}
            className="w-full rounded-md border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-900 text-sm px-3 py-1.5"
          />
        </div>
        <Select
          label="Direction"
          value={direction}
          onChange={(v) => {
            const next = v === 'INCREASE' ? 'INCREASE' : 'DECREASE';
            setDirection(next);
            if (next === 'DECREASE' && reason === 'UNRECORDED_STOCK') setReason('COUNT_CORRECTION');
          }}
          options={[
            { value: 'DECREASE', label: 'Decrease (write-off)' },
            { value: 'INCREASE', label: 'Increase (add stock)' },
          ]}
        />
        <NumericInput
          label={`Quantity${item ? ` (${item.baseUnit})` : ''} *`}
          value={quantity}
          onChange={setQuantity}
          min={0}
          precision={3}
        />
        <Select
          label="Reason"
          value={reason}
          onChange={(v) => {
            setReason(v as AdjustmentReason);
            if (v === 'UNRECORDED_STOCK') setDirection('INCREASE');
          }}
          options={increase ? ADJUSTMENT_REASONS : ADJUSTMENT_REASONS.filter((r) => r.value !== 'UNRECORDED_STOCK')}
        />
      </div>

      {increase && (
        <div>
          <NumericInput
            label={`Unit Cost (₹)${hasLots === false ? ' * required (no lots yet)' : ''}`}
            value={unitCost}
            onChange={setUnitCost}
            min={0}
          />
          <p data-testid="valuation-effect" className="text-[11px] text-slate-500 mt-1">
            Adds <CurrencyDisplay amount={round2(quantity * unitCost)} /> to stock value
          </p>
          {lotsError && <p className="text-[11px] text-amber-600 mt-1">Could not load lots: {lotsError}</p>}
        </div>
      )}

      <div className="flex flex-col gap-1">
        <label htmlFor="adj-notes" className="text-xs font-medium text-slate-700 dark:text-slate-300">
          Notes
        </label>
        <input
          id="adj-notes"
          type="text"
          value={notes}
          onChange={(e) => setNotes(e.target.value)}
          className="w-full rounded-md border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-900 text-sm px-3 py-1.5"
        />
      </div>

      {error && (
        <p role="alert" className="text-xs text-rose-600 dark:text-rose-400 font-medium">
          {error}
        </p>
      )}

      <div className="flex justify-end gap-2">
        {onCancel && (
          <Button variant="outline" size="sm" onClick={onCancel}>
            Cancel
          </Button>
        )}
        <Button size="sm" onClick={submit} isLoading={busy}>
          Apply Adjustment
        </Button>
      </div>
    </div>
  );
};

export default StockAdjustmentForm;
