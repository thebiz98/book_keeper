import React, { useEffect, useRef, useState } from 'react';
import { Trash2, Plus, Minus, Package } from 'lucide-react';
import { cn } from '@/lib/utils';
import type { CartLine } from '@/store/useCartStore';
import { CurrencyDisplay } from '../common/CurrencyDisplay';
import { Table, TableHeader, TableBody, TableRow, TableHead, TableCell } from '../ui/Table';

export interface CartTableProps {
  items: CartLine[];
  /** Lines are addressed by `lineId` (an item may appear on several lines). */
  onUpdateQuantity?: (lineId: string, qty: number) => boolean | void;
  onRemoveItem?: (lineId: string) => void;
  /** Called when focus should return to the search box (e.g. last row removed). */
  onFocusSearch?: () => void;
  readOnly?: boolean;
  className?: string;
}

const NUMBER_RE = /^\d*\.?\d*$/;
const fmtQty = (n: number) => String(Number(n.toFixed(3)));

/** `2 × Box of 10 = 20 PCS` for pack lines, `3 PCS` for loose lines. */
export function packDisplay(item: CartLine): string {
  const isPack = item.bundleId != null || (item.bundleName != null && item.unitsPerBundle > 1);
  if (!isPack) return `${fmtQty(item.quantity)} ${item.baseUnit}`;
  return `${fmtQty(item.quantity)} × ${item.bundleName ?? 'Pack'} = ${fmtQty(item.baseQuantity)} ${item.baseUnit}`;
}

const QtyCell: React.FC<{
  line: CartLine;
  onUpdate?: (lineId: string, qty: number) => boolean | void;
}> = ({ line, onUpdate }) => {
  const [draft, setDraft] = useState<string | null>(null);
  const step = (delta: number) => {
    const next = line.quantity + delta;
    if (next > 0) onUpdate?.(line.lineId, next);
  };
  return (
    <div className="flex items-center justify-center gap-1.5">
      <button
        type="button"
        aria-label={`Decrease quantity of ${line.name}`}
        onClick={() => step(-1)}
        disabled={line.quantity <= 1}
        className="p-1 rounded bg-slate-100 dark:bg-slate-800 hover:bg-slate-200 dark:hover:bg-slate-700 text-slate-600 dark:text-slate-300 disabled:opacity-40 cursor-pointer"
      >
        <Minus className="h-3 w-3" />
      </button>
      <input
        type="text"
        inputMode="decimal"
        aria-label={`Quantity of ${line.name}`}
        value={draft ?? fmtQty(line.quantity)}
        onChange={(e) => {
          const raw = e.target.value;
          if (!NUMBER_RE.test(raw)) return;
          setDraft(raw);
          const val = parseFloat(raw);
          if (Number.isFinite(val) && val > 0) onUpdate?.(line.lineId, val);
        }}
        onFocus={(e) => e.currentTarget.select()}
        onBlur={() => setDraft(null)}
        onKeyDown={(e) => {
          if (e.key === 'Enter') {
            e.preventDefault();
            e.currentTarget.blur();
          }
        }}
        className="w-14 text-center text-xs font-mono font-bold rounded border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-900 text-slate-900 dark:text-slate-100 py-0.5 focus:outline-none focus:ring-1 focus:ring-blue-500"
      />
      <button
        type="button"
        aria-label={`Increase quantity of ${line.name}`}
        onClick={() => step(1)}
        className="p-1 rounded bg-slate-100 dark:bg-slate-800 hover:bg-slate-200 dark:hover:bg-slate-700 text-slate-600 dark:text-slate-300 cursor-pointer"
      >
        <Plus className="h-3 w-3" />
      </button>
    </div>
  );
};

export const CartTable: React.FC<CartTableProps> = ({
  items = [],
  onUpdateQuantity,
  onRemoveItem,
  onFocusSearch,
  readOnly = false,
  className,
}) => {
  const bodyRef = useRef<HTMLDivElement>(null);

  // Lines are appended at the end: keep the newest one visible in the bounded table.
  const lineCount = items.length;
  const prevCount = useRef(lineCount);
  useEffect(() => {
    if (lineCount > prevCount.current) {
      const el = bodyRef.current?.querySelector<HTMLElement>('[data-table-scroll]');
      if (el) el.scrollTop = el.scrollHeight;
    }
    prevCount.current = lineCount;
  }, [lineCount]);

  if (items.length === 0) {
    return (
      <div
        className={cn(
          'flex flex-col items-center justify-center p-8 rounded-lg border border-dashed border-slate-300 dark:border-slate-800 text-slate-400 dark:text-slate-500 bg-white dark:bg-slate-900',
          className
        )}
      >
        <Package className="h-10 w-10 mb-2 stroke-1" />
        <p className="text-sm font-medium">Cart is empty</p>
        <p className="text-xs">Scan barcode or search item (F4) to add line items</p>
      </div>
    );
  }

  const rows = () => Array.from(bodyRef.current?.querySelectorAll<HTMLElement>('tr[data-line-id]') ?? []);

  const onRowKeyDown = (e: React.KeyboardEvent<HTMLTableRowElement>, line: CartLine, index: number) => {
    // Only handle keys aimed at the row itself, not at the quantity input inside it.
    if (e.target !== e.currentTarget || readOnly) return;
    const list = rows();
    if (e.key === 'Delete' || e.key === 'Backspace') {
      e.preventDefault();
      onRemoveItem?.(line.lineId);
      const next = list[index + 1] ?? list[index - 1];
      if (next) next.focus();
      else onFocusSearch?.();
    } else if (e.key === 'ArrowDown') {
      e.preventDefault();
      list[Math.min(index + 1, list.length - 1)]?.focus();
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      if (index === 0) onFocusSearch?.();
      else list[index - 1]?.focus();
    } else if (e.key === '+' || e.key === '=') {
      e.preventDefault();
      onUpdateQuantity?.(line.lineId, line.quantity + 1);
    } else if (e.key === '-' && line.quantity > 1) {
      e.preventDefault();
      onUpdateQuantity?.(line.lineId, line.quantity - 1);
    } else if (e.key === 'Enter') {
      e.preventDefault();
      e.currentTarget.querySelector<HTMLInputElement>('input')?.focus();
    }
  };

  return (
    <div
      ref={bodyRef}
      className={cn('rounded-lg border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 overflow-hidden shadow-xs', className)}
    >
      <Table dense>
        <TableHeader>
          <TableRow>
            <TableHead className="w-10 text-center">#</TableHead>
            <TableHead>Item Name</TableHead>
            <TableHead>Packaging</TableHead>
            <TableHead className="text-center w-40">Quantity</TableHead>
            <TableHead align="right">MRP</TableHead>
            <TableHead align="right">Rate</TableHead>
            <TableHead align="right">Line Total</TableHead>
            {!readOnly && <TableHead className="w-12 text-center">Action</TableHead>}
          </TableRow>
        </TableHeader>
        <TableBody>
          {items.map((item, index) => {
            const lineTotal = item.lineTotal ?? item.quantity * item.rate;
            return (
              <TableRow
                key={item.lineId}
                data-line-id={item.lineId}
                tabIndex={readOnly ? undefined : 0}
                aria-label={`${item.name}, ${packDisplay(item)}`}
                onKeyDown={(e) => onRowKeyDown(e, item, index)}
                className="focus:outline-none focus:bg-blue-50 dark:focus:bg-blue-950/40 focus:ring-1 focus:ring-inset focus:ring-blue-500"
              >
                <TableCell className="text-center text-slate-400 select-none">{index + 1}</TableCell>
                <TableCell>
                  <div className="flex flex-col">
                    <span className="font-medium text-slate-900 dark:text-slate-100">{item.name}</span>
                    <span data-testid="pack-display" className="text-[10px] text-slate-500 dark:text-slate-400 font-mono">
                      {packDisplay(item)}
                    </span>
                  </div>
                </TableCell>
                <TableCell>
                  <span className="inline-flex items-center px-1.5 py-0.5 rounded text-[10px] font-medium bg-slate-100 dark:bg-slate-800 text-slate-700 dark:text-slate-300">
                    {item.bundleName || item.baseUnit}
                  </span>
                </TableCell>
                <TableCell>
                  {readOnly ? (
                    <span className="block text-center font-mono font-medium">{packDisplay(item)}</span>
                  ) : (
                    <QtyCell line={item} onUpdate={onUpdateQuantity} />
                  )}
                </TableCell>
                <TableCell align="right" mono className="text-slate-500 text-xs">
                  {item.mrp ? <CurrencyDisplay amount={item.mrp} size="sm" /> : '—'}
                </TableCell>
                <TableCell align="right" mono>
                  <CurrencyDisplay amount={item.rate} size="sm" />
                </TableCell>
                <TableCell align="right" mono className="font-semibold text-slate-900 dark:text-slate-100">
                  <CurrencyDisplay amount={lineTotal} size="sm" />
                </TableCell>
                {!readOnly && (
                  <TableCell className="text-center">
                    <button
                      type="button"
                      title="Remove Item (Del)"
                      aria-label={`Remove ${item.name}`}
                      onClick={() => onRemoveItem?.(item.lineId)}
                      className="p-1 rounded text-slate-400 hover:text-rose-600 dark:hover:text-rose-400 transition-colors cursor-pointer"
                    >
                      <Trash2 className="h-3.5 w-3.5" />
                    </button>
                  </TableCell>
                )}
              </TableRow>
            );
          })}
        </TableBody>
      </Table>
    </div>
  );
};

export default CartTable;
