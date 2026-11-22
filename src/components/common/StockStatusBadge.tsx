import React from 'react';
import { cn } from '@/lib/utils';

export interface StockStatusBadgeProps {
  currentStock: number;
  /** Server-computed low-stock flag (is_low_stock). The only source of the Low Stock state. */
  isLowStock?: boolean;
  baseUnit?: string;
  className?: string;
}

/** Stock at or below this is treated as zero (fractional base units). */
export const STOCK_EPS = 1e-9;

export const StockStatusBadge: React.FC<StockStatusBadgeProps> = ({
  currentStock,
  isLowStock = false,
  baseUnit = 'PCS',
  className,
}) => {
  const isOutOfStock = currentStock <= STOCK_EPS;
  const isWarning = !isOutOfStock && isLowStock;

  let label: string;
  let colorStyles: string;

  if (isOutOfStock) {
    label = 'Out of Stock';
    colorStyles =
      'bg-rose-50 text-rose-700 border-rose-200 dark:bg-rose-950/60 dark:text-rose-300 dark:border-rose-800';
  } else if (isWarning) {
    label = `Low Stock: ${currentStock} ${baseUnit}`;
    colorStyles =
      'bg-amber-50 text-amber-700 border-amber-200 dark:bg-amber-950/60 dark:text-amber-300 dark:border-amber-800';
  } else {
    label = `${currentStock} ${baseUnit}`;
    colorStyles =
      'bg-emerald-50 text-emerald-700 border-emerald-200 dark:bg-emerald-950/60 dark:text-emerald-300 dark:border-emerald-800';
  }

  return (
    <span
      className={cn(
        'inline-flex items-center px-2 py-0.5 rounded text-xs font-medium border select-none tabular-nums',
        colorStyles,
        className
      )}
    >
      {label}
    </span>
  );
};

export default StockStatusBadge;
