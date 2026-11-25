import React, { useState, useEffect, useRef, useCallback, useMemo } from 'react';
import { Plus, Trash2, CheckCircle2 } from 'lucide-react';
import { api } from '@/api';
import { errorMessage } from '@/api/errors';
import { todayLocal } from '@/lib/dates';
import { usePreview } from '@/hooks/usePreview';
import type {
  BillPreview,
  CreatePurchasePayload,
  SavedBundle,
  ItemSearchResult,
  PaymentStatus,
  PreviewPurchasePayload,
} from '@/types';
import { PartySelector } from '@/components/domain/PartySelector';
import { ItemSearchTypeahead } from '@/components/domain/ItemSearchTypeahead';
import { BundleCreationModal } from '@/components/domain/BundleCreationModal';
import { ItemQuickCreateModal } from '@/components/domain/ItemQuickCreateModal';
import { BillChargesPanel, type BillChargesPanelHandle } from '@/components/domain/BillChargesPanel';
import { BillFooterTotals } from '@/components/domain/BillFooterTotals';
import { ConfirmDialog } from '@/components/common/ConfirmDialog';
import { toChargeInputs, type BillChargeLine } from '@/lib/billMath';
import { lineBaseQuantity, lineCostPerBase, type PurchaseLine } from '@/components/domain/purchaseMath';
import { CurrencyDisplay } from '@/components/common/CurrencyDisplay';
import { QuantityInput } from '@/components/common/QuantityInput';
import { Button } from '@/components/ui/Button';
import { Select } from '@/components/ui/Select';
import { EntryField } from '@/components/ui/EntryField';
import { NEW_BUNDLE, NEW_BUNDLE_LABEL } from '@/lib/bundleSentinel';
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
  // Bill-level charges and discounts (FR-19).
  const [charges, setCharges] = useState<BillChargeLine[]>([]);
  const chargeKey = useRef(1);
  const [chargeModalOpen, setChargeModalOpen] = useState(false);
  const chargesRef = useRef<BillChargesPanelHandle>(null);

  const [paymentStatus, setPaymentStatus] = useState<PaymentStatus>('PAID');
  const [paidInput, setPaidInput] = useState<number>(0);

  const [isBundleModalOpen, setIsBundleModalOpen] = useState(false);
  const [isClearConfirmOpen, setIsClearConfirmOpen] = useState(false);
  const [isSubmitting, setIsSubmitting] = useState(false);
  const submitting = useRef(false);
  const [error, setError] = useState<string | null>(null);
  const [successCode, setSuccessCode] = useState<string | null>(null);

  const searchWrapRef = useRef<HTMLDivElement>(null);
  const costRef = useRef<HTMLInputElement>(null);
  /** Non-null while the New Item quick-create modal is open (holds the pre-filled name). */
  const [quickName, setQuickName] = useState<string | null>(null);
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
    // Same flow as New Sale: item, then quantity.
    setTimeout(() => {
      const q = document.getElementById('purchase-qty') as HTMLInputElement | null;
      q?.focus();
      q?.select();
    }, 0);
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

  const unitOptions: { value: string | number; label: string }[] = selectedItem
    ? [
        { value: BASE, label: `${selectedItem.baseUnit} (base unit)` },
        ...selectedItem.bundles.map((b) => ({
          value: b.bundleId,
          label: `${b.bundleName} (× ${b.unitsPerBundle})`,
        })),
        { value: NEW_BUNDLE, label: NEW_BUNDLE_LABEL },
      ]
    : [{ value: BASE, label: 'Select an item first' }];

  const handleBundleCreated = (b: SavedBundle) => {
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
                defaultRate: b.defaultRate,
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

  // DD-56: every figure comes from the backend. `totals` previews the bill without a payment (line
  // totals, charges, total, what is open); `payPreview` adds the chosen payment (balance, errors).
  const bill = useMemo<PreviewPurchasePayload | null>(
    () =>
      lines.length === 0
        ? null
        : {
            transactionDate: inwardDate,
            partyId: vendorId,
            supplierInvoiceRef: supplierInvoiceRef.trim() || null,
            paymentMode: 'CASH',
            charges: toChargeInputs(charges),
            notes: notes.trim() || null,
            items: lines.map((l) => ({
              itemId: l.itemId,
              bundleId: l.bundleId,
              bundleQuantity: l.quantity,
              unitCost: l.unitCost,
            })),
          },
    [lines, inwardDate, vendorId, supplierInvoiceRef, charges, notes]
  );
  const totalsPreview = usePreview<PreviewPurchasePayload, BillPreview>({
    input: bill,
    fetcher: (p) => api.previewPurchase(p, null),
  });
  const totals = totalsPreview.data;
  /** PAID pays what is open (the backend's figure); UNPAID pays nothing; PARTIAL is what was typed. */
  const paidToSend =
    paymentStatus === 'PAID' ? totals?.openAmount : paymentStatus === 'UNPAID' ? 0 : paidInput;
  const payInput = useMemo<PreviewPurchasePayload | null>(
    () =>
      bill && paidToSend != null && !totalsPreview.isLoading
        ? { ...bill, paymentStatus, paidAmount: paidToSend }
        : null,
    [bill, paymentStatus, paidToSend, totalsPreview.isLoading]
  );
  const payPreview = usePreview<PreviewPurchasePayload, BillPreview>({
    input: payInput,
    fetcher: (p) => api.previewPurchase(p, null),
  });
  // Only an in-flight preview is pending; a failed one reports its error (shown above the buttons).
  const pending = bill !== null && (totalsPreview.isLoading || payPreview.isLoading);
  const previewErrors = [...totalsPreview.errors, ...payPreview.errors];
  const itemsSubtotal = totals?.itemsSubtotal ?? 0;
  const total = totals?.totalAmount ?? 0;
  const paid = paymentStatus === 'PARTIAL' ? paidInput : (payPreview.data?.payment?.paidAmount ?? paidToSend ?? 0);
  const balanceDue = payPreview.data?.payment?.balanceDue ?? totals?.openAmount ?? 0;
  const paymentError = lines.length > 0 && !pending ? (previewErrors[0]?.message ?? null) : null;
  const payVerdictMissing = bill !== null && !pending && payPreview.data === null && previewErrors.length === 0;

  const addCharge = (line: Omit<BillChargeLine, 'key'>): boolean => {
    if (!(line.amount > 0)) return false;
    setCharges((prev) => [...prev, { ...line, key: chargeKey.current++ }]);
    return true;
  };
  const updateChargeAmount = (key: number, amount: number): boolean => {
    if (!(amount > 0)) return false;
    setCharges((prev) => prev.map((c) => (c.key === key ? { ...c, amount } : c)));
    return true;
  };
  const removeCharge = (key: number) => setCharges((prev) => prev.filter((c) => c.key !== key));
  const totalBaseUnits = lines.reduce((a, l) => a + lineBaseQuantity(l), 0);

  /** Back to an empty purchase (after Save, or Clear). */
  const clearDraft = () => {
    setLines([]);
    setCharges([]);
    setSupplierInvoiceRef('');
    setNotes('');
    setVendorId(null);
    setPaymentStatus('PAID');
    setPaidInput(0);
    setInwardDate(todayLocal());
    setSelectedItem(null);
    setUnitChoice(BASE);
    setQuantity(1);
    setUnitCostText('');
    setDraftError(null);
    setError(null);
    setSuccessCode(null);
  };

  const handleRecordInward = async () => {
    if (submitting.current) return;
    if (lines.length === 0) {
      setError('Add at least one line before recording the purchase');
      return;
    }
    // Saving waits for the backend's verdict on exactly what is on screen (pending or any error blocks).
    if (!bill) return;
    if (pending) {
      setError('Still checking the bill. Try again in a moment.');
      return;
    }
    if (previewErrors.length > 0) {
      setError(previewErrors[0].message);
      return;
    }
    if (payVerdictMissing) {
      setError('Could not check the payment. Change a value and try again.');
      return;
    }

    submitting.current = true;
    setIsSubmitting(true);
    setError(null);
    setSuccessCode(null);

    const payload: CreatePurchasePayload = {
      ...bill,
      paymentStatus,
      paidAmount: paidToSend ?? 0,
    };

    try {
      const res = await api.createPurchase(payload);
      clearDraft();
      setSuccessCode(res.transactionCode);
      refreshCodePreview();
      focusSearch();
    } catch (err) {
      setError(errorMessage(err) || 'Failed to record inward purchase');
    } finally {
      submitting.current = false;
      setIsSubmitting(false);
    }
  };

  // Hotkeys: Ctrl+Enter records, Alt+B opens the bundle modal, Esc offers Clear. Own listener so the page works
  // regardless of the shell's registry; always calls the latest handler.
  const latest = useRef({
    record: handleRecordInward,
    bundle: () => {},
    charges: () => {},
    newItem: () => {},
    modalOpen: false,
    hasLines: false,
    clear: () => {},
  });
  latest.current = {
    modalOpen: isBundleModalOpen || chargeModalOpen || quickName !== null || isClearConfirmOpen,
    hasLines: lines.length > 0,
    clear: () => setIsClearConfirmOpen(true),
    record: handleRecordInward,
    charges: () => chargesRef.current?.open(),
    newItem: () => setQuickName(searchWrapRef.current?.querySelector('input')?.value.trim() ?? ''),
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
      } else if (e.altKey && e.key.toLowerCase() === 'x') {
        e.preventDefault();
        latest.current.charges();
      } else if (e.altKey && e.key.toLowerCase() === 'n') {
        e.preventDefault();
        latest.current.newItem();
      } else if (e.key === 'Escape' && !e.defaultPrevented && latest.current.hasLines) {
        // Esc with nothing else to close: offer to discard the purchase (same as New Sale).
        latest.current.clear();
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
      {/* Header: same layout as New Sale (party left; reference, code and date right). */}
      <div className="flex flex-wrap items-center justify-between gap-3 p-3.5 rounded-lg bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 shadow-xs">
        <div data-testid="purchase-vendor" className="w-80 flex-1 max-w-md">
          <PartySelector
            partyType="VENDOR"
            selectedPartyId={vendorId}
            onSelectParty={(party) => setVendorId(party ? party.id : null)}
            allowWalkIn
          />
        </div>

        <div className="flex flex-wrap items-center gap-3">
          <input
            type="text"
            aria-label="Supplier Invoice / DC #"
            title="Supplier Invoice / DC #"
            placeholder="e.g. INV-9841"
            value={supplierInvoiceRef}
            onChange={(e) => setSupplierInvoiceRef(e.target.value)}
            className="w-40 rounded-md border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-900 text-slate-900 dark:text-slate-100 text-xs px-3 py-1.5 focus:outline-none focus:ring-2 focus:ring-blue-500"
          />

          <div className="flex items-center gap-1.5 px-3 py-1 rounded-md bg-amber-50 dark:bg-amber-950/60 border border-amber-200 dark:border-amber-800">
            <span className="text-[10px] uppercase font-bold text-amber-600 dark:text-amber-400">Next Inward</span>
            <span
              data-testid="code-preview"
              title="Preview only. The final code is assigned when the purchase is recorded."
              className="text-sm font-bold font-mono text-amber-950 dark:text-amber-100"
            >
              {codePreview ?? 'PUR-…'}
            </span>
          </div>

          <div className="flex items-center gap-1 text-xs">
            <label htmlFor="purchase-date" className="text-slate-400">
              Date:
            </label>
            <input
              id="purchase-date"
              type="date"
              value={inwardDate}
              max={todayLocal()}
              onChange={(e) => setInwardDate(e.target.value)}
              className="bg-transparent font-mono font-medium text-slate-800 dark:text-slate-200 focus:outline-none"
            />
          </div>
        </div>
      </div>

      {/* Entry row: same order as New Sale (Item, Qty, Unit, Price, Add). */}
      <div
        onKeyDown={onDraftKeyDown}
        className="p-3.5 rounded-lg bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 shadow-xs flex flex-wrap lg:flex-nowrap items-start gap-2 text-xs"
      >
        <EntryField
          label="Item"
          required
          className="flex-1 min-w-[220px]"
          helper={
            selectedItem && (
              <span data-testid="selected-item" title={selectedItem.name}>
                Selected: <b className="text-slate-800 dark:text-slate-200">{selectedItem.name}</b> (
                {selectedItem.baseUnit})
              </span>
            )
          }
        >
          <div ref={searchWrapRef} data-purchase-search data-testid="purchase-item-search">
            <ItemSearchTypeahead
              onSelectItem={chooseItem}
              onCreate={(q) => setQuickName(q)}
              autoFocus
              placeholder="Search item by name or barcode…"
            />
          </div>
        </EntryField>

        <EntryField
          label="Quantity"
          required
          htmlFor="purchase-qty"
          className="w-36 shrink-0"
          helper={
            selectedBundle && unitsPerBundle > 1 ? (
              <span className="font-mono tabular-nums">
                {quantity} {selectedBundle.bundleName} × {unitsPerBundle} ={' '}
                {Math.round(quantity * unitsPerBundle * 1e6) / 1e6} {selectedItem?.baseUnit}
              </span>
            ) : null
          }
        >
          <QuantityInput
            id="purchase-qty"
            quantity={quantity}
            onChange={setQuantity}
            baseUnit={selectedItem?.baseUnit ?? 'PCS'}
            unitsPerBundle={unitsPerBundle}
            bundleName={selectedBundle?.bundleName}
            min={0.001}
            allowDecimals
            hideConversion
            disabled={!selectedItem}
          />
        </EntryField>

        <EntryField label="Unit" htmlFor="purchase-unit" className="w-44 shrink-0">
          <Select<string | number>
            id="purchase-unit"
            value={unitChoice}
            onChange={(v) => {
              // The sentinel opens the modal; the unit stays as it was (FR-29).
              if (v === NEW_BUNDLE) latest.current.bundle();
              else setUnitChoice(Number(v));
            }}
            options={unitOptions}
            disabled={!selectedItem}
            aria-label="Unit"
          />
        </EntryField>

        <EntryField
          label={`Purchase Price / ${packLabel || 'unit'}`}
          required
          htmlFor="purchase-price"
          className="w-52 shrink-0"
        >
          <TextInput
            ref={costRef}
            id="purchase-price"
            type="number"
            min={0}
            step="any"
            value={unitCostText}
            onChange={(e) => setUnitCostText(e.target.value)}
            autoSelectOnFocus
            className="text-right font-mono"
            disabled={!selectedItem}
          />
        </EntryField>

        <EntryField label="" className="shrink-0">
          <Button
            type="button"
            variant="primary"
            size="md"
            onClick={handleAddLine}
            leftIcon={<Plus className="h-4 w-4" />}
          >
            Add Line
          </Button>
        </EntryField>

        {draftError && (
          <p role="alert" className="basis-full text-xs text-rose-600 dark:text-rose-400 font-medium">
            {draftError}
          </p>
        )}
      </div>

      {/* Lines table: same column order as the sale cart (price per base unit where MRP is). */}
      <div ref={tableCardRef} className="flex-1 min-h-[220px] rounded-lg border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 overflow-hidden shadow-xs">
        <Table dense>
          <TableHeader>
            <TableRow>
              <TableHead className="w-10 text-center">#</TableHead>
              <TableHead>Item Name</TableHead>
              <TableHead>Packaging</TableHead>
              <TableHead className="text-center w-40">Quantity</TableHead>
              <TableHead align="right">Price / base unit</TableHead>
              <TableHead align="right">Purchase Price</TableHead>
              <TableHead align="right">Line Total</TableHead>
              <TableHead className="w-12 text-center">Action</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {lines.length === 0 ? (
              <TableRow>
                <TableCell colSpan={8} className="py-12 text-center text-slate-400 text-xs">
                  No inward lines yet. Pick an item above and press Enter to add it.
                </TableCell>
              </TableRow>
            ) : (
              lines.map((l, idx) => (
                <TableRow key={l.key} data-testid="purchase-line">
                  <TableCell className="text-center text-slate-400 select-none">{idx + 1}</TableCell>
                  <TableCell>
                    <div className="flex flex-col">
                      <span className="font-medium text-slate-900 dark:text-slate-100">{l.itemName}</span>
                      <span className="text-[10px] text-slate-500 dark:text-slate-400 font-mono">
                        {l.quantity} {l.packLabel}
                        {l.bundleId !== null ? ` × ${l.unitsPerBundle}` : ''}
                      </span>
                    </div>
                  </TableCell>
                  <TableCell>
                    <span className="inline-flex items-center px-1.5 py-0.5 rounded text-[10px] font-medium bg-slate-100 dark:bg-slate-800 text-slate-700 dark:text-slate-300">
                      {l.bundleId !== null ? `${l.packLabel} (× ${l.unitsPerBundle})` : l.baseUnit}
                    </span>
                  </TableCell>
                  <TableCell className="text-center font-mono font-medium">
                    <div className="flex flex-col items-center">
                      <span>
                        {l.quantity} {l.packLabel}
                      </span>
                      {l.bundleId !== null && (
                        <span className="text-[10px] text-blue-600 dark:text-blue-400">
                          = {lineBaseQuantity(l)} {l.baseUnit}
                        </span>
                      )}
                    </div>
                  </TableCell>
                  <TableCell align="right" mono className="text-slate-500 text-xs">
                    {lineCostPerBase(l).toFixed(4)}
                  </TableCell>
                  <TableCell align="right" mono>
                    <CurrencyDisplay amount={l.unitCost} size="sm" />
                  </TableCell>
                  <TableCell align="right" mono className="font-semibold text-slate-900 dark:text-slate-100">
                    {totals?.lineTotals[idx] != null ? <CurrencyDisplay amount={totals.lineTotals[idx]} size="sm" /> : '—'}
                  </TableCell>
                  <TableCell className="text-center">
                    <button
                      type="button"
                      title="Remove line"
                      aria-label={`Remove ${l.itemName}`}
                      onClick={() => handleRemoveLine(l.key)}
                      className="p-1 rounded text-slate-400 hover:text-rose-600 dark:hover:text-rose-400 transition-colors cursor-pointer"
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

      <BillChargesPanel
        ref={chargesRef}
        side="PURCHASE"
        lines={charges}
        onAdd={addCharge}
        onUpdateAmount={updateChargeAmount}
        onRemove={removeCharge}
        onDone={focusSearch}
        onModalOpenChange={setChargeModalOpen}
      />

      {/* Footer: same layout as New Sale (counts + notes left; totals, payment, messages, buttons right). */}
      <div className="p-4 rounded-lg bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 shadow-xs flex flex-wrap items-end justify-between gap-4">
        <div className="flex flex-col gap-2 min-w-[200px] flex-1 max-w-sm">
          <div className="flex flex-col text-xs text-slate-500 font-mono">
            <span>Lines: {lines.length}</span>
            <span>Total Base Units: {Number(totalBaseUnits.toFixed(3))}</span>
          </div>
          <input
            type="text"
            aria-label="Notes"
            placeholder="Notes (optional)"
            value={notes}
            onChange={(e) => setNotes(e.target.value)}
            className="w-full rounded-md border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-900 text-slate-900 dark:text-slate-100 text-xs px-3 py-1.5 focus:outline-none focus:ring-2 focus:ring-blue-500"
          />
        </div>

        <div data-testid="purchase-footer-actions" className="flex flex-col items-end gap-3 ml-auto">
          <BillFooterTotals
            testIdPrefix="purchase"
            hasCharges={charges.length > 0}
            itemsSubtotal={itemsSubtotal}
            chargesTotal={totals?.chargesTotal ?? 0}
            discountsTotal={totals?.discountsTotal ?? 0}
            totalAmount={total}
          />

          <div className="flex flex-wrap items-center justify-end gap-3">
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
                      : 'text-slate-600 dark:text-slate-400 hover:text-slate-900 dark:hover:text-slate-100'
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
            <div className="flex flex-col items-end text-xs">
              <span className="text-slate-500">Balance due (vendor payable)</span>
              <CurrencyDisplay amount={balanceDue} size="md" />
            </div>
          </div>

          {/* Messages sit right above the buttons so a refused click is never silent. */}
          {error ? (
            <p role="alert" className="max-w-md text-right text-xs text-rose-600 dark:text-rose-400 font-medium">
              {error}
            </p>
          ) : (
            paymentError && (
              <p data-testid="payment-hint" className="max-w-md text-right text-xs text-amber-700 dark:text-amber-400">
                {paymentError}
              </p>
            )
          )}
          {successCode && (
            <p
              role="status"
              data-testid="purchase-success"
              className="text-sm text-emerald-700 dark:text-emerald-400 font-medium"
            >
              Purchase recorded: <span className="font-mono font-bold">{successCode}</span>
            </p>
          )}

          <div className="flex items-center gap-2">
            <Button
              variant="outline"
              size="md"
              disabled={lines.length === 0}
              onClick={() => setIsClearConfirmOpen(true)}
              hotkey="Esc"
              leftIcon={<Trash2 className="h-4 w-4" />}
            >
              Clear
            </Button>
            <Button
              variant="primary"
              size="lg"
              data-testid="record-purchase-btn"
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
      </div>

      <ConfirmDialog
        isOpen={isClearConfirmOpen}
        title="Clear Current Purchase?"
        message="Discard every line, charge and note on this purchase? This cannot be undone."
        variant="danger"
        confirmText="Discard Purchase"
        cancelText="Keep Lines"
        onConfirm={() => {
          clearDraft();
          setIsClearConfirmOpen(false);
          focusSearch();
        }}
        onCancel={() => {
          setIsClearConfirmOpen(false);
          focusSearch();
        }}
      />

      <BundleCreationModal
        isOpen={isBundleModalOpen}
        onClose={() => setIsBundleModalOpen(false)}
        initialItemId={selectedItem?.itemId}
        baseUnit={selectedItem?.baseUnit}
        onBundleCreated={handleBundleCreated}
      />

      <ItemQuickCreateModal
        isOpen={quickName !== null}
        initialName={quickName ?? ''}
        withStock={false}
        onClose={() => {
          setQuickName(null);
          setTimeout(() => searchWrapRef.current?.querySelector('input')?.focus(), 0);
        }}
        onCreated={(it) => {
          setQuickName(null);
          chooseItem(it);
        }}
      />
    </div>
  );
};

export { NewPurchasePage as PurchasesPage };
export default NewPurchasePage;
