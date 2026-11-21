import React from 'react';
import { Trash2, Plus, Minus, Package } from 'lucide-react';
import { cn } from '@/lib/utils';
import type { CartItem } from '@/types/store';
import { CurrencyDisplay } from '../common/CurrencyDisplay';
import { Table, TableHeader, TableBody, TableRow, TableHead, TableCell } from '../ui/Table';

export interface CartTableProps {
  items: CartItem[];
  onUpdateQuantity?: (itemId: number, qty: number) => void;
  onRemoveItem?: (itemId: number) => void;
  readOnly?: boolean;
  className?: string;
}

export const CartTable: React.FC<CartTableProps> = ({
  items = [],
  onUpdateQuantity,
  onRemoveItem,
  readOnly = false,
  className,
}) => {
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

  return (
    <div className={cn('rounded-lg border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 overflow-hidden shadow-xs', className)}>
      <Table dense>
        <TableHeader>
          <TableRow>
            <TableHead className="w-10 text-center">#</TableHead>
            <TableHead>Item Name</TableHead>
            <TableHead>Packaging</TableHead>
            <TableHead className="text-center w-36">Quantity</TableHead>
            <TableHead align="right">MRP</TableHead>
            <TableHead align="right">Rate</TableHead>
            <TableHead align="right">Line Total</TableHead>
            {!readOnly && <TableHead className="w-12 text-center">Action</TableHead>}
          </TableRow>
        </TableHeader>
        <TableBody>
          {items.map((item, index) => {
            const isBundle = item.bundleName && item.unitsPerBundle > 1;
            const lineTotal = item.lineTotal ?? item.quantity * item.rate;

            return (
              <TableRow key={`${item.itemId}`}>
                <TableCell className="text-center text-slate-400 select-none">
                  {index + 1}
                </TableCell>
                <TableCell>
                  <div className="flex flex-col">
                    <span className="font-medium text-slate-900 dark:text-slate-100">
                      {item.name}
                    </span>
                    {isBundle && (
                      <span className="text-[10px] text-slate-500 dark:text-slate-400 font-mono">
                        {item.quantity} {item.bundleName} × {item.unitsPerBundle} = {item.baseQuantity} {item.baseUnit}
                      </span>
                    )}
                  </div>
                </TableCell>
                <TableCell>
                  <span className="inline-flex items-center px-1.5 py-0.5 rounded text-[10px] font-medium bg-slate-100 dark:bg-slate-800 text-slate-700 dark:text-slate-300">
                    {item.bundleName || item.baseUnit}
                  </span>
                </TableCell>
                <TableCell>
                  {readOnly ? (
                    <span className="block text-center font-mono font-medium">
                      {item.quantity} {item.bundleName || item.baseUnit}
                    </span>
                  ) : (
                    <div className="flex items-center justify-center gap-1.5">
                      <button
                        type="button"
                        onClick={() => onUpdateQuantity?.(item.itemId, Math.max(1, item.quantity - 1))}
                        disabled={item.quantity <= 1}
                        className="p-1 rounded bg-slate-100 dark:bg-slate-800 hover:bg-slate-200 dark:hover:bg-slate-700 text-slate-600 dark:text-slate-300 disabled:opacity-40 cursor-pointer"
                      >
                        <Minus className="h-3 w-3" />
                      </button>
                      <input
                        type="number"
                        min={1}
                        value={item.quantity}
                        onChange={(e) => {
                          const val = parseInt(e.target.value, 10);
                          if (!isNaN(val) && val >= 1) {
                            onUpdateQuantity?.(item.itemId, val);
                          }
                        }}
                        className="w-12 text-center text-xs font-mono font-bold rounded border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-900 text-slate-900 dark:text-slate-100 py-0.5 focus:outline-none focus:ring-1 focus:ring-blue-500"
                      />
                      <button
                        type="button"
                        onClick={() => onUpdateQuantity?.(item.itemId, item.quantity + 1)}
                        className="p-1 rounded bg-slate-100 dark:bg-slate-800 hover:bg-slate-200 dark:hover:bg-slate-700 text-slate-600 dark:text-slate-300 cursor-pointer"
                      >
                        <Plus className="h-3 w-3" />
                      </button>
                    </div>
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
                      onClick={() => onRemoveItem?.(item.itemId)}
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
