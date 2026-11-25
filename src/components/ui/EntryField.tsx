import React from 'react';
import { cn } from '@/lib/utils';

export interface EntryFieldProps {
  /** Empty string keeps the slot (for a button that lines up with the inputs). */
  label: string;
  required?: boolean;
  htmlFor?: string;
  /** Reserved line under the control; never changes the row height. */
  helper?: React.ReactNode;
  className?: string;
  children: React.ReactNode;
}

/** One entry-row column: fixed-height label slot, the control, fixed-height helper slot (FR-30). */
export const EntryField: React.FC<EntryFieldProps> = ({
  label,
  required,
  htmlFor,
  helper,
  className,
  children,
}) => {
  const text = label ? `${label}${required ? ' *' : ''}` : '';
  return (
    <div className={cn('flex flex-col gap-1 min-w-0', className)}>
      <label
        htmlFor={htmlFor}
        title={text || undefined}
        className="h-4 leading-4 truncate text-xs font-semibold text-slate-700 dark:text-slate-300 select-none"
      >
        {text}
      </label>
      {children}
      <div className="min-h-[14px] text-[10px] leading-[14px] text-slate-500 dark:text-slate-400 truncate">
        {helper}
      </div>
    </div>
  );
};

export default EntryField;
