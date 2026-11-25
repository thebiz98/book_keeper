import React from 'react';
import { CurrencyDisplay } from '@/components/common/CurrencyDisplay';

export interface BillFooterTotalsProps {
  /** Every figure comes from the bill's backend preview (DD-56). */
  itemsSubtotal: number;
  chargesTotal: number;
  discountsTotal: number;
  totalAmount: number;
  /** With no charge / discount lines only the (big) subtotal shows. */
  hasCharges: boolean;
  /** Test id prefix: `cart` on the sale page, `purchase` on the purchase page. */
  testIdPrefix: string;
}

/** The right-aligned totals stack in the new sale and new purchase footers (same place, same look). */
export const BillFooterTotals: React.FC<BillFooterTotalsProps> = ({
  itemsSubtotal,
  chargesTotal,
  discountsTotal,
  totalAmount,
  hasCharges,
  testIdPrefix,
}) => (
  <div className="flex flex-col items-end gap-0.5 text-right">
    <div className="flex items-baseline gap-3">
      <span className="text-xs text-slate-500 font-medium">{hasCharges ? 'Items Subtotal' : 'Subtotal'}</span>
      <div
        data-testid={`${testIdPrefix}-subtotal`}
        className={
          hasCharges
            ? 'text-base font-semibold font-mono tabular-nums text-slate-700 dark:text-slate-300'
            : 'text-2xl font-bold font-mono tabular-nums text-slate-900 dark:text-slate-100'
        }
      >
        <CurrencyDisplay amount={itemsSubtotal} size={hasCharges ? 'md' : 'xl'} />
      </div>
    </div>
    {hasCharges && (
      <>
        <div className="flex items-baseline gap-3 text-xs font-mono">
          <span className="text-slate-500">Charges (+)</span>
          <span data-testid={`${testIdPrefix}-charges-total`} className="text-emerald-700 dark:text-emerald-400">
            <CurrencyDisplay amount={chargesTotal} size="sm" />
          </span>
        </div>
        <div className="flex items-baseline gap-3 text-xs font-mono">
          <span className="text-slate-500">Discounts (−)</span>
          <span data-testid={`${testIdPrefix}-discounts-total`} className="text-rose-700 dark:text-rose-400">
            <CurrencyDisplay amount={discountsTotal} size="sm" />
          </span>
        </div>
        <div className="flex items-baseline gap-3">
          <span className="text-xs text-slate-500 font-medium">Grand Total</span>
          <div
            data-testid={`${testIdPrefix}-grand-total`}
            className="text-2xl font-bold font-mono tabular-nums text-slate-900 dark:text-slate-100"
          >
            <CurrencyDisplay amount={totalAmount} size="xl" />
          </div>
        </div>
      </>
    )}
  </div>
);

export default BillFooterTotals;
