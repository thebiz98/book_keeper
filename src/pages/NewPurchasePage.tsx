import React, { useState, useEffect, useRef, useCallback } from 'react';
import { Plus, Trash2, CheckCircle2 } from 'lucide-react';
import { api } from '@/api';
import { errorMessage } from '@/api/errors';
import { todayLocal } from '@/lib/dates';
import type { CreatePurchasePayload, ItemBundle, ItemSearchResult, PaymentStatus } from '@/types';
import { PartySelector } from '@/components/domain/PartySelector';
import { ItemSearchTypeahead } from '@/components/domain/ItemSearchTypeahead';
import { BundleCreationModal } from '@/components/domain/BundleCreationModal';
import {
  effectivePaid,
  lineBaseQuantity,
  lineCostPerBase,
  lineTotal,
  purchaseTotal,
  validatePayment,
  type PurchaseLine,
} from '@/components/domain/purchaseMath';
import { CurrencyDisplay } from '@/components/common/CurrencyDisplay';
import { QuantityInput } from '@/components/common/QuantityInput';
import { Button } from '@/components/ui/Button';
import { Select } from '@/components/ui/Select';
import { TextInput } from '@/components/ui/TextInput';
import { Table, TableHeader, TableBody, TableRow, TableHead, TableCell } from '@/components/ui/Table';
import {
  prefillFromLocation,
  takePurchasePrefill,
  type PurchasePrefill,
} from './purchasePrefill';

export interface NewPurchasePageProps {
  /** Pre-select a catalog item (dashboard "Inward (F3)"). See purchasePrefill.ts. */
  initialItemId?: number;
  initialItemName?: string;
}

const BASE = 0; // unit selector value for "base unit"

async function resolvePrefill(spec: PurchasePrefill): Promise<ItemSearchResult | null> {
  try {
    if (spec.name) {
      const rows = await api.searchItemsFts(spec.name, 8);
      return rows.find((r) => r.itemId === spec.itemId) ?? (spec.itemId ? null : rows[0] ?? null) ?? null;
    }
    if (spec.itemId) {
      for (let page = 1; page <= 5; page++) {
        const res = await api.getItemsCatalog({ page, limit: 200 });
        const hit = res.items.find((r) => r.itemId === spec.itemId);
        if (hit) return hit;
        if (page * res.limit >= res.total) break;
      }
    }
  } catch {
    /* fall through */
  }
  return null;
}

export const NewPurchasePage: React.FC<NewPurchasePageProps> = ({
  initialItemId,
  initialItemName,
}) => {
  const [codePreview, setCodePreview] = useState<string | null>(null);
  const [inwardDate, setInwardDate] = useState<string>(todayLocal());
  const [vendorId, setVendorId] = useState<number | null>(null);
  const [supplierInvoiceRef, setSupplierInvoiceRef] = useState<string>('');
  const [notes, setNotes] = useState<string>('');

  // Draft row
  const [selectedItem, setSelectedItem] = useState<ItemSearchResult | null>(null);
  const [unitChoice, setUnitChoice] = useState<number>(BASE); // BASE (0) or bundleId
  const [quantity, setQuantity] = useState<number>(1);
  const [unitCostText, setUnitCostText] = useState<string>('');
  const [draftError, setDraftError] = useState<string | null>(null);

  const [lines, setLines] = useState<PurchaseLine[]>([]);
  const lineKey = useRef(1);

  const [paymentStatus, setPaymentStatus] = useState<PaymentStatus>('PAID');
  const [paidInput, setPaidInput] = useState<number>(0);

  const [isBundleModalOpen, setIsBundleModalOpen] = useState(false);
  const [isSubmitting, setIsSubmitting] = useState(false);
  const submitting = useRef(false);
  const [error, setError] = useState<string | null>(null);
  const [successCode, setSuccessCode] = useState<string | null>(null);

  const searchWrapRef = useRef<HTMLDivElement>(null);
  const costRef = useRef<HTMLInputElement>(null);
  const tableCardRef = useRef<HTMLDivElement>(null);

  const focusSearch = () => {
    setTimeout(() => searchWrapRef.current?.querySelector('input')?.focus(), 0);
  };

  const refreshCodePreview = useCallback(() => {
    api
      .getNextPurchaseCode()
      .then((c) => setCodePreview(c || null))
      .catch(() => setCodePreview(null));
  }, []);

  useEffect(() => {
    refreshCodePreview();
  }, [refreshCodePreview]);

  const chooseItem = useCallback((item: ItemSearchResult) => {
    setSelectedItem(item);
    setUnitChoice(item.matchedBundleId ?? BASE);
    setQuantity(1);
    setDraftError(null);
    setTimeout(() => costRef.current?.focus(), 0);
  }, []);

  // Pre-fill (dashboard Inward action / ?item= / props). Runs once on mount.
  useEffect(() => {
    const spec: PurchasePrefill | null =
      initialItemId || initialItemName
        ? { itemId: initialItemId, name: initialItemName }
        : (takePurchasePrefill() ?? prefillFromLocation());
    if (!spec) return;
    let cancelled = false;
    resolvePrefill(spec).then((item) => {
      if (item && !cancelled) chooseItem(item);
    });
    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const selectedBundle = selectedItem?.bundles.find((b) => b.bundleId === unitChoice) ?? null;
  const unitsPerBundle = selectedBundle ? selectedBundle.unitsPerBundle : 1;
  const packLabel = selectedBundle ? selectedBundle.bundleName : (selectedItem?.baseUnit ?? '');

  const unitOptions = selectedItem
    ? [
        { value: BASE, label: `${selectedItem.baseUnit} (base unit)` },
        ...selectedItem.bundles.map((b) => ({
          value: b.bundleId,
          label: `${b.bundleName} (× ${b.unitsPerBundle})`,
        })),
      ]
    : [{ value: BASE, label: 'Select an item first' }];

  const handleBundleCreated = (b: ItemBundle) => {
    setSelectedItem((cur) =>
      cur && cur.itemId === b.itemId
        ? {
            ...cur,
            bundles: [
              ...cur.bundles.filter((x) => x.bundleId !== b.id),
              {
                bundleId: b.id,
                bundleName: b.bundleName,
                unitsPerBundle: b.unitsPerBundle,
                bundlePrice: b.defaultBundlePrice,
                barcode: b.barcode,
              },
            ],
          }
        : cur
    );
    setUnitChoice(b.id);
  };

  const handleAddLine = () => {
    if (!selectedItem) {
      setDraftError('Select an item first');
      return;
    }
    if (!(quantity > 0)) {
      setDraftError('Quantity must be greater than 0');
      return;
    }
    const cost = unitCostText.trim() === '' ? NaN : Number(unitCostText);
    if (!Number.isFinite(cost) || cost < 0) {
      setDraftError('Enter a unit cost (0 or more)');
      return;
    }
    setLines((prev) => [
      ...prev,
      {
        key: lineKey.current++,
        itemId: selectedItem.itemId,
        itemName: selectedItem.name,
        baseUnit: selectedItem.baseUnit,
        bundleId: selectedBundle ? selectedBundle.bundleId : null,
        packLabel,
        unitsPerBundle,
        quantity,
        unitCost: cost,
      },
    ]);
    setSelectedItem(null);
    setUnitChoice(BASE);
    setQuantity(1);
    setUnitCostText('');
    setDraftError(null);
    focusSearch();
  };

  // Lines are appended at the end: keep the newest one visible in the bounded table.
  const prevLineCount = useRef(0);
  useEffect(() => {
    if (lines.length > prevLineCount.current) {
      const el = tableCardRef.current?.querySelector<HTMLElement>('[data-table-scroll]');
      if (el) el.scrollTop = el.scrollHeight;
    }
    prevLineCount.current = lines.length;
  }, [lines.length]);

  const handleRemoveLine = (key: number) => setLines((prev) => prev.filter((l) => l.key !== key));

  const total = purchaseTotal(lines);
  const paid = effectivePaid(paymentStatus, total, paidInput);
  const balanceDue = Math.max(0, total - paid);
  const paymentError = lines.length > 0 ? validatePayment(paymentStatus, total, paid, vendorId) : null;
  const totalBaseUnits = lines.reduce((a, l) => a + lineBaseQuantity(l), 0);

  const handleRecordInward = async () => {
    if (submitting.current) return;
    if (lines.length === 0) {
      setError('Add at least one line before recording the purchase');
      return;
    }
    const pe = validatePayment(paymentStatus, total, paid, vendorId);
    if (pe) {
      setError(pe);
      return;
    }

    submitting.current = true;
    setIsSubmitting(true);
    setError(null);
    setSuccessCode(null);

    const payload: CreatePurchasePayload = {
      transactionDate: inwardDate,
      partyId: vendorId,
      supplierInvoiceRef: supplierInvoiceRef.trim() || null,
      paymentStatus,
      paidAmount: paid,
      paymentMode: 'CASH',
      notes: notes.trim() || null,
      items: lines.map((l) => ({
        itemId: l.itemId,
        bundleId: l.bundleId,
        bundleQuantity: l.quantity,
        unitCost: l.unitCost,
      })),
    };

    try {
      const res = await api.createPurchase(payload);
      setSuccessCode(res.transactionCode);
      setLines([]);
      setSupplierInvoiceRef('');
      setNotes('');
      setVendorId(null);
      setPaymentStatus('PAID');
      setPaidInput(0);
      setInwardDate(todayLocal());
      setSelectedItem(null);
      setUnitCostText('');
      refreshCodePreview();
      focusSearch();
    } catch (err) {
      setError(errorMessage(err) || 'Failed to record inward purchase');
    } finally {
      submitting.current = false;
      setIsSubmitting(false);
    }
  };

  // Hotkeys: Ctrl+Enter records, Alt+B opens the bundle modal. Own listener so the page works
  // regardless of the shell's registry; always calls the latest handler.
  const latest = useRef({ record: handleRecordInward, bundle: () => {}, modalOpen: false });
  latest.current = {
    modalOpen: isBundleModalOpen,
    record: handleRecordInward,
    bundle: () => {
      if (selectedItem) setIsBundleModalOpen(true);
      else setDraftError('Select an item before creating a bundle (Alt+B)');
    },
  };
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (latest.current.modalOpen) return;
      if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) {
        e.preventDefault();
        void latest.current.record();
      } else if (e.altKey && e.key.toLowerCase() === 'b') {
        e.preventDefault();
        latest.current.bundle();
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, []);

  const onDraftKeyDown = (e: React.KeyboardEvent) => {
    if (e.key !== 'Enter' || e.ctrlKey || e.metaKey || e.defaultPrevented) return;
    if ((e.target as HTMLElement).closest('[data-purchase-search]')) return;
    e.preventDefault();
    handleAddLine();
  };

  return (
    <div className="flex flex-col gap-4">
      {/* Header Info Bar */}
      <div className="flex flex-wrap items-end justify-between gap-3 p-3.5 rounded-lg bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 shadow-xs">
        <div className="flex flex-wrap items-end gap-3">
          <div className="flex items-center gap-1.5 px-3 py-1.5 rounded-md bg-amber-50 dark:bg-amber-950/60 border border-amber-200 dark:border-amber-800">
            <span className="text-[10px] uppercase font-bold text-amber-600 dark:text-amber-400">
              Inward
            </span>
            <span
              data-testid="code-preview"
              title="Preview only. The final code is assigned when the purchase is recorded."
              className="text-sm font-bold font-mono text-amber-950 dark:text-amber-100"
            >
              {codePreview ?? 'PUR-…'}
            </span>
          </div>

          <div className="w-36">
            <TextInput
              label="Inward Date"
              type="date"
              value={inwardDate}
              onChange={(e) => setInwardDate(e.target.value)}
              className="font-mono"
            />
          </div>

          <div className="w-44">
            <TextInput
              label="Notes"
              placeholder="optional"
              value={notes}
              onChange={(e) => setNotes(e.target.value)}
            />
          </div>

          <div className="w-44">
            <TextInput
              label="Supplier Invoice / DC #"
              placeholder="e.g. INV-9841"
              value={supplierInvoiceRef}
              onChange={(e) => setSupplierInvoiceRef(e.target.value)}
            />
          </div>
        </div>

        <div className="w-72">
          <span className="text-xs font-medium text-slate-700 dark:text-slate-300">Vendor</span>
          <PartySelector
            partyType="VENDOR"
            selectedPartyId={vendorId}
            onSelectParty={(party) => setVendorId(party ? party.id : null)}
            allowWalkIn
          />
        </div>
      </div>

      {/* Inward Line Item Entry Row */}
      <div
        onKeyDown={onDraftKeyDown}
        className="p-3.5 rounded-lg bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 shadow-xs flex flex-wrap items-start gap-2 text-xs"
      >
        <div className="flex-1 min-w-[220px] flex flex-col gap-1">
          <span className="font-semibold text-slate-700 dark:text-slate-300">Item *</span>
          <div ref={searchWrapRef} data-purchase-search data-testid="purchase-item-search">
            <ItemSearchTypeahead
              onSelectItem={chooseItem}
              autoFocus
              placeholder="Search catalog item by name, barcode or category..."
            />
          </div>
          {selectedItem && (
            <span data-testid="selected-item" className="text-[11px] text-slate-500">
              Selected: <b className="text-slate-800 dark:text-slate-200">{selectedItem.name}</b> (
              {selectedItem.baseUnit})
            </span>
          )}
        </div>

        <div className="w-44">
          <Select
            label="Unit"
            hotkey="Alt+B new"
            value={unitChoice}
            onChange={(v) => setUnitChoice(Number(v))}
            options={unitOptions}
            disabled={!selectedItem}
            aria-label="Unit"
          />
        </div>

        <div className="w-36 flex flex-col gap-1">
          <span className="font-semibold text-slate-700 dark:text-slate-300">Quantity *</span>
          <QuantityInput
            quantity={quantity}
            onChange={setQuantity}
            baseUnit={selectedItem?.baseUnit ?? 'PCS'}
            unitsPerBundle={unitsPerBundle}
            bundleName={selectedBundle?.bundleName}
            min={0.001}
            allowDecimals
            disabled={!selectedItem}
          />
        </div>

        <div className="w-32">
          <TextInput
            ref={costRef}
            label={`Unit Cost (₹ / ${packLabel || 'unit'}) *`}
            type="number"
            min={0}
            step="any"
            value={unitCostText}
            onChange={(e) => setUnitCostText(e.target.value)}
            autoSelectOnFocus
            className="text-right font-mono"
            disabled={!selectedItem}
          />
        </div>

        <div className="flex flex-col gap-1">
          <span className="invisible">.</span>
          <div className="flex gap-2">
            <Button
              type="button"
              variant="outline"
              size="md"
              disabled={!selectedItem}
              onClick={() => latest.current.bundle()}
              hotkey="Alt+B"
            >
              New Bundle
            </Button>
            <Button
              type="button"
              variant="primary"
              size="md"
              onClick={handleAddLine}
              leftIcon={<Plus className="h-4 w-4" />}
            >
              Add Line
            </Button>
          </div>
        </div>

        {draftError && (
          <p role="alert" className="basis-full text-xs text-rose-600 dark:text-rose-400 font-medium">
            {draftError}
          </p>
        )}
      </div>

      {/* Inward Items Table */}
      <div ref={tableCardRef} className="flex-1 min-h-[220px] rounded-lg border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 overflow-hidden shadow-xs">
        <Table dense>
          <TableHeader>
            <TableRow>
              <TableHead className="w-10 text-center">#</TableHead>
              <TableHead>Item Name</TableHead>
              <TableHead>Packaging</TableHead>
              <TableHead className="text-center">Inward Qty</TableHead>
              <TableHead className="text-center">Base Units</TableHead>
              <TableHead align="right">Unit Cost (₹)</TableHead>
              <TableHead align="right">Cost / Base Unit (₹)</TableHead>
              <TableHead align="right">Line Total (₹)</TableHead>
              <TableHead className="w-12 text-center">Action</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {lines.length === 0 ? (
              <TableRow>
                <TableCell colSpan={9} className="py-12 text-center text-slate-400 text-xs">
                  No inward lines yet. Pick an item above and press Enter to add it.
                </TableCell>
              </TableRow>
            ) : (
              lines.map((l, idx) => (
                <TableRow key={l.key} data-testid="purchase-line">
                  <TableCell className="text-center text-slate-400 select-none">{idx + 1}</TableCell>
                  <TableCell className="font-semibold text-slate-900 dark:text-slate-100">
                    {l.itemName}
                  </TableCell>
                  <TableCell className="text-slate-500">
                    {l.bundleId !== null ? `${l.packLabel} (× ${l.unitsPerBundle})` : l.baseUnit}
                  </TableCell>
                  <TableCell className="text-center font-mono font-medium">
                    {l.quantity} {l.packLabel}
                  </TableCell>
                  <TableCell className="text-center font-mono font-medium text-blue-600 dark:text-blue-400">
                    {lineBaseQuantity(l)} {l.baseUnit}
                  </TableCell>
                  <TableCell align="right" mono>
                    <CurrencyDisplay amount={l.unitCost} size="sm" />
                  </TableCell>
                  <TableCell align="right" mono className="text-slate-500">
                    {lineCostPerBase(l).toFixed(4)}
                  </TableCell>
                  <TableCell align="right" mono className="font-bold text-slate-900 dark:text-slate-100">
                    <CurrencyDisplay amount={lineTotal(l)} size="sm" />
                  </TableCell>
                  <TableCell className="text-center">
                    <button
                      type="button"
                      aria-label={`Remove ${l.itemName}`}
                      onClick={() => handleRemoveLine(l.key)}
                      className="p-1 rounded text-slate-400 hover:text-rose-600 transition-colors cursor-pointer"
                    >
                      <Trash2 className="h-3.5 w-3.5" />
                    </button>
                  </TableCell>
                </TableRow>
              ))
            )}
          </TableBody>
        </Table>
      </div>

      {/* Footer Settlement & Commit Bar */}
      <div className="p-4 rounded-lg bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 shadow-xs flex flex-wrap items-center justify-between gap-4">
        <div className="flex items-center gap-6">
          <div className="flex flex-col">
            <span className="text-xs text-slate-500 font-medium">Total Purchase Value</span>
            <div data-testid="purchase-total" className="text-2xl font-bold font-mono text-slate-900 dark:text-slate-100">
              <CurrencyDisplay amount={total} size="xl" />
            </div>
          </div>
          <div className="h-8 w-px bg-slate-200 dark:bg-slate-800" />
          <div className="flex flex-col text-xs text-slate-500 font-mono">
            <span>Lines: {lines.length}</span>
            <span>Total Base Units: {totalBaseUnits}</span>
          </div>
        </div>

        <div className="flex items-center gap-3">
          <div
            role="radiogroup"
            aria-label="Payment status"
            className="flex p-0.5 rounded-lg border border-slate-200 dark:border-slate-700 bg-slate-100 dark:bg-slate-800 text-xs"
          >
            {(['PAID', 'PARTIAL', 'UNPAID'] as const).map((st) => (
              <button
                key={st}
                type="button"
                role="radio"
                aria-checked={paymentStatus === st}
                onClick={() => setPaymentStatus(st)}
                className={`px-3 py-1.5 rounded-md font-semibold transition-colors cursor-pointer ${
                  paymentStatus === st
                    ? 'bg-white dark:bg-slate-900 text-amber-600 dark:text-amber-400 shadow-2xs'
                    : 'text-slate-600 dark:text-slate-400 hover:text-slate-900'
                }`}
              >
                {st}
              </button>
            ))}
          </div>

          <div className="flex items-center gap-1.5 bg-slate-50 dark:bg-slate-800 px-2 py-1 rounded border border-slate-200 dark:border-slate-700 text-xs">
            <span className="text-slate-400">Paid: ₹</span>
            <input
              type="number"
              aria-label="Paid amount"
              min={0}
              step="any"
              disabled={paymentStatus !== 'PARTIAL'}
              value={paymentStatus === 'PARTIAL' ? paidInput || '' : paid}
              onChange={(e) => setPaidInput(parseFloat(e.target.value) || 0)}
              className="w-24 bg-transparent font-mono font-bold text-right focus:outline-none disabled:opacity-60"
            />
          </div>
          <div className="flex flex-col text-xs">
            <span className="text-slate-500">Balance due (vendor payable)</span>
            <CurrencyDisplay amount={balanceDue} size="md" />
          </div>
        </div>

        <div className="flex items-center gap-2">
          <Button
            variant="primary"
            size="lg"
            disabled={lines.length === 0}
            isLoading={isSubmitting}
            onClick={() => void handleRecordInward()}
            hotkey="Ctrl+Enter"
            leftIcon={<CheckCircle2 className="h-5 w-5" />}
          >
            Record Inward Stock
          </Button>
        </div>
      </div>

      {paymentError && !error && (
        <p data-testid="payment-hint" className="text-xs text-amber-700 dark:text-amber-400 px-1">
          {paymentError}
        </p>
      )}

      {error && (
        <p role="alert" className="text-xs text-rose-600 dark:text-rose-400 font-medium px-1">
          {error}
        </p>
      )}

      {successCode && (
        <p
          role="status"
          data-testid="purchase-success"
          className="text-sm text-emerald-700 dark:text-emerald-400 font-medium px-1"
        >
          Purchase recorded: <span className="font-mono font-bold">{successCode}</span>
        </p>
      )}

      <BundleCreationModal
        isOpen={isBundleModalOpen}
        onClose={() => setIsBundleModalOpen(false)}
        initialItemId={selectedItem?.itemId}
        baseUnit={selectedItem?.baseUnit}
        onBundleCreated={handleBundleCreated}
      />
    </div>
  );
};

export { NewPurchasePage as PurchasesPage };
export default NewPurchasePage;
