import React, { useEffect, useState } from 'react';
import { api, errorMessage } from '@/api';
import { round2 } from '@/lib/money';
import { todayLocal } from '@/lib/dates';
import type { ItemLot, SetItemStockResult } from '@/types';
import { Modal } from '../ui/Modal';
import { Button } from '../ui/Button';
import { FormActions, enterToSubmit } from '../ui/FormActions';
import { NumericInput } from '../ui/NumericInput';
import { Select } from '../ui/Select';
import { CurrencyDisplay } from '../common/CurrencyDisplay';
import { estimateFifoCost, latestLotCost } from './lotCost';

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

const round3 = (n: number) => Math.round(n * 1000) / 1000;

/**
 * Set stock on hand (FR-08 rev., DD-40). "Set counted stock" sends the counted quantity and lets the
 * backend compute the delta; "Add quantity" posts an UNRECORDED_STOCK adjustment directly.
 */
export const SetStockDialog: React.FC<SetStockDialogProps> = ({ isOpen, onClose, item, onDone }) => {
  const [mode, setMode] = useState<Mode>('SET');
  const [counted, setCounted] = useState(item.currentStock);
  const [addQty, setAddQty] = useState(0);
  const [date, setDate] = useState(todayLocal());
  const [unitCost, setUnitCost] = useState(0);
  const [lots, setLots] = useState<ItemLot[]>([]);
  const [hasLots, setHasLots] = useState<boolean | null>(null);
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
    setUnitCost(0);
    setHasLots(null);
    setLots([]);
    let live = true;
    api
      .getItemLots(item.itemId)
      .then((l) => {
        if (!live) return;
        setLots(l);
        setHasLots(l.length > 0);
        const c = latestLotCost(l);
        if (c !== null) setUnitCost(c);
      })
      .catch(() => {
        if (live) setHasLots(false);
      });
    return () => {
      live = false;
    };
  }, [isOpen, item.itemId]); // eslint-disable-line react-hooks/exhaustive-deps

  const delta = mode === 'SET' ? round3(counted - item.currentStock) : addQty;
  const up = delta > 0;
  const down = delta < 0;

  const submit = async () => {
    if (delta === 0 || busy) return;
    if (!date) return setError('Date is required');
    if (mode === 'SET' && counted < 0) return setError('Counted stock cannot be negative');
    if (mode === 'ADD' && !(addQty > 0)) return setError('Quantity must be greater than 0');
    if (up && !(unitCost > 0)) return setError('Unit cost is required for unrecorded stock');
    setBusy(true);
    setError(null);
    try {
      const noteVal = notes.trim() || null;
      let res: SetItemStockResult;
      if (mode === 'SET') {
        res = await api.setItemStock({
          itemId: item.itemId,
          countedQuantity: counted,
          unitCost: up ? round2(unitCost) : null,
          adjustmentDate: date,
          notes: noteVal,
        });
      } else {
        const r = await api.createStockAdjustment({
          adjustmentDate: date,
          itemId: item.itemId,
          quantity: addQty,
          reason: 'UNRECORDED_STOCK',
          unitCost: round2(unitCost),
          notes: noteVal,
        });
        res = {
          adjustmentId: r.adjustmentId,
          delta: addQty,
          reason: 'UNRECORDED_STOCK',
          previousStock: item.currentStock,
          newStock: round3(item.currentStock + addQty),
          unitCost: round2(unitCost),
          valueChange: round2(addQty * unitCost),
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
      canSave={delta !== 0}
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
          {up && (
            <NumericInput
              label={`Unit Cost (₹)${hasLots === false ? ' * required (no lots yet)' : ''}`}
              value={unitCost}
              onChange={setUnitCost}
              min={0}
            />
          )}
          <div
            data-testid="set-stock-preview"
            className="rounded-md bg-slate-50 dark:bg-slate-800 px-3 py-2 text-sm"
          >
            {up && (
              <span>
                +{delta} {item.baseUnit} · Unrecorded stock · Adds <CurrencyDisplay amount={round2(delta * unitCost)} /> to
                stock value
              </span>
            )}
            {down && (
              <span>
                −{-delta} {item.baseUnit} · Count correction · Estimated write-off ≈{' '}
                <CurrencyDisplay amount={estimateFifoCost(lots, -delta)} /> (oldest lots first)
              </span>
            )}
            {delta === 0 && <span>No change</span>}
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
