import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { usePreview } from '@/hooks/usePreview';
import { Trash2 } from 'lucide-react';
import { api, errorMessage } from '@/api';
import { todayLocal } from '@/lib/dates';
import type {
  ItemSearchResult,
  Party,
  PurchaseDetailView,
  ReturnResult,
  VendorReturnItemPayload,
  VendorReturnLot,
  VendorReturnPayload,
} from '@/types';
import { Button } from '../ui/Button';
import { NumericInput } from '../ui/NumericInput';
import { Select } from '../ui/Select';
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '../ui/Table';
import { CurrencyDisplay } from '../common/CurrencyDisplay';
import { ItemSearchTypeahead } from './ItemSearchTypeahead';
import { PartySelector } from './PartySelector';
import {
  BillPicker,
  FieldLabel,
  RefundPreview,
  ReturnResultPanel,
  inputCls,
} from './ReturnFormShared';
import { refreshAppMetrics } from './navParams';

interface BillLine {
  purchaseItemId: number;
  itemId: number;
  name: string;
  remaining: number;
  cost: number;
  qty: number;
}

interface VendorLine {
  key: number;
  itemId: number;
  name: string;
  baseUnit: string;
  /** DD-86: the backend's eligible lots for this vendor, in the order received; null = loading. */
  lots: VendorReturnLot[] | null;
  /** String(purchaseItemId), or '' while none is chosen. */
  lot: string;
  qty: number;
}

const money = (v: number) => `₹${v.toFixed(2)}`;

export function lotLabel(l: VendorReturnLot): string {
  const net = Math.abs(l.netUnitCost - l.unitCost) > 0.0001 ? ` (net ${money(l.netUnitCost)})` : '';
  return `${l.purchaseCode} · ${l.transactionDate} · ${money(l.unitCost)}${net} · ${l.remainingQuantity} ${l.baseUnit} left`;
}


export interface ReturnFormVendorProps {
  initialBillId?: number | null;
  onDone?: () => void;
}

export const ReturnFormVendor: React.FC<ReturnFormVendorProps> = ({ initialBillId = null, onDone }) => {
  const [mode, setMode] = useState<'BILL' | 'VENDOR'>('BILL');
  const [detail, setDetail] = useState<PurchaseDetailView | null>(null);
  const [billLines, setBillLines] = useState<BillLine[]>([]);
  const [vendor, setVendor] = useState<Party | null>(null);
  const [lines, setLines] = useState<VendorLine[]>([]);
  const [date, setDate] = useState(todayLocal());
  const [reason, setReason] = useState('');
  const [notes, setNotes] = useState('');
  const [loadError, setLoadError] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [result, setResult] = useState<ReturnResult | null>(null);

  const loadBill = useCallback((id: number) => {
    setLoadError(null);
    setDetail(null);
    setBillLines([]);
    api
      .getPurchaseDetails(id)
      .then((d) => {
        setDetail(d);
        setBillLines(
          d.items.map((it) => ({
            purchaseItemId: it.purchaseItem.id,
            itemId: it.purchaseItem.itemId,
            name: it.itemName,
            remaining: it.purchaseItem.remainingQuantity,
            cost: it.purchaseItem.unitCost,
            qty: 0,
          }))
        );
      })
      .catch((e) => setLoadError(errorMessage(e)));
  }, []);

  useEffect(() => {
    if (initialBillId) {
      setMode('BILL');
      loadBill(initialBillId);
    }
  }, [initialBillId, loadBill]);

  const patchBill = (i: number, qty: number) => setBillLines((ls) => ls.map((l, idx) => (idx === i ? { ...l, qty } : l)));
  const patchLine = (key: number, p: Partial<VendorLine>) =>
    setLines((ls) => ls.map((l) => (l.key === key ? { ...l, ...p } : l)));

  // DD-86: the lot list comes from the backend; a late answer for another vendor is dropped.
  const vendorRef = useRef<number | null>(null);
  const loadLots = useCallback((key: number, itemId: number, name: string, partyId: number) => {
    api
      .listVendorReturnLots(itemId, partyId, null)
      .then((lots) => {
        if (vendorRef.current !== partyId) return;
        setLines((ls) =>
          ls.map((l) =>
            l.key === key ? { ...l, lots, lot: lots.length === 1 ? String(lots[0].purchaseItemId) : '' } : l
          )
        );
      })
      .catch((e) => setLoadError(`Could not load lots for ${name}: ${errorMessage(e)}`));
  }, []);

  const pickVendor = (p: Party | null) => {
    setVendor(p);
    vendorRef.current = p?.id ?? null;
    setLines((ls) => ls.map((l) => ({ ...l, lots: p ? null : [], lot: '' })));
    if (p) lines.forEach((l) => loadLots(l.key, l.itemId, l.name, p.id));
  };

  const addItem = (it: ItemSearchResult) => {
    const key = Date.now() + Math.random();
    setLines((ls) => [
      ...ls,
      { key, itemId: it.itemId, name: it.name, baseUnit: it.baseUnit, lots: vendor ? null : [], lot: '', qty: 1 },
    ]);
    if (vendor) loadLots(key, it.itemId, it.name, vendor.id);
  };

  // DD-56: the value (lot cost net of the bill discount), the credited/cash split and every
  // refusal (no lot, lot not eligible, more than remaining ...) come from `preview_vendor_return`.
  const payload = useMemo<VendorReturnPayload | null>(() => {
    const items: VendorReturnItemPayload[] =
      mode === 'BILL'
        ? billLines
            .filter((l) => l.qty > 0)
            .map((l) => ({ itemId: l.itemId, purchaseItemId: l.purchaseItemId, quantity: l.qty }))
        : lines.map((l) => ({ itemId: l.itemId, purchaseItemId: l.lot ? Number(l.lot) : null, quantity: l.qty }));
    if (items.length === 0 || (mode === 'BILL' && !detail) || (mode === 'VENDOR' && !vendor)) return null;
    return {
      originalPurchaseId: mode === 'BILL' ? detail?.purchase.id ?? null : null,
      partyId: mode === 'VENDOR' ? vendor?.id ?? null : null,
      returnDate: date,
      settlementType: 'REFUND',
      reason: reason.trim() || null,
      notes: notes.trim() || null,
      items,
    };
  }, [mode, billLines, lines, detail, vendor, date, reason, notes]);
  const preview = usePreview<VendorReturnPayload, ReturnResult>({
    input: payload,
    fetcher: (p) => api.previewVendorReturn(p),
  });

  const submit = async () => {
    setError(null);
    if (mode === 'BILL' && !detail) return setError('Select a purchase bill first');
    if (mode === 'VENDOR' && !vendor) return setError('Select a vendor first');
    if (!payload) return setError('Enter a return quantity for at least one line');
    if (preview.isLoading) return;
    if (preview.errors.length > 0) return setError(preview.errors[0].message);
    setBusy(true);
    try {
      const res = await api.createVendorReturn(payload);
      setResult(res);
      refreshAppMetrics();
      onDone?.();
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  };

  const reset = () => {
    setResult(null);
    setDetail(null);
    setBillLines([]);
    setLines([]);
    setNotes('');
    setReason('');
    setError(null);
  };

  if (result) return <ReturnResultPanel result={result} onAnother={reset} />;

  return (
    <div className="flex flex-col gap-4">
      <div className="flex gap-4 text-xs">
        {(['BILL', 'VENDOR'] as const).map((m) => (
          <label key={m} className="flex items-center gap-1.5 cursor-pointer">
            <input type="radio" name="vreturn-mode" checked={mode === m} onChange={() => setMode(m)} />
            {m === 'BILL' ? 'From purchase bill' : 'By vendor and lot'}
          </label>
        ))}
      </div>

      {loadError && (
        <p role="alert" className="text-xs text-rose-600 dark:text-rose-400">
          {loadError}
        </p>
      )}

      {mode === 'BILL' ? (
        <>
          {!detail && <BillPicker kind="PURCHASE" onPick={loadBill} />}
          {detail && (
            <div className="flex flex-col gap-2">
              <div className="flex items-center justify-between text-xs bg-slate-50 dark:bg-slate-800 p-2.5 rounded-md">
                <span>
                  <span className="font-mono font-semibold">{detail.purchase.transactionCode}</span> · {detail.purchase.transactionDate} ·{' '}
                  {detail.purchase.partyName ?? '-'} · payable <CurrencyDisplay amount={detail.purchase.balanceDue} size="sm" />
                </span>
                <Button size="xs" variant="ghost" onClick={() => { setDetail(null); setBillLines([]); }}>
                  Change bill
                </Button>
              </div>
              <Table dense scroll="bounded">
                <TableHeader>
                  <TableRow className="text-left text-slate-400">
                    <TableHead className="py-1">Item</TableHead>
                    <TableHead className="text-right">Remaining</TableHead>
                    <TableHead className="text-right">Lot cost</TableHead>
                    <TableHead className="w-28">Return qty</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {billLines.map((l, i) => (
                    <TableRow key={l.purchaseItemId} >
                      <TableCell className="py-1 font-medium">{l.name}</TableCell>
                      <TableCell className="text-right font-mono">{l.remaining}</TableCell>
                      <TableCell className="text-right">
                        <CurrencyDisplay amount={l.cost} size="sm" />
                      </TableCell>
                      <TableCell>
                        <NumericInput
                          aria-label={`Return quantity for ${l.name}`}
                          value={l.qty}
                          onChange={(v) => patchBill(i, v)}
                          min={0}
                          precision={3}
                        />
                      </TableCell>
                    </TableRow>
                  ))}
                </TableBody>
              </Table>
            </div>
          )}
        </>
      ) : (
        <div className="flex flex-col gap-3">
          <div className="max-w-sm">
            <FieldLabel>Vendor *</FieldLabel>
            <PartySelector partyType="VENDOR" selectedPartyId={vendor?.id ?? null} onSelectParty={pickVendor} allowWalkIn={false} />
          </div>
          <div>
            <FieldLabel>Add item to return</FieldLabel>
            <ItemSearchTypeahead physicalOnly onSelectItem={addItem} placeholder="Search item to return..." />
          </div>
          {lines.map((l) => {
            const chosen = l.lots?.find((x) => String(x.purchaseItemId) === l.lot) ?? null;
            return (
            <div key={l.key} className="grid grid-cols-[1fr_2fr_7rem_auto] gap-2 items-end text-xs">
              <span className="font-medium pb-2">{l.name}</span>
              {!vendor ? (
                <span className="pb-2 text-slate-500">Choose the vendor to see its lots</span>
              ) : l.lots === null ? (
                <span className="pb-2 text-slate-500">Loading lots...</span>
              ) : l.lots.length === 0 ? (
                <span data-testid="no-vendor-lots" className="pb-2 text-slate-500">
                  No stock of this item from {vendor.name} to return
                </span>
              ) : (
                <Select
                  label="Lot"
                  aria-label={`Lot for ${l.name}`}
                  value={l.lot}
                  onChange={(v) => patchLine(l.key, { lot: String(v) })}
                  options={[
                    ...(l.lots.length > 1 ? [{ value: '', label: 'Choose a lot...' }] : []),
                    ...l.lots.map((lot) => ({ value: String(lot.purchaseItemId), label: lotLabel(lot) })),
                  ]}
                />
              )}
              <NumericInput
                label={chosen ? `Qty (max ${chosen.remainingQuantity})` : 'Qty'}
                aria-label={`Return quantity for ${l.name}`}
                value={l.qty}
                onChange={(v) => patchLine(l.key, { qty: v })}
                min={0}
                precision={3}
              />
              <button
                type="button"
                aria-label={`Remove ${l.name}`}
                onClick={() => setLines((ls) => ls.filter((x) => x.key !== l.key))}
                className="p-1 mb-1 text-slate-400 hover:text-rose-600 cursor-pointer"
              >
                <Trash2 className="h-3.5 w-3.5" />
              </button>
            </div>
            );
          })}
        </div>
      )}

      <div className="grid grid-cols-3 gap-3">
        <div className="flex flex-col gap-1">
          <FieldLabel htmlFor="vret-date">Return Date</FieldLabel>
          <input id="vret-date" type="date" value={date} max={todayLocal()} onChange={(e) => setDate(e.target.value)} className={inputCls} />
        </div>
        <div className="flex flex-col gap-1">
          <FieldLabel htmlFor="vret-reason">Reason</FieldLabel>
          <input id="vret-reason" type="text" value={reason} onChange={(e) => setReason(e.target.value)} className={inputCls} />
        </div>
        <div className="flex flex-col gap-1">
          <FieldLabel htmlFor="vret-notes">Notes</FieldLabel>
          <input id="vret-notes" type="text" value={notes} onChange={(e) => setNotes(e.target.value)} className={inputCls} />
        </div>
      </div>

      <RefundPreview
        result={preview.data}
        errors={preview.errors}
        label="Preview (reduces payable first, rest is cash from vendor)"
      />

      {error && (
        <p role="alert" className="text-xs text-rose-600 dark:text-rose-400 font-medium">
          {error}
        </p>
      )}
      <div>
        <Button onClick={submit} isLoading={busy}>
          Record Vendor Return
        </Button>
      </div>
    </div>
  );
};

export default ReturnFormVendor;
