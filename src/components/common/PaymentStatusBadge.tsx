import React from 'react';
import { cn } from '@/lib/utils';
import type { PaymentStatus } from '@/types';
import { CurrencyDisplay } from './CurrencyDisplay';

export interface PaymentStatusBadgeProps {
  status: PaymentStatus;
  balanceDue?: number;
  className?: string;
}

export const PaymentStatusBadge: React.FC<PaymentStatusBadgeProps> = ({
  status,
  balanceDue,
  className,
}) => {
  const normStatus = status;

  let styles =
    'bg-slate-100 text-slate-700 border border-slate-200 dark:bg-slate-800 dark:text-slate-300 dark:border-slate-700';

  if (normStatus === 'Paid') {
    styles =
      'bg-emerald-50 text-emerald-700 border border-emerald-200 dark:bg-emerald-950/60 dark:text-emerald-300 dark:border-emerald-800';
  } else if (normStatus === 'Partial') {
    styles =
      'bg-amber-50 text-amber-700 border border-amber-200 dark:bg-amber-950/60 dark:text-amber-300 dark:border-amber-800';
  } else if (normStatus === 'Unpaid') {
    styles =
      'bg-rose-50 text-rose-700 border border-rose-200 dark:bg-rose-950/60 dark:text-rose-300 dark:border-rose-800';
  }

  return (
    <span
      className={cn(
        'inline-flex items-center gap-1.5 px-2 py-0.5 rounded text-xs font-semibold select-none tabular-nums',
        styles,
        className
      )}
    >
      <span>{normStatus.toUpperCase()}</span>
      {balanceDue !== undefined && balanceDue > 0 && normStatus !== 'Paid' && (
        <span className="opacity-80 font-normal">
          (Due: <CurrencyDisplay amount={balanceDue} size="sm" />)
        </span>
      )}
    </span>
  );
};

export default PaymentStatusBadge;
