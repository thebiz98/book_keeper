import React, { useCallback, useEffect, useState } from 'react';
import { Trash2 } from 'lucide-react';
import { api, errorMessage } from '@/api';
import { round2 } from '@/lib/money';
import { todayLocal } from '@/lib/dates';
import type {
  CustomerReturnItemPayload,
  ItemSearchResult,
  Party,
  ReturnResult,
  SaleDetailView,
  SettlementType,
  StockCondition,
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
  EPS,
  FieldLabel,
  RefundPreview,
  ReturnResultPanel,
  inputCls,
  usePartyBalance,
} from './ReturnFormShared';
import { refreshAppMetrics } from './navParams';

const CONDITIONS = [
  { value: 'RESTOCKABLE', label: 'Restockable' },
  { value: 'DAMAGED_SCRAP', label: 'Damaged / scrap' },
] as const;

interface BillLine {
  saleItemId: number;
  itemId: number;
  name: string;
  sold: number;
  returned: number;
  /** Sold rate net of the bill discount share (FR-19): the refund cap. */
  soldRate: number;
  /** Rate on the bill before the bill-level discount. */
  listRate: number;
  qty: number;
  rate: number;
  condition: StockCondition;
}

interface FreeLine {
  key: number;
  itemId: number;
  name: string;
  baseUnit: string;
  qty: number;
  rate: number;
  condition: StockCondition;
  /** null while loading / unknown. */
  hasLots: boolean | null;
  cost: number;
}

interface IssueLine {
  itemId: number;
  name: string;
  baseUnit: string;
  qty: number;
  price: number;
}

const returnable = (l: BillLine) => Math.max(0, Math.round((l.sold - l.returned) * 1e6) / 1e6);

export interface ReturnFormCustomerProps {
  /** Pre-linked bill (from Sales History [Return]). */
  initialBillId?: number | null;
  onDone?: () => void;
}

export const ReturnFormCustomer: React.FC<ReturnFormCustomerProps> = ({ initialBillId = null, onDone }) => {
  const [mode, setMode] = useState<'BILL' | 'FREE'>('BILL');
  const [detail, setDetail] = useState<SaleDetailView | null>(null);
  const [lines, setLines] = useState<BillLine[]>([]);
  const [freeLines, setFreeLines] = useState<FreeLine[]>([]);
  const [customer, setCustomer] = useState<Party | null>(null);
  const [settlement, setSettlement] = useState<SettlementType>('REFUND');
  const [issue, setIssue] = useState<IssueLine[]>([]);
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
    setLines([]);
    api
      .getSaleDetails(id)
      .then((d) => {
        setDetail(d);
        setLines(
          d.items.map((it) => ({
            saleItemId: it.saleItem.id,
            itemId: it.saleItem.itemId,
            name: it.itemName,
            sold: it.saleItem.quantity,
            returned: it.returnedQuantity,
            soldRate: it.netUnitPrice,
            listRate: it.saleItem.unitPrice,
            qty: 0,
            rate: it.netUnitPrice,
            condition: 'RESTOCKABLE',
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

  const patchLine = (i: number, p: Partial<BillLine>) =>
    setLines((ls) => ls.map((l, idx) => (idx === i ? { ...l, ...p } : l)));
  const patchFree = (key: number, p: Partial<FreeLine>) =>
    setFreeLines((ls) => ls.map((l) => (l.key === key ? { ...l, ...p } : l)));

  const addFreeItem = (it: ItemSearchResult) => {
    const key = Date.now() + Math.random();
    setFreeLines((ls) => [
      ...ls,
      {
        key,
        itemId: it.itemId,
        name: it.name,
        baseUnit: it.baseUnit,
        qty: 1,
        rate: it.defaultSalePrice,
        condition: 'RESTOCKABLE',
        hasLots: null,
        cost: 0,
      },
    ]);
    api
      .getItemLots(it.itemId)
      .then((lots) => patchFree(key, { hasLots: lots.length > 0 }))
      .catch(() => patchFree(key, { hasLots: null }));
  };

  const addIssueItem = (it: ItemSearchResult) =>
    setIssue((xs) =>
      xs.some((x) => x.itemId === it.itemId)
        ? xs.map((x) => (x.itemId === it.itemId ? { ...x, qty: x.qty + 1 } : x))
        : [...xs, { itemId: it.itemId, name: it.name, baseUnit: it.baseUnit, qty: 1, price: it.defaultSalePrice }]
    );

  const returnedValue =
    mode === 'BILL'
      ? round2(lines.reduce((s, l) => s + (l.qty > 0 ? l.qty * l.rate : 0), 0))
      : round2(freeLines.reduce((s, l) => s + l.qty * l.rate, 0));
  const issuedValue = round2(issue.reduce((s, x) => s + x.qty * x.price, 0));

  const freeBalance = usePartyBalance('CUSTOMER', mode === 'FREE' ? customer?.id ?? null : null);
  const balance = mode === 'BILL' ? detail?.sale.balanceDue ?? 0 : customer ? freeBalance : 0;

  const submit = async () => {
    setError(null);
    let items: CustomerReturnItemPayload[] = [];
    if (mode === 'BILL') {
      if (!detail) return setError('Select a bill first');
      for (const l of lines) {
        if (l.qty <= 0) continue;
        if (l.qty > returnable(l) + EPS) {
          return setError(`${l.name}: quantity ${l.qty} exceeds returnable ${returnable(l)}`);
        }
        if (l.rate > l.soldRate + EPS) {
          return setError(`${l.name}: rate ${l.rate} exceeds the sold rate ${l.soldRate}`);
        }
        if (l.rate < 0) return setError(`${l.name}: rate cannot be negative`);
        items.push({
          itemId: l.itemId,
          saleItemId: l.saleItemId,
          quantity: l.qty,
          unitPrice: l.rate,
          condition: l.condition,
          unitCostOverride: null,
        });
      }
    } else {
      for (const l of freeLines) {
        if (!(l.qty > 0)) return setError(`${l.name}: quantity must be greater than 0`);
        if (l.hasLots === false && !(l.cost > 0)) {
          return setError(`${l.name}: unit cost is required (item has no stock history)`);
        }
        items.push({
          itemId: l.itemId,
          saleItemId: null,
          quantity: l.qty,
          unitPrice: l.rate,
          condition: l.condition,
          unitCostOverride: l.hasLots === true ? null : l.cost > 0 ? l.cost : null,
        });
      }
    }
    if (items.length === 0) return setError('Enter a return quantity for at least one line');
    if (settlement === 'REPLACEMENT') {
      if (mode !== 'BILL') return setError('Replacements need the original bill');
      if (issue.length === 0) return setError('Add at least one item to issue as a replacement');
      if (issue.some((x) => !(x.qty > 0))) return setError('Replacement quantities must be greater than 0');
    }
    setBusy(true);
    try {
      const res = await api.createCustomerReturn({
        originalSaleId: mode === 'BILL' ? detail?.sale.id ?? null : null,
        partyId: mode === 'FREE' ? customer?.id ?? null : null,
        returnDate: date,
        settlementType: settlement,
        reason: reason.trim() || null,
        notes: notes.trim() || null,
        items,
        replacementItems:
          settlement === 'REPLACEMENT' ? issue.map((x) => ({ itemId: x.itemId, quantity: x.qty })) : [],
      });
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
    setLines([]);
    setFreeLines([]);
    setIssue([]);
    setSettlement('REFUND');
    setNotes('');
    setReason('');
    setError(null);
  };

  if (result) return <ReturnResultPanel result={result} onAnother={reset} />;

  return (
    <div className="flex flex-col gap-4">
      <div className="flex gap-4 text-xs">
        {(['BILL', 'FREE'] as const).map((m) => (
          <label key={m} className="flex items-center gap-1.5 cursor-pointer">
            <input
              type="radio"
              name="return-mode"
              checked={mode === m}
              onChange={() => {
                setMode(m);
                if (m === 'FREE') setSettlement('REFUND');
              }}
            />
            {m === 'BILL' ? 'With bill' : 'No receipt'}
          </label>
        ))}
      </div>

      {mode === 'BILL' ? (
        <>
          {!detail && <BillPicker kind="SALE" onPick={loadBill} />}
          {loadError && (
            <p role="alert" className="text-xs text-rose-600 dark:text-rose-400">
              {loadError}
            </p>
          )}
          {detail && (
            <div className="flex flex-col gap-2">
              <div className="flex items-center justify-between text-xs bg-slate-50 dark:bg-slate-800 p-2.5 rounded-md">
                <span>
                  <span className="font-mono font-semibold">{detail.sale.transactionCode}</span> · {detail.sale.transactionDate} ·{' '}
                  {detail.sale.partyName ?? 'Walk-in'} · balance due{' '}
                  <CurrencyDisplay amount={detail.sale.balanceDue} size="sm" />
                </span>
                <Button size="xs" variant="ghost" onClick={() => { setDetail(null); setLines([]); }}>
                  Change bill
                </Button>
              </div>
              <Table dense scroll="bounded">
                <TableHeader>
                  <TableRow className="text-left text-slate-400">
                    <TableHead className="py-1">Item</TableHead>
                    <TableHead className="text-right">Sold</TableHead>
                    <TableHead className="text-right">Returned</TableHead>
                    <TableHead className="text-right">Returnable</TableHead>
                    <TableHead className="text-right">Sold rate</TableHead>
                    <TableHead className="w-24">Return qty</TableHead>
                    <TableHead className="w-24">Rate</TableHead>
                    <TableHead className="w-36">Condition</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {lines.map((l, i) => (
                    <TableRow key={l.saleItemId} >
                      <TableCell className="py-1 font-medium">{l.name}</TableCell>
                      <TableCell className="text-right font-mono">{l.sold}</TableCell>
                      <TableCell className="text-right font-mono">{l.returned}</TableCell>
                      <TableCell className="text-right font-mono">{returnable(l)}</TableCell>
                      <TableCell className="text-right">
                        <CurrencyDisplay amount={l.soldRate} size="sm" />
                        {l.soldRate < l.listRate - 0.005 && (
                          <div className="text-[10px] text-slate-400" data-testid="net-rate-hint">
                            net of bill discount (sold ₹{l.listRate})
                          </div>
                        )}
                      </TableCell>
                      <TableCell>
                        <NumericInput
                          aria-label={`Return quantity for ${l.name}`}
                          value={l.qty}
                          onChange={(v) => patchLine(i, { qty: v })}
                          min={0}
                          precision={3}
                        />
                      </TableCell>
                      <TableCell>
                        <NumericInput
                          aria-label={`Return rate for ${l.name}`}
                          value={l.rate}
                          onChange={(v) => patchLine(i, { rate: v })}
                          min={0}
                        />
                      </TableCell>
                      <TableCell>
                        <Select
                          aria-label={`Condition for ${l.name}`}
                          value={l.condition}
                          onChange={(v) => patchLine(i, { condition: v as StockCondition })}
                          options={[...CONDITIONS]}
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
            <FieldLabel>Customer (optional)</FieldLabel>
            <PartySelector
              partyType="CUSTOMER"
              selectedPartyId={customer?.id ?? null}
              onSelectParty={setCustomer}
            />
          </div>
          <div>
            <FieldLabel>Add returned item</FieldLabel>
            <ItemSearchTypeahead onSelectItem={addFreeItem} placeholder="Search item to return..." />
          </div>
          {freeLines.length > 0 && (
            <Table dense scroll="bounded">
              <TableHeader>
                <TableRow className="text-left text-slate-400">
                  <TableHead className="py-1">Item</TableHead>
                  <TableHead className="w-24">Qty</TableHead>
                  <TableHead className="w-24">Rate</TableHead>
                  <TableHead className="w-36">Condition</TableHead>
                  <TableHead className="w-28">Unit cost</TableHead>
                  <TableHead />
                </TableRow>
              </TableHeader>
              <TableBody>
                {freeLines.map((l) => (
                  <TableRow key={l.key} >
                    <TableCell className="py-1 font-medium">{l.name}</TableCell>
                    <TableCell>
                      <NumericInput
                        aria-label={`Return quantity for ${l.name}`}
                        value={l.qty}
                        onChange={(v) => patchFree(l.key, { qty: v })}
                        min={0}
                        precision={3}
                      />
                    </TableCell>
                    <TableCell>
                      <NumericInput
                        aria-label={`Return rate for ${l.name}`}
                        value={l.rate}
                        onChange={(v) => patchFree(l.key, { rate: v })}
                        min={0}
                      />
                    </TableCell>
                    <TableCell>
                      <Select
                        aria-label={`Condition for ${l.name}`}
                        value={l.condition}
                        onChange={(v) => patchFree(l.key, { condition: v as StockCondition })}
                        options={[...CONDITIONS]}
                      />
                    </TableCell>
                    <TableCell>
                      {l.hasLots === true ? (
                        <span className="text-slate-400">from lots</span>
                      ) : (
                        <NumericInput
                          aria-label={`Unit cost for ${l.name}`}
                          value={l.cost}
                          onChange={(v) => patchFree(l.key, { cost: v })}
                          min={0}
                        />
                      )}
                    </TableCell>
                    <TableCell>
                      <button
                        type="button"
                        aria-label={`Remove ${l.name}`}
                        onClick={() => setFreeLines((ls) => ls.filter((x) => x.key !== l.key))}
                        className="p-1 text-slate-400 hover:text-rose-600 cursor-pointer"
                      >
                        <Trash2 className="h-3.5 w-3.5" />
                      </button>
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          )}
        </div>
      )}

      <div className="grid grid-cols-3 gap-3">
        <div className="flex flex-col gap-1">
          <FieldLabel htmlFor="ret-date">Return Date</FieldLabel>
          <input id="ret-date" type="date" value={date} max={todayLocal()} onChange={(e) => setDate(e.target.value)} className={inputCls} />
        </div>
        <div className="flex flex-col gap-1">
          <FieldLabel htmlFor="ret-reason">Reason</FieldLabel>
          <input id="ret-reason" type="text" value={reason} onChange={(e) => setReason(e.target.value)} className={inputCls} />
        </div>
        <div className="flex flex-col gap-1">
          <FieldLabel htmlFor="ret-notes">Notes</FieldLabel>
          <input id="ret-notes" type="text" value={notes} onChange={(e) => setNotes(e.target.value)} className={inputCls} />
        </div>
      </div>

      <div className="flex gap-4 text-xs items-center">
        <span className="font-medium">Settlement:</span>
        <label className="flex items-center gap-1.5 cursor-pointer">
          <input type="radio" name="settlement" checked={settlement === 'REFUND'} onChange={() => setSettlement('REFUND')} />
          Refund
        </label>
        <label className={`flex items-center gap-1.5 ${mode === 'BILL' ? 'cursor-pointer' : 'opacity-40'}`}>
          <input
            type="radio"
            name="settlement"
            disabled={mode !== 'BILL'}
            checked={settlement === 'REPLACEMENT'}
            onChange={() => setSettlement('REPLACEMENT')}
          />
          Replacement / exchange (with bill only)
        </label>
      </div>

      {settlement === 'REFUND' ? (
        <RefundPreview value={returnedValue} balance={balance} />
      ) : (
        <div className="flex flex-col gap-2" data-testid="replacement-section">
          <FieldLabel>Items to issue</FieldLabel>
          <ItemSearchTypeahead onSelectItem={addIssueItem} placeholder="Search item to issue..." />
          {issue.map((x) => (
            <div key={x.itemId} className="flex items-center gap-2 text-xs">
              <span className="flex-1 font-medium">{x.name}</span>
              <div className="w-24">
                <NumericInput
                  aria-label={`Issue quantity for ${x.name}`}
                  value={x.qty}
                  onChange={(v) => setIssue((xs) => xs.map((y) => (y.itemId === x.itemId ? { ...y, qty: v } : y)))}
                  min={0}
                  precision={3}
                />
              </div>
              <span>{x.baseUnit}</span>
              <button
                type="button"
                aria-label={`Remove ${x.name}`}
                onClick={() => setIssue((xs) => xs.filter((y) => y.itemId !== x.itemId))}
                className="p-1 text-slate-400 hover:text-rose-600 cursor-pointer"
              >
                <Trash2 className="h-3.5 w-3.5" />
              </button>
            </div>
          ))}
          {issue.length > 0 && Math.abs(issuedValue - returnedValue) > 0.005 && (
            <p data-testid="value-difference" className="text-xs text-amber-700 dark:text-amber-400">
              Value difference: returned <CurrencyDisplay amount={returnedValue} size="sm" /> vs issued{' '}
              <CurrencyDisplay amount={issuedValue} size="sm" />. No money moves in a replacement; do a Refund plus a new
              sale if the difference must be paid.
            </p>
          )}
        </div>
      )}

      {error && (
        <p role="alert" className="text-xs text-rose-600 dark:text-rose-400 font-medium">
          {error}
        </p>
      )}
      <div>
        <Button onClick={submit} isLoading={busy}>
          Record Return
        </Button>
      </div>
    </div>
  );
};

export default ReturnFormCustomer;
