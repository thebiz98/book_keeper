import React, { useEffect, useState } from 'react';
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
  /** Fractional quantities (e.g. 2.5 KG). Default true. */
  allowDecimals?: boolean;
  /** The caller shows the bundle conversion line itself (e.g. in a fixed helper slot). */
  hideConversion?: boolean;
  id?: string;
  'aria-label'?: string;
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
  allowDecimals = true,
  hideConversion = false,
  id,
  'aria-label': ariaLabel,
}) => {
  const isBundle = unitsPerBundle > 1 && bundleName;
  const totalBaseUnits = Math.round(quantity * unitsPerBundle * 1e6) / 1e6;
  const [text, setText] = useState(String(quantity));
  useEffect(() => {
    setText((prev) => (parseFloat(prev) === quantity ? prev : String(quantity)));
  }, [quantity]);
  const isExceedingStock = maxStock !== undefined && totalBaseUnits > maxStock;

  const handleChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const raw = e.target.value;
    setText(raw);
    const val = allowDecimals ? parseFloat(raw) : parseInt(raw, 10);
    if (isNaN(val)) {
      onChange(min);
      return;
    }
    onChange(Math.max(min, val));
  };

  return (
    <div className={cn('flex flex-col gap-0.5', className)}>
      <div className="relative flex items-center">
        <input
          id={id}
          aria-label={ariaLabel}
          type="number"
          min={min}
          step={allowDecimals ? 'any' : 1}
          value={text}
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

      {isBundle && !hideConversion && (
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
