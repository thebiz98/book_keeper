import React from 'react';
import { cn } from '@/lib/utils';

export interface QuantityInputProps {
  quantity: number;
  onChange: (qty: number) => void;
  baseUnit: string;
  unitsPerBundle?: number;
  bundleName?: string;
  maxStock?: number;
  className?: string;
  disabled?: boolean;
  min?: number;
}

export const QuantityInput: React.FC<QuantityInputProps> = ({
  quantity,
  onChange,
  baseUnit,
  unitsPerBundle = 1,
  bundleName,
  maxStock,
  className,
  disabled = false,
  min = 1,
}) => {
  const isBundle = unitsPerBundle > 1 && bundleName;
  const totalBaseUnits = quantity * unitsPerBundle;
  const isExceedingStock = maxStock !== undefined && totalBaseUnits > maxStock;

  const handleChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const val = parseInt(e.target.value, 10);
    if (isNaN(val)) {
      onChange(min);
      return;
    }
    const clamped = Math.max(min, val);
    onChange(clamped);
  };

  return (
    <div className={cn('flex flex-col gap-0.5', className)}>
      <div className="relative flex items-center">
        <input
          type="number"
          min={min}
          value={quantity}
          onChange={handleChange}
          disabled={disabled}
          className={cn(
            'w-full rounded-md border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-900 text-slate-900 dark:text-slate-100 text-sm px-3 py-1.5 pr-14 tabular-nums font-mono text-right focus:outline-none focus:ring-2 focus:ring-blue-500',
            isExceedingStock && 'border-rose-500 focus:ring-rose-500',
            disabled && 'opacity-50 cursor-not-allowed bg-slate-50 dark:bg-slate-800'
          )}
        />
        <span className="absolute right-2.5 text-xs font-medium text-slate-400 dark:text-slate-500 pointer-events-none select-none uppercase">
          {bundleName || baseUnit}
        </span>
      </div>

      {isBundle && (
        <span className="text-[10px] text-slate-500 dark:text-slate-400 tabular-nums font-mono">
          {quantity} {bundleName} × {unitsPerBundle} = {totalBaseUnits} {baseUnit}
        </span>
      )}

      {isExceedingStock && (
        <span role="alert" className="text-[10px] text-rose-600 dark:text-rose-400 font-medium">
          Exceeds available stock ({maxStock} {baseUnit})
        </span>
      )}
    </div>
  );
};

export default QuantityInput;
