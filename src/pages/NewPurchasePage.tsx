import React, { useState, useEffect, useRef, useCallback, useMemo } from 'react';
import { Plus, Trash2, CheckCircle2 } from 'lucide-react';
import { api } from '@/api';
import { errorMessage } from '@/api/errors';
import { todayLocal } from '@/lib/dates';
import { useAppStore } from '@/store/useAppStore';
import { peekBillEdit, takeBillEdit } from './billEditHandoff';
import { purchaseLineToDraft } from '@/lib/billEditDraft';
import { usePreview } from '@/hooks/usePreview';
import { ConfirmDialog } from '@/components/common/ConfirmDialog';
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
import { BillTotalsSummary } from '@/components/domain/BillTotalsSummary';
import { toChargeInputs, type BillChargeLine } from '@/lib/billMath';
import { lineBaseQuantity, lineCostPerBase, type PurchaseLine } from '@/components/domain/purchaseMath';
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
  // FR-20 edit mode: decided synchronously so the code preview never overwrites the bill's code.
  const editRequested = useRef(peekBillEdit()?.kind === 'PURCHASE');
  const [editing, setEditing] = useState<{ id: number; code: string } | null>(null);
  const [cancelEditOpen, setCancelEditOpen] = useState(false);
  const setActiveRoute = useAppStore((s) => s.setActiveRoute);
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
      .then((c) => {
        if (!editRequested.current) setCodePreview(c || null);
      })
      .catch(() => {
        if (!editRequested.current) setCodePreview(null);
      });
  }, []);

  useEffect(() => {
    refreshCodePreview();
  }, [refreshCodePreview]);

  // FR-20: load the queued purchase into the entry screen.
  useEffect(() => {
    const req = takeBillEdit();
    if (req?.kind !== 'PURCHASE') return;
    let live = true;
    (async () => {
      try {
        const [d, g] = await Promise.all([api.getPurchaseDetails(req.id), api.getBillEditGuard('PURCHASE', req.id)]);
        if (!live) return;
        if (!g.canEdit) {
          editRequested.current = false;
          setError(g.reasons.find((r) => r.blocksEdit)?.message ?? 'This purchase cannot be edited');
          refreshCodePreview();
          return;
        }
        const drafts: PurchaseLine[] = [];
        for (const it of d.items) {
          const lot = it.purchaseItem;
          const found = (await api.searchItemsFts(lot.itemName, 8).catch(() => [])).find((r) => r.itemId === lot.itemId);
          const lim = g.lotLimits.find((l) => l.purchaseItemId === lot.id);
          drafts.push({
            ...purchaseLineToDraft(lot, found ?? null),
            key: lineKey.current++,
            minBaseQty: lim?.consumedQuantity ?? 0,
            locked: lim?.referenced ?? false,
            costLocked: lim?.vendorReturned ?? false,
          });
        }
        if (!live) return;
        const p = d.purchase;
        setEditing({ id: p.id, code: p.transactionCode });
        setCodePreview(p.transactionCode);
        setInwardDate(p.transactionDate);
        setVendorId(p.partyId);
        setSupplierInvoiceRef(p.supplierInvoiceRef ?? '');
        setNotes(p.notes ?? '');
        setLines(drafts);
        setCharges(d.charges.map((c) => ({ key: chargeKey.current++, chargeTypeId: c.chargeTypeId, name: c.chargeName, effect: c.effect, amount: c.amount })));
        // Prefill the payment control from what is stored; the backend preview judges the result.
        const up = g.upfrontAmount;
        setPaymentStatus(p.paymentStatus === 'PAID' ? 'PAID' : up > 0 ? 'PARTIAL' : 'UNPAID');
        setPaidInput(up);
      } catch (e) {
        if (live) {
          editRequested.current = false;
          setError(errorMessage(e));
          refreshCodePreview();
        }
      }
    })();
    return () => {
      live = false;
    };
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

  const handleRemoveLine = (key: number) => setLines((prev) => prev.filter((l) => l.key !== key || l.locked));
  const patchLine = (key: number, patch: Partial<PurchaseLine>) =>
    setLines((prev) => prev.map((l) => (l.key === key ? { ...l, ...patch } : l)));

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
              ...(editing ? { purchaseItemId: l.purchaseItemId ?? null } : {}),
            })),
          },
    [lines, inwardDate, vendorId, supplierInvoiceRef, charges, notes, editing]
  );
  const totalsPreview = usePreview<PreviewPurchasePayload, BillPreview>({
    input: bill,
    fetcher: (p) => api.previewPurchase(p, editing?.id ?? null),
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
    fetcher: (p) => api.previewPurchase(p, editing?.id ?? null),
  });
  const pending = bill !== null && (totalsPreview.isLoading || payPreview.isLoading || payPreview.data === null);
  const previewErrors = [...totalsPreview.errors, ...payPreview.errors];
  const itemsSubtotal = totals?.itemsSubtotal ?? 0;
  const total = totals?.totalAmount ?? 0;
  const applied = totals?.appliedAmount ?? 0;
  const paid = paymentStatus === 'PARTIAL' ? paidInput : (payPreview.data?.payment?.paidAmount ?? paidToSend ?? 0);
  const balanceDue = payPreview.data?.payment?.balanceDue ?? totals?.openAmount ?? 0;
  const paymentError = lines.length > 0 && !pending ? (previewErrors[0]?.message ?? null) : null;

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

  const handleRecordInward = async () => {
    if (submitting.current) return;
    if (lines.length === 0) {
      setError('Add at least one line before recording the purchase');
      return;
    }
    // Saving waits for the backend's verdict on exactly what is on screen (pending or any error blocks).
    if (!bill || pending) return;
    if (previewErrors.length > 0) {
      setError(previewErrors[0].message);
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
      if (editing) {
        await api.updatePurchase(editing.id, payload);
        setEditing(null);
        setLines([]);
        setCharges([]);
        setActiveRoute('purchase-history');
        return;
      }
      const res = await api.createPurchase(payload);
      setSuccessCode(res.transactionCode);
      setLines([]);
      setCharges([]);
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
  const latest = useRef({
    record: handleRecordInward,
    bundle: () => {},
    charges: () => {},
    newItem: () => {},
    modalOpen: false,
  });
  latest.current = {
    modalOpen: isBundleModalOpen || chargeModalOpen || quickName !== null,
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
      {editing && (
        <div
          data-testid="edit-banner"
          className="flex items-center justify-between gap-3 px-3.5 py-2 rounded-lg bg-amber-50 dark:bg-amber-950/40 border border-amber-300 dark:border-amber-800 text-xs text-amber-900 dark:text-amber-200"
        >
          <span>
            Editing <span className="font-mono font-bold">{editing.code}</span> — saving replaces the original bill
            {applied > 0 && <> · Already paid later: ₹{applied.toFixed(2)}</>}
          </span>
          <Button variant="outline" size="sm" data-testid="cancel-edit" onClick={() => setCancelEditOpen(true)}>
            Cancel edit
          </Button>
        </div>
      )}
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
              onCreate={(q) => setQuickName(q)}
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
            label={`Purchase Price (₹ / ${packLabel || 'unit'}) *`}
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
              <TableHead align="right">Purchase Price (₹)</TableHead>
              <TableHead align="right">Purchase Price / base unit (₹)</TableHead>
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
                    {editing ? (
                      <div className="flex flex-col items-center gap-0.5">
                        <input
                          type="number"
                          aria-label={`Quantity ${l.itemName}`}
                          min={0.001}
                          step="any"
                          value={l.quantity}
                          onChange={(e) => patchLine(l.key, { quantity: parseFloat(e.target.value) || 0 })}
                          className="w-20 text-right bg-transparent border border-slate-300 dark:border-slate-700 rounded px-1"
                        />
                        {!!l.minBaseQty && l.minBaseQty > 0 && (
                          <span data-testid="used-hint" className="text-[10px] text-slate-500">
                            Used: {l.minBaseQty}
                          </span>
                        )}
                      </div>
                    ) : (
                      <>
                        {l.quantity} {l.packLabel}
                      </>
                    )}
                  </TableCell>
                  <TableCell className="text-center font-mono font-medium text-blue-600 dark:text-blue-400">
                    {lineBaseQuantity(l)} {l.baseUnit}
                  </TableCell>
                  <TableCell align="right" mono>
                    {editing ? (
                      <input
                        type="number"
                        aria-label={`Unit cost ${l.itemName}`}
                        min={0}
                        step="any"
                        value={l.unitCost}
                        disabled={l.costLocked}
                        title={l.costLocked ? 'Returned to the vendor at this cost: delete that vendor return to change it' : undefined}
                        onChange={(e) => patchLine(l.key, { unitCost: parseFloat(e.target.value) || 0 })}
                        className="w-24 text-right bg-transparent border border-slate-300 dark:border-slate-700 rounded px-1 disabled:opacity-60"
                      />
                    ) : (
                      <CurrencyDisplay amount={l.unitCost} size="sm" />
                    )}
                  </TableCell>
                  <TableCell align="right" mono className="text-slate-500">
                    {lineCostPerBase(l).toFixed(4)}
                  </TableCell>
                  <TableCell align="right" mono className="font-bold text-slate-900 dark:text-slate-100">
                    {totals?.lineTotals[idx] != null ? <CurrencyDisplay amount={totals.lineTotals[idx]} size="sm" /> : '—'}
                  </TableCell>
                  <TableCell className="text-center">
                    <button
                      type="button"
                      aria-label={`Remove ${l.itemName}`}
                      disabled={l.locked}
                      title={l.locked ? 'Already sold or moved: this line cannot be removed' : undefined}
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

      {/* Footer Settlement & Commit Bar */}
      <div className="p-4 rounded-lg bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 shadow-xs flex flex-wrap items-center justify-between gap-4">
        <div className="flex items-center gap-6">
          <div className="flex flex-col">
            <span className="text-xs text-slate-500 font-medium">
              {charges.length > 0 ? 'Grand Total (payable)' : 'Total Purchase Value'}
            </span>
            <div data-testid="purchase-total" className="text-2xl font-bold font-mono text-slate-900 dark:text-slate-100">
              <CurrencyDisplay amount={total} size="xl" />
            </div>
          </div>
          {charges.length > 0 && (
            <div className="w-56" data-testid="purchase-breakdown">
              <span data-testid="purchase-items-subtotal" className="sr-only">
                {itemsSubtotal}
              </span>
              <BillTotalsSummary compact itemsSubtotal={itemsSubtotal} lines={charges} grandTotal={total} />
            </div>
          )}
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
            {editing ? 'Update Purchase' : 'Record Inward Stock'}
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

      <ConfirmDialog
        isOpen={cancelEditOpen}
        title="Discard changes?"
        message={`Leave the edit of ${editing?.code ?? 'this purchase'} without saving. The original bill stays as it was.`}
        variant="warning"
        confirmText="Discard changes"
        cancelText="Keep editing"
        onConfirm={() => {
          setCancelEditOpen(false);
          setEditing(null);
          setLines([]);
          setCharges([]);
          setActiveRoute('purchase-history');
        }}
        onCancel={() => setCancelEditOpen(false)}
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
