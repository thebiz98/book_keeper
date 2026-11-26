import React, { useEffect, useMemo, useState } from 'react';
import { api, errorMessage } from '@/api';
import { usePreview } from '@/hooks/usePreview';
import type { ItemLot, OpeningQuantityResult, UpdateOpeningQuantityPayload } from '@/types';
import { Modal } from '../ui/Modal';
import { NumericInput } from '../ui/NumericInput';
import { FormActions, enterToSubmit } from '../ui/FormActions';
import { CurrencyDisplay } from '../common/CurrencyDisplay';
import { refreshAppMetrics } from './navParams';

export interface OpeningQtyEditModalProps {
  /** null = closed. Only OPENING lots are passed in. */
  lot: ItemLot | null;
  baseUnit: string;
  onClose: () => void;
  /** Called after a successful save; the caller reloads its lots and item. */
  onSaved: () => void;
}

/**
 * FR-51 / DD-84: correct the quantity entered as opening stock. Not a loss and not an adjustment.
 * The minimum (already used), the new stock and the value change come from `preview_opening_lot_quantity`
 * (DD-56); every refusal is the backend's message.
 */
export const OpeningQtyEditModal: React.FC<OpeningQtyEditModalProps> = ({ lot, baseUnit, onClose, onSaved }) => {
  const [qty, setQty] = useState(0);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const lotId = lot?.purchaseItemId ?? null;

  useEffect(() => {
    if (!lot) return;
    setQty(lot.quantity);
    setError(null);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [lotId]);

  /** Format check only: a number, 0 or more. */
  const problem = !Number.isFinite(qty) || qty < 0 ? 'Quantity must be 0 or more' : null;
  const previewInput = useMemo<UpdateOpeningQuantityPayload | null>(
    () => (lot && !problem ? { purchaseItemId: lot.purchaseItemId, quantity: qty } : null),
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [lotId, qty, problem]
  );
  const preview = usePreview<UpdateOpeningQuantityPayload, OpeningQuantityResult>({
    input: previewInput,
    fetcher: (i) => api.previewOpeningLotQuantity(i),
  });
  const result = preview.data;
  const changed = result != null && result.newQuantity !== result.oldQuantity;
  const canSave = !problem && !preview.isLoading && result != null && preview.errors.length === 0 && changed;

  const save = async () => {
    if (!lot || !canSave || saving) return;
    setSaving(true);
    setError(null);
    try {
      await api.updateOpeningLotQuantity({ purchaseItemId: lot.purchaseItemId, quantity: qty });
      onSaved();
      refreshAppMetrics();
      onClose();
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setSaving(false);
    }
  };

  return (
    <Modal
      isOpen={lot !== null}
      onClose={onClose}
      title="Edit Opening Quantity"
      description={lot ? `${lot.transactionCode} on ${lot.transactionDate}` : undefined}
      size="md"
    >
      {lot && (
        <div className="flex flex-col gap-4 py-1" onKeyDown={enterToSubmit(save)} data-testid="opening-qty-edit">
          <div className="grid grid-cols-3 gap-3 text-xs">
            <div>
              <div className="text-[10px] uppercase tracking-wide text-slate-400">Opening quantity</div>
              <div className="font-mono">
                {lot.quantity} {baseUnit}
              </div>
            </div>
            <div>
              <div className="text-[10px] uppercase tracking-wide text-slate-400">Already used</div>
              <div className="font-mono" data-testid="opening-used">
                {result ? result.consumed : lot.quantity - lot.remainingQuantity} {baseUnit}
              </div>
            </div>
            <div>
              <div className="text-[10px] uppercase tracking-wide text-slate-400">Minimum allowed</div>
              <div className="font-mono" data-testid="opening-min">
                {result ? result.consumed : lot.quantity - lot.remainingQuantity} {baseUnit}
              </div>
            </div>
          </div>
          <NumericInput label={`New opening quantity (${baseUnit}) *`} aria-label="Opening quantity" value={qty} onChange={setQty} min={0} autoFocus />
          {preview.errors.length === 0 && result && changed && (
            <div className="text-xs rounded-md border border-slate-200 dark:border-slate-800 p-3 flex flex-col gap-1" data-testid="opening-preview">
              <div className="flex justify-between">
                <span className="text-slate-500">Stock on hand</span>
                <span className="font-mono">
                  {result.oldStock} <span className="text-slate-400">to</span> <span className="font-semibold">{result.newStock}</span> {baseUnit}
                </span>
              </div>
              <div className="flex justify-between">
                <span className="text-slate-500">Stock value</span>
                <span>
                  {result.valueChange >= 0 ? '+' : '-'}
                  <CurrencyDisplay amount={Math.abs(result.valueChange)} size="sm" />
                </span>
              </div>
            </div>
          )}
          <p className="text-[11px] text-slate-500">Not recorded as a loss or adjustment.</p>
          {preview.errors.map((e, i) => (
            <p key={i} role="alert" className="text-xs text-rose-600 dark:text-rose-400 font-medium">
              {e.message}
            </p>
          ))}
          {(problem || error) && (
            <p role="alert" className="text-xs text-rose-600 dark:text-rose-400 font-medium">
              {error ?? problem}
            </p>
          )}
          <FormActions onSave={save} onCancel={onClose} isSubmitting={saving} canSave={canSave} />
        </div>
      )}
    </Modal>
  );
};

export default OpeningQtyEditModal;
