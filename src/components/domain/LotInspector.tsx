import React, { useEffect, useState } from 'react';
import { api } from '@/api';
import { errorMessage } from '@/api/errors';
import type { ItemLot, LotSource } from '@/types';
import { CurrencyDisplay } from '../common/CurrencyDisplay';
import { STOCK_EPS } from '../common/StockStatusBadge';
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
}

export const LotInspector: React.FC<LotInspectorProps> = ({ itemId, baseUnit }) => {
  const [lots, setLots] = useState<ItemLot[] | null>(null);
  const [error, setError] = useState<string | null>(null);

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
  }, [itemId]);

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
          </TableRow>
        ))}
      </TableBody>
    </Table>
  );
};

export default LotInspector;
