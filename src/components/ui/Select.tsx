import React from 'react';
import { ChevronDown } from 'lucide-react';
import { cn } from '@/lib/utils';

export interface SelectOption<T = string | number> {
  value: T;
  label: string;
  disabled?: boolean;
}

export interface SelectProps<T = string | number>
  extends Omit<React.SelectHTMLAttributes<HTMLSelectElement>, 'onChange' | 'value'> {
  label?: string;
  options: SelectOption<T>[];
  value: T;
  onChange: (val: T) => void;
  error?: string;
  hotkey?: string;
}

export function Select<T extends string | number = string>({
  className,
  label,
  options,
  value,
  onChange,
  error,
  hotkey,
  id,
  disabled,
  ...props
}: SelectProps<T>) {
  const generatedId = React.useId();
  const selectId = id || generatedId;

  const handleChange = (e: React.ChangeEvent<HTMLSelectElement>) => {
    const rawVal = e.target.value;
    // Determine whether value type is number or string based on options
    const matchedOption = options.find((opt) => String(opt.value) === rawVal);
    if (matchedOption) {
      onChange(matchedOption.value);
    } else {
      onChange(rawVal as unknown as T);
    }
  };

  return (
    <div className="flex flex-col gap-1 w-full">
      {label && (
        <div className="flex items-center justify-between">
          <label
            htmlFor={selectId}
            className="text-xs font-medium text-slate-700 dark:text-slate-300 select-none"
          >
            {label}
          </label>
          {hotkey && (
            <kbd className="px-1 py-0.5 rounded bg-black/10 dark:bg-white/10 text-[10px] font-mono font-semibold uppercase">
              {hotkey}
            </kbd>
          )}
        </div>
      )}
      <div className="relative flex items-center w-full">
        <select
          id={selectId}
          value={String(value)}
          onChange={handleChange}
          disabled={disabled}
          aria-invalid={error ? true : undefined}
          aria-describedby={error ? `${selectId}-error` : undefined}
          className={cn(
            'w-full rounded-md border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-900 text-slate-900 dark:text-slate-100 text-sm px-3 py-1.5 pr-8 appearance-none focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-blue-500 disabled:opacity-50 disabled:bg-slate-50 dark:disabled:bg-slate-800 transition-colors cursor-pointer',
            error && 'border-rose-500 focus:ring-rose-500 focus:border-rose-500',
            className
          )}
          {...props}
        >
          {options.map((opt) => (
            <option key={String(opt.value)} value={String(opt.value)} disabled={opt.disabled}>
              {opt.label}
            </option>
          ))}
        </select>
        <ChevronDown className="absolute right-2.5 h-4 w-4 pointer-events-none text-slate-400 dark:text-slate-500" />
      </div>
      {error && (
        <p id={`${selectId}-error`} role="alert" className="text-xs text-rose-600 dark:text-rose-400 mt-0.5">
          {error}
        </p>
      )}
    </div>
  );
}

export default Select;
