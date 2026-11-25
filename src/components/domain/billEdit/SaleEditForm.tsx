import React, { useEffect, useRef, useState } from 'react';
import { Trash2 } from 'lucide-react';
import { api, errorMessage } from '@/api';
import type { PreviewSalePayload } from '@/types';
import { Button } from '../../ui/Button';
import { Select } from '../../ui/Select';
import { TextInput } from '../../ui/TextInput';
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '../../ui/Table';
import { CurrencyDisplay } from '../../common/CurrencyDisplay';
import { PartySelector } from '../PartySelector';
import { ItemSearchTypeahead } from '../ItemSearchTypeahead';
import { BillChargesPanel } from '../BillChargesPanel';
import { useSaleEditDraft } from './useSaleEditDraft';
import { useBillEditPreview } from './useBillEditPreview';
import { EditErrors, EditTotals, PaymentStatusControl, inputCls } from './EditParts';

export interface SaleEditFormProps {
  billId: number;
  onCancel: () => void;
  /** The update succeeded; the drawer goes back to view mode and reloads. */
  onSaved: () => void;
  /** The drawer guards closing while there are unsaved changes. */
  onDirtyChange?: (dirty: boolean) => void;
}

const BASE = 0; // unit select value for the base unit

/**
 * Edit mode of the sale bill drawer (FR-20, DD-57). Local draft only: the billing screen's cart is
 * never read or written, so a sale in progress there stays as it is.
 */
export const SaleEditForm: React.FC<SaleEditFormProps> = ({ billId, onCancel, onSaved, onDirtyChange }) => {
  const dr = useSaleEditDraft(billId);
  const { draft } = dr;
  const [saving, setSaving] = useState(false);
  const savingRef = useRef(false);
  const [saveError, setSaveError] = useState<string | null>(null);
  const tableRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    onDirtyChange?.(dr.dirty);
  }, [dr.dirty, onDirtyChange]);

  const preview = useBillEditPreview<PreviewSalePayload>({
    bill: dr.bill,
    paymentStatus: draft?.paymentStatus ?? 'PAID',
    paidInput: draft?.paidInput ?? 0,
    fetcher: (p) => api.previewSale(p, billId),
  });

  const dateError = preview.errors.find((e) => e.field === 'transaction_date')?.message;
  const partyError = preview.errors.find((e) => e.field === 'party_id' || e.field === 'PARTY_REQUIRED_WITH_PAYMENTS')?.message;
  const listed = preview.errors.filter((e) => e.message !== dateError && e.message !== partyError).map((e) => e.message);
  const messages = saveError ? [...listed, saveError] : listed;
  const canSave =
    dr.load.status === 'ready' && !dr.incomplete && !preview.pending && preview.errors.length === 0 && !saving;

  const save = async () => {
    if (savingRef.current || !canSave) return;
    const payload = dr.toPayload(preview.paidToSend ?? 0);
    if (!payload) return;
    savingRef.current = true;
    setSaving(true);
    setSaveError(null);
    try {
      await api.updateSale(billId, payload);
      onSaved();
    } catch (e) {
      setSaveError(errorMessage(e));
    } finally {
      savingRef.current = false;
      setSaving(false);
    }
  };
  const saveRef = useRef(save);
  saveRef.current = save;
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && !e.altKey && !e.shiftKey && e.key.toLowerCase() === 's') {
        e.preventDefault();
        if (!e.repeat) void saveRef.current();
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, []);

  if (dr.load.status === 'loading') return <p className="text-xs text-slate-500">Loading...</p>;
  if (dr.load.status === 'error' || !draft) {
    return (
      <div className="flex flex-col gap-3">
        <p role="alert" className="text-xs text-rose-600 dark:text-rose-400">
          {dr.load.status === 'error' ? dr.load.message : 'Could not load the bill'}
        </p>
        <div>
          <Button variant="outline" size="sm" onClick={onCancel}>
            Back
          </Button>
        </div>
      </div>
    );
  }

  const focusQuantity = (key: number) =>
    setTimeout(() => tableRef.current?.querySelector<HTMLInputElement>(`[data-line-key="${key}"] input[data-qty]`)?.focus(), 0);

  return (
    <div className="flex flex-col gap-4 text-xs" data-testid="sale-edit-form">
      <h3 className="text-sm font-semibold">
        Editing <span className="font-mono">{draft.code}</span>
      </h3>

      <div className="grid grid-cols-2 gap-3 bg-slate-50 dark:bg-slate-800 p-3 rounded-md">
        <TextInput
          label="Date"
          type="date"
          value={draft.date}
          error={dateError}
          onChange={(e) => dr.setHeader({ date: e.target.value })}
          className="font-mono"
        />
        <div>
          <span className="text-xs font-medium text-slate-700 dark:text-slate-300">Customer</span>
          <PartySelector
            partyType="CUSTOMER"
            selectedPartyId={draft.partyId}
            onSelectParty={(party) => dr.setHeader({ partyId: party ? party.id : null })}
            allowWalkIn
          />
          {partyError && (
            <p role="alert" className="text-xs text-rose-600 dark:text-rose-400 mt-0.5">
              {partyError}
            </p>
          )}
        </div>
        <div className="col-span-2">
          <TextInput label="Notes" value={draft.notes} onChange={(e) => dr.setHeader({ notes: e.target.value })} />
        </div>
      </div>

      <div ref={tableRef}>
        <Table dense scroll="none">
          <TableHeader>
            <TableRow className="text-left text-slate-400">
              <TableHead className="py-1">Item</TableHead>
              <TableHead>Unit</TableHead>
              <TableHead align="right">Qty</TableHead>
              <TableHead align="right">Rate (₹)</TableHead>
              <TableHead align="right">Total</TableHead>
              <TableHead className="w-8" />
            </TableRow>
          </TableHeader>
          <TableBody>
            {draft.lines.length === 0 && (
              <TableRow>
                <TableCell colSpan={6} className="py-6 text-center text-slate-400">
                  No lines. Search an item below to add one.
                </TableCell>
              </TableRow>
            )}
            {draft.lines.map((l, idx) => (
              <TableRow key={l.key} data-testid="edit-line" data-line-key={l.key}>
                <TableCell className="py-1 font-medium">{l.name}</TableCell>
                <TableCell className="min-w-28">
                  <Select
                    aria-label={`Unit ${l.name}`}
                    value={l.bundleId ?? BASE}
                    onChange={(v) => dr.setUnit(l.key, Number(v) === BASE ? null : Number(v))}
                    options={[
                      { value: BASE, label: l.baseUnit },
                      ...l.bundles.map((b) => ({ value: b.bundleId, label: `${b.bundleName} (× ${b.unitsPerBundle})` })),
                    ]}
                  />
                </TableCell>
                <TableCell align="right">
                  <input
                    type="number"
                    data-qty
                    aria-label={`Quantity ${l.name}`}
                    min={0.001}
                    step="any"
                    value={l.quantity || ''}
                    onChange={(e) => dr.setLine(l.key, { quantity: parseFloat(e.target.value) || 0 })}
                    className={`w-20 ${inputCls}`}
                  />
                </TableCell>
                <TableCell align="right">
                  <input
                    type="number"
                    aria-label={`Rate ${l.name}`}
                    min={0}
                    step="any"
                    value={l.rate ?? ''}
                    onChange={(e) =>
                      dr.setLine(l.key, { rate: e.target.value === '' ? null : parseFloat(e.target.value) })
                    }
                    className={`w-24 ${inputCls}`}
                  />
                </TableCell>
                <TableCell align="right" mono className="font-semibold">
                  {preview.totals?.lineTotals[idx] != null ? (
                    <CurrencyDisplay amount={preview.totals.lineTotals[idx]} size="sm" />
                  ) : (
                    '—'
                  )}
                </TableCell>
                <TableCell className="text-center">
                  <button
                    type="button"
                    aria-label={`Remove ${l.name}`}
                    onClick={() => dr.removeLine(l.key)}
                    className="p-1 rounded text-slate-400 hover:text-rose-600 cursor-pointer"
                  >
                    <Trash2 className="h-3.5 w-3.5" />
                  </button>
                </TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      </div>

      <div data-testid="edit-add-line" className="flex flex-col gap-1">
        <span className="font-semibold text-slate-700 dark:text-slate-300">Add line</span>
        <ItemSearchTypeahead
          placeholder="Search item by name, barcode or category..."
          onSelectItem={(item) => focusQuantity(dr.addLine(item))}
        />
      </div>

      <BillChargesPanel
        side="SALE"
        lines={draft.charges}
        onAdd={dr.addCharge}
        onUpdateAmount={dr.updateChargeAmount}
        onRemove={dr.removeCharge}
        onDone={() => {}}
      />

      <div className="flex flex-wrap items-start justify-between gap-4">
        <PaymentStatusControl
          status={draft.paymentStatus}
          onStatus={(s) => dr.setHeader({ paymentStatus: s })}
          paidShown={preview.paidShown}
          paidInput={draft.paidInput}
          onPaidInput={(n) => dr.setHeader({ paidInput: n })}
          applied={preview.totals?.appliedAmount ?? dr.applied}
        />
        <EditTotals totals={preview.totals} balanceDue={preview.balanceDue} />
      </div>

      <div className="sticky bottom-0 -mx-5 -mb-5 px-5 py-3 bg-slate-50 dark:bg-slate-900 border-t border-slate-100 dark:border-slate-800 flex flex-col gap-2">
        <EditErrors messages={messages} />
        <div className="flex justify-end gap-2">
          <Button variant="outline" size="sm" data-testid="edit-cancel" onClick={onCancel} disabled={saving}>
            Cancel
          </Button>
          <Button
            variant="primary"
            size="sm"
            data-testid="edit-save"
            disabled={!canSave}
            isLoading={saving}
            onClick={() => void save()}
            title="Save changes (Ctrl+S)"
          >
            Save changes
          </Button>
        </div>
      </div>
    </div>
  );
};

export default SaleEditForm;
