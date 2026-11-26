import React, { useEffect, useState } from 'react';
import { api } from '@/api';
import { errorMessage } from '@/api/errors';
import type { ItemLot, LotSource } from '@/types';
import { CurrencyDisplay } from '../common/CurrencyDisplay';
import { STOCK_EPS } from '../common/StockStatusBadge';
import { Pencil } from 'lucide-react';
import { LotPriceEditModal } from './LotPriceEditModal';
import { OpeningQtyEditModal } from './OpeningQtyEditModal';
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '../ui/Table';

const SOURCE_LABEL: Record<LotSource, string> = {
  PURCHASE: 'Purchase',
  ADJUSTMENT: 'Adjustment',
  OPENING: 'Opening',
  RETURN: 'Return',
};

const SOURCE_STYLE: Record<LotSource, string> = {
  PURCHASE: 'bg-blue-50 text-blue-700 border-blue-200 dark:bg-blue-950/50 dark:text-blue-300 dark:border-blue-800',
  ADJUSTMENT: 'bg-amber-50 text-amber-700 border-amber-200 dark:bg-amber-950/50 dark:text-amber-300 dark:border-amber-800',
  OPENING: 'bg-slate-100 text-slate-700 border-slate-200 dark:bg-slate-800 dark:text-slate-300 dark:border-slate-700',
  RETURN: 'bg-violet-50 text-violet-700 border-violet-200 dark:bg-violet-950/50 dark:text-violet-300 dark:border-violet-800',
};

/** Active (remaining > 0) lots first in FIFO order, then exhausted lots, newest first. */
export function sortLots(lots: ItemLot[]): ItemLot[] {
  const active = (l: ItemLot) => l.remainingQuantity > STOCK_EPS;
  return [...lots].sort((a, b) => {
    if (active(a) !== active(b)) return active(a) ? -1 : 1;
    const byDate = a.transactionDate.localeCompare(b.transactionDate);
    if (byDate !== 0) return active(a) ? byDate : -byDate;
    return active(a) ? a.purchaseItemId - b.purchaseItemId : b.purchaseItemId - a.purchaseItemId;
  });
}

export interface LotInspectorProps {
  itemId: number;
  baseUnit: string;
  /** FR-25: show the Purchase Price edit button (hidden for RETURN lots). */
  editable?: boolean;
  /** Called after a lot price was saved (the lots reload too), so the caller can refresh its own views. */
  onChanged?: () => void;
  /** Reports the price modal opening / closing, so a host drawer can ignore its own close shortcuts. */
  onModalToggle?: (open: boolean) => void;
}

export const LotInspector: React.FC<LotInspectorProps> = ({ itemId, baseUnit, editable = false, onChanged, onModalToggle }) => {
  const [lots, setLots] = useState<ItemLot[] | null>(null);
  const [editing, setEditing] = useState<ItemLot | null>(null);
  const [editingQty, setEditingQty] = useState<ItemLot | null>(null);
  const [reloadKey, setReloadKey] = useState(0);
  const [error, setError] = useState<string | null>(null);

  const modalOpen = editing !== null || editingQty !== null;
  useEffect(() => {
    onModalToggle?.(modalOpen);
    return () => onModalToggle?.(false);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [modalOpen]);

  useEffect(() => {
    let cancelled = false;
    setLots(null);
    setError(null);
    api
      .getItemLots(itemId)
      .then((res) => {
        if (!cancelled) setLots(sortLots(res));
      })
      .catch((e) => {
        if (!cancelled) setError(errorMessage(e));
      });
    return () => {
      cancelled = true;
    };
  }, [itemId, reloadKey]);

  if (error) {
    return (
      <p role="alert" className="text-xs text-rose-600 dark:text-rose-400 px-2 py-1">
        Could not load lots: {error}
      </p>
    );
  }
  if (lots === null) return <p className="text-xs text-slate-400 px-2 py-1">Loading lots...</p>;
  if (lots.length === 0) return <p className="text-xs text-slate-400 px-2 py-1">No lots recorded.</p>;

  return (
    <>
    <Table dense scroll="none" data-testid="lot-table">
      <TableHeader>
        <TableRow className="text-left text-slate-400">
          <TableHead className="py-1 px-2 font-medium">Lot Date</TableHead>
          <TableHead className="py-1 px-2 font-medium">Code</TableHead>
          <TableHead className="py-1 px-2 font-medium">Source</TableHead>
          <TableHead className="py-1 px-2 font-medium">Vendor</TableHead>
          <TableHead className="py-1 px-2 font-medium text-right">Original Qty</TableHead>
          <TableHead className="py-1 px-2 font-medium text-right">Remaining</TableHead>
          <TableHead className="py-1 px-2 font-medium text-right">Purchase Price</TableHead>
          {editable && <TableHead className="py-1 px-2" />}
        </TableRow>
      </TableHeader>
      <TableBody>
        {lots.map((l) => (
          <TableRow
            key={l.purchaseItemId}
            data-testid="lot-row"
            className={`${
              l.remainingQuantity > STOCK_EPS ? '' : 'opacity-50'
            }`}
          >
            <TableCell className="py-1 px-2 font-mono">{l.transactionDate}</TableCell>
            <TableCell className="py-1 px-2 font-mono">{l.transactionCode}</TableCell>
            <TableCell className="py-1 px-2">
              <span
                className={`inline-flex px-1.5 py-0.5 rounded border text-[10px] font-semibold ${SOURCE_STYLE[l.source]}`}
              >
                {SOURCE_LABEL[l.source] ?? l.source}
              </span>
            </TableCell>
            <TableCell className="py-1 px-2">{l.vendorName || '—'}</TableCell>
            <TableCell className="py-1 px-2 text-right font-mono">
              {l.quantity} {baseUnit}
            </TableCell>
            <TableCell className="py-1 px-2 text-right font-mono font-semibold">
              {l.remainingQuantity} {baseUnit}
            </TableCell>
            <TableCell className="py-1 px-2 text-right">
              <CurrencyDisplay amount={l.unitCost} size="sm" />
            </TableCell>
            {editable && (
              <TableCell className="py-1 px-2 text-right">
                {l.source !== 'RETURN' && (
                  <button
                    type="button"
                    aria-label={`Edit purchase price of ${l.transactionCode}`}
                    className="p-1 rounded text-slate-500 hover:text-blue-600 hover:bg-slate-100 dark:hover:bg-slate-800"
                    onClick={() => setEditing(l)}
                  >
                    <Pencil size={12} />
                  </button>
                )}
                {l.source === 'OPENING' && (
                  <button
                    type="button"
                    aria-label={`Edit quantity of ${l.transactionCode}`}
                    className="ml-1 px-1.5 py-0.5 rounded text-[10px] font-medium text-slate-500 hover:text-blue-600 hover:bg-slate-100 dark:hover:bg-slate-800"
                    onClick={() => setEditingQty(l)}
                  >
                    Edit quantity
                  </button>
                )}
              </TableCell>
            )}
          </TableRow>
        ))}
      </TableBody>
    </Table>
    {editable && (
      <OpeningQtyEditModal
        lot={editingQty}
        baseUnit={baseUnit}
        onClose={() => setEditingQty(null)}
        onSaved={() => {
          setReloadKey((k) => k + 1);
          onChanged?.();
        }}
      />
    )}
    {editable && (
      <LotPriceEditModal
        lot={editing}
        baseUnit={baseUnit}
        onClose={() => setEditing(null)}
        onSaved={() => {
          setReloadKey((k) => k + 1);
          onChanged?.();
        }}
      />
    )}
    </>
  );
};

export default LotInspector;
