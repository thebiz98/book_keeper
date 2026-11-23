import React, { useEffect, useMemo, useState } from 'react';
import { api, errorMessage } from '@/api';
import { usePreview } from '@/hooks/usePreview';
import type { ItemLot, LotPriceUpdateResult, PaymentSettlement, PurchaseDetailView } from '@/types';
import type { UpdateLotPricePayload } from '@/types/api';
import { Modal } from '../ui/Modal';
import { Button } from '../ui/Button';
import { NumericInput } from '../ui/NumericInput';
import { FormActions, enterToSubmit } from '../ui/FormActions';
import { CurrencyDisplay } from '../common/CurrencyDisplay';
import { SettlementEditModal } from './SettlementEditModal';
import { refreshAppMetrics } from './navParams';

export interface LotPriceEditModalProps {
  /** null = closed. RETURN lots are never passed in. */
  lot: ItemLot | null;
  baseUnit: string;
  onClose: () => void;
  /** Called after a successful save; the caller reloads its lots. */
  onSaved: () => void;
}

/** Price shown for editing: per pack when the lot has a pack label, else per base unit. */
export function lotEntryPrice(lot: ItemLot): number {
  const total = lot.unitCost * lot.quantity;
  const per = lot.packLabel ? total / (lot.packQuantity || 1) : lot.unitCost;
  return Number(per.toFixed(4));
}

const Row: React.FC<{ label: string; old: React.ReactNode; next: React.ReactNode }> = ({ label, old, next }) => (
  <div className="flex items-center justify-between text-xs">
    <span className="text-slate-500">{label}</span>
    <span className="font-mono">
      {old} <span className="text-slate-400">to</span> <span className="font-semibold">{next}</span>
    </span>
  </div>
);

/**
 * FR-25: edit one lot's Purchase Price. The bill and payment effect, and every refusal (returns,
 * vendor return, total below what was applied), come from `preview_lot_purchase_price` (DD-56).
 */
export const LotPriceEditModal: React.FC<LotPriceEditModalProps> = ({ lot, baseUnit, onClose, onSaved }) => {
  const [price, setPrice] = useState(0);
  const [detail, setDetail] = useState<PurchaseDetailView | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [reloadKey, setReloadKey] = useState(0);
  const [editingRow, setEditingRow] = useState<PaymentSettlement | null>(null);

  const isPurchase = lot?.source === 'PURCHASE';
  const lotId = lot?.purchaseItemId ?? null;

  useEffect(() => {
    if (!lot) return;
    setPrice(lotEntryPrice(lot));
    setError(null);
    setEditingRow(null);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [lotId]);

  // The bill as stored (its old figures and payment rows); the effect of the edit is the backend's.
  useEffect(() => {
    if (!lot || lot.source !== 'PURCHASE') {
      setDetail(null);
      return;
    }
    let live = true;
    setLoadError(null);
    api
      .getPurchaseDetails(lot.purchaseTransactionId)
      .then((d) => live && setDetail(d))
      .catch((e) => live && setLoadError(errorMessage(e)));
    return () => {
      live = false;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [lotId, reloadKey]);

  /** Format check only: a number, 0 or more. */
  const problem = !Number.isFinite(price) || price < 0 ? 'Purchase price must be 0 or more' : null;
  // `reload` is part of the input: a corrected payment changes the answer for the same price.
  const previewInput = useMemo<{ payload: UpdateLotPricePayload; reload: number } | null>(
    () => (lot && !problem ? { payload: { purchaseItemId: lot.purchaseItemId, unitPrice: price }, reload: reloadKey } : null),
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [lotId, price, problem, reloadKey]
  );
  const preview = usePreview<{ payload: UpdateLotPricePayload; reload: number }, LotPriceUpdateResult>({
    input: previewInput,
    fetcher: (i) => api.previewLotPurchasePrice(i.payload),
  });
  const result = preview.data;
  const previewErrors = preview.errors;
  const belowApplied = previewErrors.some((e) => e.field === 'TOTAL_BELOW_APPLIED');
  const otherErrors = previewErrors.filter((e) => e.field !== 'TOTAL_BELOW_APPLIED');
  /** The backend reports an unchanged price by returning the same unit cost. */
  const changed = result != null && result.newUnitCost !== result.oldUnitCost;
  const settlementRows = (detail?.settlements ?? []).filter((s) => s.kind === 'SETTLEMENT');

  const canSave = !problem && !preview.isLoading && result != null && previewErrors.length === 0 && changed;

  const save = async () => {
    if (!lot || !canSave || saving) return;
    setSaving(true);
    setError(null);
    try {
      await api.updateLotPurchasePrice({ purchaseItemId: lot.purchaseItemId, unitPrice: price });
      onSaved();
      refreshAppMetrics();
      onClose();
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setSaving(false);
    }
  };

  const unitLabel = lot?.packLabel ?? baseUnit;
  const bill = result?.bill ?? null;

  return (
    <Modal
      isOpen={lot !== null}
      onClose={onClose}
      title="Edit Purchase Price"
      description={lot ? `${lot.transactionCode} on ${lot.transactionDate}` : undefined}
      size="lg"
    >
      {lot && (
        <div className="flex flex-col gap-4 py-1" onKeyDown={enterToSubmit(save)} data-testid="lot-price-edit">
          <div className="grid grid-cols-3 gap-3 text-xs">
            <div>
              <div className="text-[10px] uppercase tracking-wide text-slate-400">Quantity</div>
              <div className="font-mono">
                {lot.quantity} {baseUnit}
              </div>
            </div>
            <div>
              <div className="text-[10px] uppercase tracking-wide text-slate-400">Remaining</div>
              <div className="font-mono">
                {lot.remainingQuantity} {baseUnit}
              </div>
            </div>
            <div>
              <div className="text-[10px] uppercase tracking-wide text-slate-400">Current Purchase Price</div>
              <CurrencyDisplay amount={lot.unitCost} size="sm" />
              <span className="text-slate-400"> / {baseUnit}</span>
            </div>
          </div>
          <NumericInput
            label={`Purchase price per ${unitLabel} (₹) *`}
            aria-label="Purchase price"
            value={price}
            onChange={setPrice}
            min={0}
            autoFocus
          />
          <p className="text-[11px] text-slate-500">
            Cost of sold units (COGS) updates; sale bills don&apos;t change.
          </p>

          {loadError && (
            <p role="alert" className="text-xs text-rose-600 dark:text-rose-400">
              Could not load the bill: {loadError}
            </p>
          )}

          {isPurchase && detail && (
            <div className="rounded-md border border-slate-200 dark:border-slate-800 p-3 flex flex-col gap-1.5" data-testid="lot-price-preview">
              <div className="text-xs font-semibold">Bill {detail.purchase.transactionCode}</div>
              {bill && (
                <>
                  <Row label="Bill total" old={`₹${bill.oldTotal.toFixed(2)}`} next={`₹${bill.newTotal.toFixed(2)}`} />
                  <Row label="Cash paid" old={`₹${bill.oldPaid.toFixed(2)}`} next={`₹${bill.newPaid.toFixed(2)}`} />
                  <Row label="Balance due" old={`₹${detail.purchase.balanceDue.toFixed(2)}`} next={`₹${bill.balanceDue.toFixed(2)}`} />
                  <Row label="Status" old={detail.purchase.paymentStatus} next={bill.paymentStatus} />
                </>
              )}
            </div>
          )}

          {belowApplied && detail && (
            <div role="alert" className="rounded-md border border-rose-300 dark:border-rose-800 p-3 text-xs" data-testid="lot-price-blocked">
              <p className="font-medium text-rose-600 dark:text-rose-400">
                {previewErrors.find((e) => e.field === 'TOTAL_BELOW_APPLIED')?.message} Correct these payments first.
              </p>
              <ul className="mt-2 flex flex-col gap-1">
                {settlementRows.map((s) => (
                  <li key={s.id} className="flex items-center justify-between" data-testid="blocked-settlement">
                    <span className="font-mono">
                      {s.settlementDate} · ₹{s.amountPaid.toFixed(2)}
                    </span>
                    <Button variant="outline" size="sm" aria-label={`Edit payment ${s.id}`} onClick={() => setEditingRow(s)}>
                      Edit
                    </Button>
                  </li>
                ))}
                {settlementRows.length === 0 && <li className="text-slate-500">Return credit was applied to this bill.</li>}
              </ul>
            </div>
          )}

          {otherErrors.map((e, i) => (
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
          <SettlementEditModal
            settlement={editingRow}
            billCode={detail?.purchase.transactionCode}
            maxAmount={editingRow && detail ? detail.purchase.balanceDue + editingRow.amountPaid : 0}
            onClose={() => setEditingRow(null)}
            onSaved={() => {
              setReloadKey((k) => k + 1);
              refreshAppMetrics();
            }}
          />
        </div>
      )}
    </Modal>
  );
};

export default LotPriceEditModal;
