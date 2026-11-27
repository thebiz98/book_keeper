import React, { useEffect, useMemo, useState } from 'react';
import { api, errorMessage } from '@/api';
import { usePreview } from '@/hooks/usePreview';
import { todayLocal } from '@/lib/dates';
import type {
  SetItemStockPayload,
  SetItemStockResult,
  SetStockMode,
  SetStockOption,
  StockAdjustmentPayload,
  UndoableWriteoff,
} from '@/types';
import { Modal } from '../ui/Modal';
import { Button } from '../ui/Button';
import { FormActions, enterToSubmit } from '../ui/FormActions';
import { NumericInput } from '../ui/NumericInput';
import { Select } from '../ui/Select';
import { CurrencyDisplay } from '../common/CurrencyDisplay';
import { UndoWriteoffHint } from './UndoWriteoffHint';

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
  /** Stock changed without a Save (an earlier write-off was undone from the hint). */
  onStockChanged?: () => void;
}

type Mode = 'SET' | 'ADD';

type StockInput = { mode: 'SET'; payload: SetItemStockPayload } | { mode: 'ADD'; payload: StockAdjustmentPayload };
/** What the dialog shows, whichever command was previewed. */
interface StockView {
  delta: number;
  previousStock: number;
  newStock: number;
  unitCost: number | null;
  valueChange: number;
  /** The path the backend applied or preselected; null while a choice is needed. */
  path: SetStockMode | null;
  options: SetStockOption[];
  choiceRequired: boolean;
  undoableWriteoffs: UndoableWriteoff[];
}

/**
 * Set stock on hand (FR-08 rev., DD-40, FR-57). "Set counted stock" sends the counted quantity and lets
 * the backend compute the delta; "Add quantity" posts an UNRECORDED_STOCK adjustment directly. The delta,
 * the default unit cost, the FIFO loss, the value change, the paths offered ("Opening stock was entered
 * wrong" or a stock adjustment), which are enabled and why not, and the write-offs to undo instead all
 * come from `preview_set_item_stock` / `preview_stock_adjustment` (DD-56). With two enabled paths the user
 * must pick one: there is no silent default to the loss.
 */
export const SetStockDialog: React.FC<SetStockDialogProps> = ({ isOpen, onClose, item, onDone, onStockChanged }) => {
  const [mode, setMode] = useState<Mode>('SET');
  const [counted, setCounted] = useState(item.currentStock);
  const [addQty, setAddQty] = useState(0);
  const [date, setDate] = useState(todayLocal());
  /** Typed unit cost; null = not typed, the backend uses its default cost. A typed 0 is a real cost (S4). */
  const [typedCost, setTypedCost] = useState<number | null>(null);
  const [notes, setNotes] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<SetItemStockResult | null>(null);
  /** The path the user picked; null = none picked (the backend preselects when only one is enabled). */
  const [picked, setPicked] = useState<SetStockMode | null>(null);
  const [refreshKey, setRefreshKey] = useState(0);

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
    setPicked(null);
  }, [isOpen, item.itemId]); // eslint-disable-line react-hooks/exhaustive-deps

  // A new count can change the direction and the paths, so the user picks again.
  useEffect(() => setPicked(null), [counted, mode]);

  const noteVal = notes.trim() || null;
  const input = useMemo<StockInput | null>(() => {
    if (!isOpen || !date) return null;
    if (mode === 'SET') {
      return {
        mode,
        payload: { itemId: item.itemId, countedQuantity: counted, unitCost: typedCost, adjustmentDate: date, notes: noteVal, mode: picked },
      };
    }
    if (!(addQty > 0)) return null;
    return {
      mode,
      payload: { adjustmentDate: date, itemId: item.itemId, quantity: addQty, reason: 'UNRECORDED_STOCK', unitCost: typedCost, notes: noteVal },
    };
  }, [isOpen, mode, item.itemId, counted, addQty, date, typedCost, noteVal, picked]);

  const preview = usePreview<StockInput, StockView>({
    input,
    refreshKey,
    fetcher: async (i) => {
      if (i.mode === 'SET') {
        const r = await api.previewSetItemStock(i.payload);
        return {
          delta: r.delta,
          previousStock: r.previousStock,
          newStock: r.newStock,
          unitCost: r.unitCost,
          valueChange: r.valueChange,
          path: r.mode ?? null,
          options: r.options ?? [],
          choiceRequired: r.choiceRequired ?? false,
          undoableWriteoffs: r.undoableWriteoffs ?? [],
        };
      }
      const r = await api.previewStockAdjustment(i.payload);
      return {
        delta: i.payload.quantity,
        previousStock: r.previousStock,
        newStock: r.newStock,
        unitCost: r.unitCost,
        valueChange: r.valueChange,
        path: 'ADJUSTMENT',
        options: [],
        choiceRequired: false,
        undoableWriteoffs: r.undoableWriteoffs ?? [],
      };
    },
  });
  const view = preview.data;
  const delta = view?.delta ?? 0;
  const up = delta > 0;
  const down = delta < 0;
  const hasInput = input !== null;
  const options = view?.options ?? [];
  const showChoice = mode === 'SET' && options.length > 1;
  const selected: SetStockMode | null = picked ?? view?.path ?? null;
  const selectedOption = options.find((o) => o.mode === selected) ?? null;
  const choiceNeeded = view?.choiceRequired ?? false;
  const opening = selected === 'OPENING_CORRECTION';
  // Which path takes the typed cost is the backend's (takesUnitCost); before a path is known, a count
  // above the system stock shows the field so it can be typed straight away.
  const showCost =
    mode === 'ADD' || (selectedOption ? selectedOption.takesUnitCost : counted > (view?.previousStock ?? item.currentStock));
  const canSave =
    hasInput && !preview.isLoading && view !== null && preview.errors.length === 0 && delta !== 0 && !choiceNeeded;

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
          previousStock: view?.previousStock ?? item.currentStock,
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

  const systemStock = view?.previousStock ?? item.currentStock;

  return (
    <Modal
      isOpen={isOpen}
      onClose={onClose}
      title="Set stock on hand"
      description={`${item.name} - system stock ${systemStock} ${item.baseUnit}`}
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
              {result.mode === 'OPENING_CORRECTION' && <p className="text-sm">Opening stock corrected. No loss was booked.</p>}
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

          {showChoice && (
            <fieldset data-testid="set-stock-paths" className="flex flex-col gap-1.5">
              <legend className="text-xs font-medium text-slate-700 dark:text-slate-300 mb-1">
                Why is the stock different? *
              </legend>
              {options.map((o) => (
                <label
                  key={o.mode}
                  data-testid={`set-stock-path-${o.mode}`}
                  className={`flex items-start gap-2 rounded-md border px-2.5 py-1.5 text-sm ${
                    o.enabled
                      ? 'border-slate-300 dark:border-slate-700 cursor-pointer'
                      : 'border-slate-200 dark:border-slate-800 opacity-70 cursor-not-allowed'
                  } ${selected === o.mode ? 'ring-2 ring-blue-500/40' : ''}`}
                >
                  <input
                    type="radio"
                    name="set-stock-path"
                    value={o.mode}
                    checked={selected === o.mode}
                    disabled={!o.enabled}
                    onChange={() => setPicked(o.mode)}
                    className="mt-1"
                  />
                  <span className="flex flex-col">
                    <span className="font-medium">{o.label}</span>
                    <span className="text-xs text-slate-500">{o.detail}</span>
                    {!o.enabled && o.disabledReason && (
                      <span data-testid="set-stock-path-disabled" className="text-xs text-amber-700 dark:text-amber-400">
                        {o.disabledReason}
                      </span>
                    )}
                  </span>
                </label>
              ))}
            </fieldset>
          )}

          {showCost && (
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
            {preview.errors.length === 0 && choiceNeeded && (
              <span data-testid="set-stock-choose">
                {up ? '+' : '−'}
                {Math.abs(delta)} {item.baseUnit} · Choose why the stock is different to see the effect.
              </span>
            )}
            {preview.errors.length === 0 && !choiceNeeded && opening && delta !== 0 && (
              <span>
                {up ? '+' : '−'}
                {Math.abs(delta)} {item.baseUnit} · Opening stock corrected · No loss · Stock value{' '}
                <CurrencyDisplay amount={view?.valueChange ?? 0} colored />
              </span>
            )}
            {preview.errors.length === 0 && !choiceNeeded && !opening && up && (
              <span>
                +{delta} {item.baseUnit} · Unrecorded stock · Adds <CurrencyDisplay amount={view?.valueChange ?? 0} /> to
                stock value
              </span>
            )}
            {preview.errors.length === 0 && !choiceNeeded && !opening && down && (
              <span>
                −{-delta} {item.baseUnit} · Count correction · Write-off ≈{' '}
                <CurrencyDisplay amount={-(view?.valueChange ?? 0)} /> (oldest lots first)
              </span>
            )}
            {preview.errors.length === 0 && view !== null && delta === 0 && <span>No change</span>}
          </div>
          {preview.errors.length === 0 && (
            <UndoWriteoffHint
              writeoffs={view?.undoableWriteoffs}
              baseUnit={item.baseUnit}
              lead={
                down
                  ? 'These write-offs used the opening stock. Undo one to correct the opening stock without a loss.'
                  : undefined
              }
              onUndone={() => {
                setRefreshKey((k) => k + 1);
                onStockChanged?.();
              }}
            />
          )}
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
