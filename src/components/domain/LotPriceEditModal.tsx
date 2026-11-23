import React, { useEffect, useMemo, useState } from 'react';
import { api, errorMessage } from '@/api';
import { round2 } from '@/lib/money';
import { lotPriceEffect } from '@/lib/lotPriceEffect';
import type { BillEditGuard } from '@/types/api';
import type { ItemLot, PaymentSettlement, PurchaseDetailView } from '@/types';
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

/** FR-25: edit one lot's Purchase Price, with a live preview of the bill and payment effect. */
export const LotPriceEditModal: React.FC<LotPriceEditModalProps> = ({ lot, baseUnit, onClose, onSaved }) => {
  const [price, setPrice] = useState(0);
  const [detail, setDetail] = useState<PurchaseDetailView | null>(null);
  const [guard, setGuard] = useState<BillEditGuard | null>(null);
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

  useEffect(() => {
    if (!lot || lot.source !== 'PURCHASE') {
      setDetail(null);
      setGuard(null);
      return;
    }
    let live = true;
    setLoadError(null);
    Promise.all([api.getPurchaseDetails(lot.purchaseTransactionId), api.getBillEditGuard('PURCHASE', lot.purchaseTransactionId)])
      .then(([d, g]) => {
        if (!live) return;
        setDetail(d);
        setGuard(g);
      })
      .catch((e) => live && setLoadError(errorMessage(e)));
    return () => {
      live = false;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [lotId, reloadKey]);

  const packQty = lot?.packQuantity ?? 1;
  const newLineTotal = round2(packQty * price);
  const changed = lot ? Math.abs(newLineTotal / (lot.quantity || 1) - lot.unitCost) > 1e-9 : false;

  const effect = useMemo(() => {
    if (!lot || !isPurchase || !detail || !guard) return null;
    return lotPriceEffect({
      otherLineTotals: detail.items.filter((i) => i.purchaseItem.id !== lot.purchaseItemId).map((i) => i.purchaseItem.totalAmount),
      editedLineTotal: newLineTotal,
      charges: detail.charges,
      settled: detail.settlements.filter((s) => s.kind === 'SETTLEMENT').reduce((s, r) => s + r.amountPaid, 0),
      applied: guard.appliedAmount,
      upfrontOld: guard.upfrontAmount,
      oldBalanceDue: detail.purchase.balanceDue,
    });
  }, [lot, isPurchase, detail, guard, newLineTotal]);

  const guardBlocks = guard?.reasons.filter((r) => r.blocksEdit) ?? [];
  const limit = guard?.lotLimits.find((l) => l.purchaseItemId === lot?.purchaseItemId);
  const vendorReturnBlock = isPurchase && changed && !!limit?.vendorReturned;
  const belowApplied = effect?.blocked === true;
  const settlementRows = (detail?.settlements ?? []).filter((s) => s.kind === 'SETTLEMENT');
  const waiting = isPurchase && !loadError && (detail === null || guard === null);

  const problem = !Number.isFinite(price) || price < 0 ? 'Purchase price must be 0 or more' : null;
  const canSave = !problem && changed && guardBlocks.length === 0 && !vendorReturnBlock && !belowApplied && !waiting;

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
              {effect && !effect.blocked && (
                <>
                  <Row label="Bill total" old={`₹${detail.purchase.totalAmount.toFixed(2)}`} next={`₹${effect.newTotal.toFixed(2)}`} />
                  <Row label="Cash paid" old={`₹${detail.purchase.paidAmount.toFixed(2)}`} next={`₹${effect.newPaid.toFixed(2)}`} />
                  <Row label="Balance due" old={`₹${detail.purchase.balanceDue.toFixed(2)}`} next={`₹${effect.balanceDue.toFixed(2)}`} />
                  <Row label="Status" old={detail.purchase.paymentStatus} next={effect.status} />
                </>
              )}
            </div>
          )}

          {belowApplied && effect && effect.blocked && detail && (
            <div role="alert" className="rounded-md border border-rose-300 dark:border-rose-800 p-3 text-xs" data-testid="lot-price-blocked">
              <p className="font-medium text-rose-600 dark:text-rose-400">
                The new total ₹{effect.newTotal.toFixed(2)} is below the ₹{effect.applied.toFixed(2)} already applied to this
                bill. Correct these payments first.
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

          {guardBlocks.map((r) => (
            <p key={r.code} role="alert" className="text-xs text-rose-600 dark:text-rose-400 font-medium">
              {r.message}
            </p>
          ))}
          {vendorReturnBlock && (
            <p role="alert" className="text-xs text-rose-600 dark:text-rose-400 font-medium">
              This stock was returned to the vendor at its old price. Delete that vendor return to change the price.
            </p>
          )}
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
