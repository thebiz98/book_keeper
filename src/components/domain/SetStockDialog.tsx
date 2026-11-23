import React, { useEffect, useMemo, useState } from 'react';
import { api, errorMessage } from '@/api';
import { usePreview } from '@/hooks/usePreview';
import { todayLocal } from '@/lib/dates';
import type { SetItemStockPayload, SetItemStockResult, StockAdjustmentPayload } from '@/types';
import { Modal } from '../ui/Modal';
import { Button } from '../ui/Button';
import { FormActions, enterToSubmit } from '../ui/FormActions';
import { NumericInput } from '../ui/NumericInput';
import { Select } from '../ui/Select';
import { CurrencyDisplay } from '../common/CurrencyDisplay';

export interface SetStockItem {
  itemId: number;
  name: string;
  currentStock: number;
  baseUnit: string;
}

export interface SetStockDialogProps {
  isOpen: boolean;
  onClose: () => void;
  item: SetStockItem;
  onDone?: (result: SetItemStockResult) => void;
}

type Mode = 'SET' | 'ADD';

type StockInput = { mode: 'SET'; payload: SetItemStockPayload } | { mode: 'ADD'; payload: StockAdjustmentPayload };
/** What the dialog shows, whichever command was previewed. */
interface StockView {
  delta: number;
  newStock: number;
  unitCost: number | null;
  valueChange: number;
}

/**
 * Set stock on hand (FR-08 rev., DD-40). "Set counted stock" sends the counted quantity and lets the
 * backend compute the delta; "Add quantity" posts an UNRECORDED_STOCK adjustment directly. The delta,
 * the default unit cost (latest lot), the FIFO loss and the value change all come from
 * `preview_set_item_stock` / `preview_stock_adjustment` (DD-56).
 */
export const SetStockDialog: React.FC<SetStockDialogProps> = ({ isOpen, onClose, item, onDone }) => {
  const [mode, setMode] = useState<Mode>('SET');
  const [counted, setCounted] = useState(item.currentStock);
  const [addQty, setAddQty] = useState(0);
  const [date, setDate] = useState(todayLocal());
  /** Typed unit cost; null = not typed, the backend uses the latest lot cost. */
  const [typedCost, setTypedCost] = useState<number | null>(null);
  const [notes, setNotes] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<SetItemStockResult | null>(null);

  useEffect(() => {
    if (!isOpen) return;
    setMode('SET');
    setCounted(item.currentStock);
    setAddQty(0);
    setDate(todayLocal());
    setNotes('');
    setError(null);
    setResult(null);
    setTypedCost(null);
  }, [isOpen, item.itemId]); // eslint-disable-line react-hooks/exhaustive-deps

  const cost = typedCost !== null && typedCost > 0 ? typedCost : null;
  const noteVal = notes.trim() || null;
  const input = useMemo<StockInput | null>(() => {
    if (!isOpen || !date) return null;
    if (mode === 'SET') {
      return { mode, payload: { itemId: item.itemId, countedQuantity: counted, unitCost: cost, adjustmentDate: date, notes: noteVal } };
    }
    if (!(addQty > 0)) return null;
    return {
      mode,
      payload: { adjustmentDate: date, itemId: item.itemId, quantity: addQty, reason: 'UNRECORDED_STOCK', unitCost: cost, notes: noteVal },
    };
  }, [isOpen, mode, item.itemId, counted, addQty, date, cost, noteVal]);

  const preview = usePreview<StockInput, StockView>({
    input,
    fetcher: async (i) => {
      if (i.mode === 'SET') {
        const r = await api.previewSetItemStock(i.payload);
        return { delta: r.delta, newStock: r.newStock, unitCost: r.unitCost, valueChange: r.valueChange };
      }
      const r = await api.previewStockAdjustment(i.payload);
      return { delta: i.payload.quantity, newStock: r.newStock, unitCost: r.unitCost, valueChange: r.valueChange };
    },
  });
  const view = preview.data;
  const delta = view?.delta ?? 0;
  const up = delta > 0;
  const down = delta < 0;
  const hasInput = input !== null;
  const canSave = hasInput && !preview.isLoading && view !== null && preview.errors.length === 0 && delta !== 0;

  const submit = async () => {
    if (!canSave || busy || !input) return;
    setBusy(true);
    setError(null);
    try {
      let res: SetItemStockResult;
      if (input.mode === 'SET') {
        res = await api.setItemStock(input.payload);
      } else {
        const r = await api.createStockAdjustment(input.payload);
        res = {
          adjustmentId: r.adjustmentId,
          delta: input.payload.quantity,
          reason: 'UNRECORDED_STOCK',
          previousStock: item.currentStock,
          newStock: view?.newStock ?? item.currentStock,
          unitCost: view?.unitCost ?? null,
          valueChange: view?.valueChange ?? 0,
        };
      }
      setResult(res);
      onDone?.(res);
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  };

  const footer = result ? (
    <Button size="sm" onClick={onClose}>
      Close
    </Button>
  ) : (
    <FormActions
      onSave={() => void submit()}
      onCancel={onClose}
      isSubmitting={busy}
      canSave={canSave}
      saveLabel="Save stock"
      saveHotkey="Ctrl+Enter"
    />
  );

  return (
    <Modal
      isOpen={isOpen}
      onClose={onClose}
      title="Set stock on hand"
      description={`${item.name} - system stock ${item.currentStock} ${item.baseUnit}`}
      size="md"
      footer={footer}
    >
      {result ? (
        <div className="flex flex-col gap-2 py-1" data-testid="set-stock-result">
          {result.delta === 0 ? (
            <p className="text-sm">No change: stock is already {result.newStock} {item.baseUnit}.</p>
          ) : (
            <>
              <p className="text-sm font-semibold text-emerald-700 dark:text-emerald-400">
                Stock is now {result.newStock} {item.baseUnit}.
              </p>
              <p className="text-sm">
                Stock value change: <CurrencyDisplay amount={result.valueChange} colored />
              </p>
            </>
          )}
        </div>
      ) : (
        <div className="flex flex-col gap-3 py-1" onKeyDown={enterToSubmit(() => void submit())}>
          <Select
            label="Mode"
            value={mode}
            onChange={(v) => setMode(v === 'ADD' ? 'ADD' : 'SET')}
            options={[
              { value: 'SET', label: 'Set counted stock' },
              { value: 'ADD', label: 'Add quantity' },
            ]}
          />
          <div className="grid grid-cols-2 gap-3">
            {mode === 'SET' ? (
              <NumericInput
                label={`Counted stock (${item.baseUnit}) *`}
                value={counted}
                onChange={setCounted}
                min={0}
                precision={3}
              />
            ) : (
              <NumericInput
                label={`Quantity to add (${item.baseUnit}) *`}
                value={addQty}
                onChange={setAddQty}
                min={0}
                precision={3}
              />
            )}
            <div className="flex flex-col gap-1">
              <label htmlFor="setstock-date" className="text-xs font-medium text-slate-700 dark:text-slate-300">
                Date
              </label>
              <input
                id="setstock-date"
                type="date"
                value={date}
                max={todayLocal()}
                onChange={(e) => setDate(e.target.value)}
                className="w-full rounded-md border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-900 text-sm px-3 py-1.5"
              />
            </div>
          </div>
          {(mode === 'ADD' || counted > item.currentStock) && (
            <NumericInput
              label="Unit Cost (₹)"
              value={typedCost ?? view?.unitCost ?? 0}
              onChange={setTypedCost}
              min={0}
            />
          )}
          <div
            data-testid="set-stock-preview"
            className="rounded-md bg-slate-50 dark:bg-slate-800 px-3 py-2 text-sm"
          >
            {preview.errors.length > 0 && (
              <span role="alert" data-testid="stock-preview-error" className="text-rose-600 dark:text-rose-400">
                {preview.errors[0].message}
              </span>
            )}
            {preview.errors.length === 0 && up && (
              <span>
                +{delta} {item.baseUnit} · Unrecorded stock · Adds <CurrencyDisplay amount={view?.valueChange ?? 0} /> to
                stock value
              </span>
            )}
            {preview.errors.length === 0 && down && (
              <span>
                −{-delta} {item.baseUnit} · Count correction · Write-off ≈{' '}
                <CurrencyDisplay amount={-(view?.valueChange ?? 0)} /> (oldest lots first)
              </span>
            )}
            {preview.errors.length === 0 && view !== null && delta === 0 && <span>No change</span>}
          </div>
          <div className="flex flex-col gap-1">
            <label htmlFor="setstock-notes" className="text-xs font-medium text-slate-700 dark:text-slate-300">
              Notes
            </label>
            <input
              id="setstock-notes"
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
        </div>
      )}
    </Modal>
  );
};

export default SetStockDialog;
