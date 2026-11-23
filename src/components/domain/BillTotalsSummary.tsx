import React from 'react';
import { formatINR } from '@/lib/money';
import { cn } from '@/lib/utils';
import type { ChargeEffect } from '@/types';

export interface BillTotalsSummaryProps {
  itemsSubtotal: number;
  lines: { name: string; effect: ChargeEffect; amount: number }[];
  grandTotal: number;
  compact?: boolean;
}

/** Items subtotal, each charge/discount line and the grand total (FR-19). */
export const BillTotalsSummary: React.FC<BillTotalsSummaryProps> = ({ itemsSubtotal, lines, grandTotal, compact }) => (
  <div className={cn('flex flex-col gap-0.5 font-mono', compact ? 'text-xs' : 'text-sm')} data-testid="bill-totals">
    <div className="flex justify-between">
      <span className="text-slate-500">Items subtotal</span>
      <span data-testid="bt-items">{formatINR(itemsSubtotal)}</span>
    </div>
    {lines.map((l, i) => (
      <div key={i} className="flex justify-between" data-testid="bt-line">
        <span className="text-slate-500">{l.name}</span>
        <span className={l.effect === 'CHARGE' ? 'text-emerald-700 dark:text-emerald-400' : 'text-rose-700 dark:text-rose-400'}>
          {l.effect === 'CHARGE' ? '+' : '−'}
          {formatINR(l.amount)}
        </span>
      </div>
    ))}
    <div className="flex justify-between border-t border-slate-200 dark:border-slate-700 pt-0.5 font-semibold">
      <span>Grand total</span>
      <span data-testid="bt-grand">{formatINR(grandTotal)}</span>
    </div>
  </div>
);

export default BillTotalsSummary;
