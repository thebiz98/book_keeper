import React, { useState, useEffect } from 'react';
import { Plus, Trash2, CheckCircle2 } from 'lucide-react';
import { api } from '@/api';
import type { CreatePurchasePayload } from '@/types';
import { useAppStore } from '@/store/useAppStore';
import { PartySelector } from '@/components/domain/PartySelector';
import { BundleCreationModal } from '@/components/domain/BundleCreationModal';
import { CurrencyDisplay } from '@/components/common/CurrencyDisplay';
import { Button } from '@/components/ui/Button';
import { Table, TableHeader, TableBody, TableRow, TableHead, TableCell } from '@/components/ui/Table';

interface PurchaseLineItem {
  itemId: number;
  itemName: string;
  bundleId?: number | null;
  bundleName?: string;
  unitsPerBundle: number;
  quantity: number;
  baseQuantity: number;
  unitCost: number;
  mrp?: number;
  defaultSalePrice?: number;
  lineTotal: number;
}

export const NewPurchasePage: React.FC = () => {
  const recordCashFlow = useAppStore((s) => s.recordCashFlow);

  const [purchaseCode, setPurchaseCode] = useState<string>('PUR-1001');
  const [inwardDate, setInwardDate] = useState<string>(
    new Date().toISOString().split('T')[0]
  );
  const [vendorId, setVendorId] = useState<number | null>(null);
  const [vendorInvoiceRef, setVendorInvoiceRef] = useState<string>('');
  const [notes, setNotes] = useState<string>('');

  // Current inward row draft
  const [itemName, setItemName] = useState<string>('');
  const [quantity, setQuantity] = useState<number>(1);
  const [unitsPerBundle, setUnitsPerBundle] = useState<number>(1);
  const [unitCost, setUnitCost] = useState<number>(0);
  const [mrp, setMrp] = useState<number>(0);
  const [defaultSalePrice, setDefaultSalePrice] = useState<number>(0);

  // Line items list
  const [items, setItems] = useState<PurchaseLineItem[]>([]);

  // Payment status
  const [paymentStatus, setPaymentStatus] = useState<'PAID' | 'PARTIAL' | 'UNPAID'>('PAID');
  const [paidAmount, setPaidAmount] = useState<number>(0);

  const [isBundleModalOpen, setIsBundleModalOpen] = useState<boolean>(false);
  const [isSubmitting, setIsSubmitting] = useState<boolean>(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    api
      .getNextPurchaseCode()
      .then((code) => {
        if (code) setPurchaseCode(code);
      })
      .catch(() => {});
  }, []);

  const totalCost = items.reduce((acc, it) => acc + it.lineTotal, 0);

  useEffect(() => {
    if (paymentStatus === 'PAID') {
      setPaidAmount(totalCost);
    } else if (paymentStatus === 'UNPAID') {
      setPaidAmount(0);
    }
  }, [totalCost, paymentStatus]);

  const handleAddLineItem = () => {
    if (!itemName.trim()) {
      setError('Item name is required');
      return;
    }
    if (quantity <= 0) {
      setError('Quantity must be greater than 0');
      return;
    }
    if (unitCost <= 0) {
      setError('Unit cost must be greater than 0');
      return;
    }

    setError(null);
    const lineTotal = quantity * unitCost;
    const baseQuantity = quantity * unitsPerBundle;

    const newItem: PurchaseLineItem = {
      itemId: Date.now(),
      itemName: itemName.trim(),
      unitsPerBundle,
      quantity,
      baseQuantity,
      unitCost,
      mrp: mrp > 0 ? mrp : undefined,
      defaultSalePrice: defaultSalePrice > 0 ? defaultSalePrice : undefined,
      lineTotal,
    };

    setItems((prev) => [...prev, newItem]);

    // Reset draft row
    setItemName('');
    setQuantity(1);
    setUnitsPerBundle(1);
    setUnitCost(0);
    setMrp(0);
    setDefaultSalePrice(0);
  };

  const handleRemoveItem = (index: number) => {
    setItems((prev) => prev.filter((_, idx) => idx !== index));
  };

  const handleRecordInward = async () => {
    if (items.length === 0) {
      setError('Cannot record inward purchase with zero line items');
      return;
    }

    setIsSubmitting(true);
    setError(null);

    const payload: CreatePurchasePayload = {
      transactionDate: inwardDate,
      partyId: vendorId ?? null,
      // TODO(F6-b): send supplierInvoiceRef as its own field instead of packing it into notes.
      supplierInvoiceRef: vendorInvoiceRef.trim() || null,
      paymentStatus,
      notes: vendorInvoiceRef
        ? `Supplier Inv #${vendorInvoiceRef}. ${notes}`.trim()
        : notes.trim() || null,
      paidAmount: paymentStatus === 'UNPAID' ? 0 : paidAmount,
      paymentMode: 'CASH',
      // No catalog bundle is selected on this screen, so lines go inward in base units.
      items: items.map((it) => ({
        itemId: it.itemId,
        bundleId: it.bundleId ?? null,
        bundleQuantity: it.bundleId ? it.quantity : it.quantity * it.unitsPerBundle,
        unitCost: it.bundleId ? it.unitCost : it.unitCost / it.unitsPerBundle,
      })),
    };

    try {
      await api.createPurchase(payload);

      // Record physical cash outflow if paid from cash drawer
      const actualPaid = paymentStatus === 'UNPAID' ? 0 : paidAmount;
      if (actualPaid > 0) {
        recordCashFlow({ purchaseCash: actualPaid });
      }

      // Reset form & get next code
      setItems([]);
      setVendorInvoiceRef('');
      setNotes('');
      const nextCode = await api.getNextPurchaseCode();
      if (nextCode) setPurchaseCode(nextCode);
    } catch (err: any) {
      setError(err?.message || 'Failed to record inward purchase');
    } finally {
      setIsSubmitting(false);
    }
  };

  return (
    <div className="flex flex-col gap-4">
      {/* Header Info Bar */}
      <div className="flex flex-wrap items-center justify-between gap-3 p-3.5 rounded-lg bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 shadow-xs">
        <div className="flex items-center gap-3">
          <div className="flex items-center gap-1.5 px-3 py-1 rounded-md bg-amber-50 dark:bg-amber-950/60 border border-amber-200 dark:border-amber-800">
            <span className="text-[10px] uppercase font-bold text-amber-600 dark:text-amber-400">
              Inward
            </span>
            <span className="text-sm font-bold font-mono text-amber-950 dark:text-amber-100">
              {purchaseCode}
            </span>
          </div>

          <div className="flex items-center gap-1 text-xs">
            <label className="text-slate-400">Date:</label>
            <input
              type="date"
              value={inwardDate}
              onChange={(e) => setInwardDate(e.target.value)}
              className="bg-transparent font-mono font-medium text-slate-800 dark:text-slate-200 focus:outline-none"
            />
          </div>

          <div className="flex items-center gap-1 text-xs">
            <label className="text-slate-400">Supplier Inv #:</label>
            <input
              type="text"
              placeholder="e.g. INV-9841"
              value={vendorInvoiceRef}
              onChange={(e) => setVendorInvoiceRef(e.target.value)}
              className="w-28 px-2 py-0.5 rounded border border-slate-200 dark:border-slate-700 bg-transparent text-xs"
            />
          </div>
        </div>

        <div className="w-72">
          <PartySelector
            partyType="VENDOR"
            selectedPartyId={vendorId}
            onSelectParty={(party) => setVendorId(party ? party.id : null)}
            allowWalkIn
          />
        </div>
      </div>

      {/* Inward Line Item Entry Row */}
      <div className="p-3.5 rounded-lg bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 shadow-xs flex flex-wrap items-end gap-2 text-xs">
        <div className="flex-1 min-w-[200px] flex flex-col gap-1">
          <label className="font-semibold text-slate-700 dark:text-slate-300">
            Item Name *
          </label>
          <input
            type="text"
            placeholder="e.g. Classmate Notebook 120p, Reynolds 045..."
            value={itemName}
            onChange={(e) => setItemName(e.target.value)}
            className="w-full rounded-md border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-900 px-3 py-1.5 focus:outline-none focus:ring-2 focus:ring-blue-500"
          />
        </div>

        <div className="w-24 flex flex-col gap-1">
          <label className="font-semibold text-slate-700 dark:text-slate-300">Quantity *</label>
          <input
            type="number"
            min={1}
            value={quantity}
            onChange={(e) => setQuantity(parseInt(e.target.value, 10) || 1)}
            className="w-full rounded-md border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-900 px-3 py-1.5 text-right font-mono tabular-nums focus:outline-none focus:ring-2 focus:ring-blue-500"
          />
        </div>

        <div className="w-28 flex flex-col gap-1">
          <label className="font-semibold text-slate-700 dark:text-slate-300">Multiplier</label>
          <input
            type="number"
            min={1}
            value={unitsPerBundle}
            onChange={(e) => setUnitsPerBundle(parseInt(e.target.value, 10) || 1)}
            title="Units per box/bundle"
            className="w-full rounded-md border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-900 px-3 py-1.5 text-right font-mono tabular-nums focus:outline-none focus:ring-2 focus:ring-blue-500"
          />
        </div>

        <div className="w-28 flex flex-col gap-1">
          <label className="font-semibold text-slate-700 dark:text-slate-300">Unit Cost (₹) *</label>
          <input
            type="number"
            min={0}
            step="any"
            value={unitCost || ''}
            onChange={(e) => setUnitCost(parseFloat(e.target.value) || 0)}
            className="w-full rounded-md border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-900 px-3 py-1.5 text-right font-mono tabular-nums focus:outline-none focus:ring-2 focus:ring-blue-500"
          />
        </div>

        <div className="w-24 flex flex-col gap-1">
          <label className="font-semibold text-slate-700 dark:text-slate-300">MRP (₹)</label>
          <input
            type="number"
            min={0}
            step="any"
            value={mrp || ''}
            onChange={(e) => setMrp(parseFloat(e.target.value) || 0)}
            className="w-full rounded-md border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-900 px-3 py-1.5 text-right font-mono tabular-nums focus:outline-none focus:ring-2 focus:ring-blue-500"
          />
        </div>

        <Button
          type="button"
          variant="primary"
          size="md"
          onClick={handleAddLineItem}
          leftIcon={<Plus className="h-4 w-4" />}
        >
          Add Batch
        </Button>
      </div>

      {/* Inward Items Table */}
      <div className="flex-1 min-h-[220px] rounded-lg border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 overflow-hidden shadow-xs">
        <Table dense>
          <TableHeader>
            <TableRow>
              <TableHead className="w-10 text-center">#</TableHead>
              <TableHead>Item Name</TableHead>
              <TableHead className="text-center">Inward Qty</TableHead>
              <TableHead className="text-center">Total Base Units</TableHead>
              <TableHead align="right">Unit Cost (₹)</TableHead>
              <TableHead align="right">MRP (₹)</TableHead>
              <TableHead align="right">Line Total (₹)</TableHead>
              <TableHead className="w-12 text-center">Action</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {items.length === 0 ? (
              <TableRow>
                <TableCell colSpan={8} className="py-12 text-center text-slate-400 text-xs">
                  No inward batch items added. Enter details above and click Add Batch.
                </TableCell>
              </TableRow>
            ) : (
              items.map((it, idx) => (
                <TableRow key={it.itemId}>
                  <TableCell className="text-center text-slate-400 select-none">
                    {idx + 1}
                  </TableCell>
                  <TableCell className="font-semibold text-slate-900 dark:text-slate-100">
                    {it.itemName}
                  </TableCell>
                  <TableCell className="text-center font-mono font-medium">
                    {it.quantity} {it.unitsPerBundle > 1 ? 'Bundles' : 'Units'}
                  </TableCell>
                  <TableCell className="text-center font-mono font-medium text-blue-600 dark:text-blue-400">
                    {it.baseQuantity} PCS
                  </TableCell>
                  <TableCell align="right" mono>
                    <CurrencyDisplay amount={it.unitCost} size="sm" />
                  </TableCell>
                  <TableCell align="right" mono className="text-slate-500">
                    {it.mrp ? <CurrencyDisplay amount={it.mrp} size="sm" /> : '—'}
                  </TableCell>
                  <TableCell align="right" mono className="font-bold text-slate-900 dark:text-slate-100">
                    <CurrencyDisplay amount={it.lineTotal} size="sm" />
                  </TableCell>
                  <TableCell className="text-center">
                    <button
                      type="button"
                      onClick={() => handleRemoveItem(idx)}
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
            <span className="text-xs text-slate-500 font-medium">Total Inward Value</span>
            <div className="text-2xl font-bold font-mono text-slate-900 dark:text-slate-100">
              <CurrencyDisplay amount={totalCost} size="xl" />
            </div>
          </div>

          <div className="h-8 w-px bg-slate-200 dark:bg-slate-800" />

          <div className="flex flex-col text-xs text-slate-500 font-mono">
            <span>Batches: {items.length}</span>
            <span>Total Inward Units: {items.reduce((a, b) => a + b.baseQuantity, 0)} PCS</span>
          </div>
        </div>

        <div className="flex items-center gap-3">
          <div className="flex p-0.5 rounded-lg border border-slate-200 dark:border-slate-700 bg-slate-100 dark:bg-slate-800 text-xs">
            {(['PAID', 'PARTIAL', 'UNPAID'] as const).map((st) => (
              <button
                key={st}
                type="button"
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

          {paymentStatus === 'PARTIAL' && (
            <div className="flex items-center gap-1.5 bg-slate-50 dark:bg-slate-800 px-2 py-1 rounded border border-slate-200 dark:border-slate-700 text-xs">
              <span className="text-slate-400">Paid: ₹</span>
              <input
                type="number"
                value={paidAmount}
                onChange={(e) => setPaidAmount(parseFloat(e.target.value) || 0)}
                className="w-20 bg-transparent font-mono font-bold text-right focus:outline-none"
              />
            </div>
          )}
        </div>

        <div className="flex items-center gap-2">
          <Button
            variant="primary"
            size="lg"
            disabled={items.length === 0}
            isLoading={isSubmitting}
            onClick={handleRecordInward}
            hotkey="Ctrl+Enter"
            leftIcon={<CheckCircle2 className="h-5 w-5" />}
          >
            Record Inward Stock
          </Button>
        </div>
      </div>

      {error && (
        <p role="alert" className="text-xs text-rose-600 dark:text-rose-400 font-medium px-1">
          {error}
        </p>
      )}

      {/* Bundle Creation Modal */}
      <BundleCreationModal
        isOpen={isBundleModalOpen}
        onClose={() => setIsBundleModalOpen(false)}
      />
    </div>
  );
};

export { NewPurchasePage as PurchasesPage };
export default NewPurchasePage;
