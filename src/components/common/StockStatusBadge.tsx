import React from 'react';
import { cn } from '@/lib/utils';

export interface StockStatusBadgeProps {
  currentStock: number;
  /** Server-computed low-stock flag (is_low_stock). The only source of the Low Stock state. */
  isLowStock?: boolean;
  baseUnit?: string;
  /** Per-item level (server flag still decides the state); used for the tooltip and aria text. */
  lowStockThreshold?: number;
  className?: string;
}

/** Badge colour tokens, shared with the service availability badge. */
export const STOCK_TONE = {
  ok: 'bg-emerald-50 text-emerald-700 border-emerald-200 dark:bg-emerald-950/60 dark:text-emerald-300 dark:border-emerald-800',
  low: 'bg-amber-50 text-amber-700 border-amber-200 dark:bg-amber-950/60 dark:text-amber-300 dark:border-amber-800',
  out: 'bg-rose-50 text-rose-700 border-rose-200 dark:bg-rose-950/60 dark:text-rose-300 dark:border-rose-800',
  neutral:
    'bg-slate-50 text-slate-500 border-slate-200 dark:bg-slate-900/60 dark:text-slate-400 dark:border-slate-700',
} as const;

/** Base classes of every stock-style badge. */
export const STOCK_BADGE_BASE =
  'inline-flex items-center px-2 py-0.5 rounded text-xs font-medium border select-none tabular-nums';

/** Stock at or below this is treated as zero (fractional base units). */
export const STOCK_EPS = 1e-9;

export const StockStatusBadge: React.FC<StockStatusBadgeProps> = ({
  currentStock,
  isLowStock = false,
  baseUnit = 'PCS',
  lowStockThreshold,
  className,
}) => {
  const isOutOfStock = currentStock <= STOCK_EPS;
  const isWarning = !isOutOfStock && isLowStock;

  let label: string;
  let colorStyles: string;

  if (isOutOfStock) {
    label = 'Out of Stock';
    colorStyles = STOCK_TONE.out;
  } else if (isWarning) {
    label = `Low Stock: ${currentStock} ${baseUnit}`;
    colorStyles = STOCK_TONE.low;
  } else {
    label = `${currentStock} ${baseUnit}`;
    colorStyles = STOCK_TONE.ok;
  }

  const hint =
    lowStockThreshold === undefined
      ? undefined
      : lowStockThreshold === 0
        ? 'Low-stock alert off'
        : `Low when below ${lowStockThreshold} ${baseUnit}`;

  return (
    <span
      title={hint}
      aria-label={hint ? `${label}. ${hint}` : undefined}
      className={cn(
        STOCK_BADGE_BASE,
        colorStyles,
        className
      )}
    >
      {label}
    </span>
  );
};

export default StockStatusBadge;
