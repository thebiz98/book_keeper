import React from 'react';
import { X } from 'lucide-react';
import { cn } from '@/lib/utils';

export interface TextInputProps extends React.InputHTMLAttributes<HTMLInputElement> {
  label?: string;
  error?: string;
  autoSelectOnFocus?: boolean;
  prefixIcon?: React.ReactNode;
  suffixAction?: React.ReactNode;
  onClear?: () => void;
}

export const TextInput = React.forwardRef<HTMLInputElement, TextInputProps>(
  (
    {
      className,
      label,
      error,
      autoSelectOnFocus,
      prefixIcon,
      suffixAction,
      onClear,
      onFocus,
      id,
      value,
      ...props
    },
    ref
  ) => {
    const generatedId = React.useId();
    const inputId = id || generatedId;

    const handleFocus = (e: React.FocusEvent<HTMLInputElement>) => {
      if (autoSelectOnFocus) {
        e.currentTarget.select();
      }
      onFocus?.(e);
    };

    return (
      <div className="flex flex-col gap-1 w-full">
        {label && (
          <label
            htmlFor={inputId}
            className="text-xs font-medium text-slate-700 dark:text-slate-300 select-none"
          >
            {label}
          </label>
        )}
        <div className="relative flex items-center w-full">
          {prefixIcon && (
            <div className="absolute left-2.5 flex items-center pointer-events-none text-slate-400 dark:text-slate-500">
              {prefixIcon}
            </div>
          )}
          <input
            ref={ref}
            id={inputId}
            value={value}
            aria-invalid={error ? true : undefined}
            aria-describedby={error ? `${inputId}-error` : undefined}
            onFocus={handleFocus}
            className={cn(
              'w-full rounded-md border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-900 text-slate-900 dark:text-slate-100 text-sm px-3 py-1.5 placeholder-slate-400 focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-blue-500 disabled:opacity-50 disabled:bg-slate-50 dark:disabled:bg-slate-800 transition-colors',
              prefixIcon ? 'pl-8' : '',
              suffixAction || onClear ? 'pr-8' : '',
              error && 'border-rose-500 focus:ring-rose-500 focus:border-rose-500',
              className
            )}
            {...props}
          />
          {onClear && value && (
            <button
              type="button"
              onClick={onClear}
              className="absolute right-2.5 p-0.5 text-slate-400 hover:text-slate-600 dark:text-slate-500 dark:hover:text-slate-300 rounded cursor-pointer"
            >
              <X className="h-3.5 w-3.5" />
            </button>
          )}
          {suffixAction && !onClear && (
            <div className="absolute right-2.5 flex items-center">{suffixAction}</div>
          )}
        </div>
        {error && (
          <p id={`${inputId}-error`} role="alert" className="text-xs text-rose-600 dark:text-rose-400 mt-0.5">
            {error}
          </p>
        )}
      </div>
    );
  }
);

TextInput.displayName = 'TextInput';
export default TextInput;
