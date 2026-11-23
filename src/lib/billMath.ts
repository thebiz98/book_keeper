import type { ChargeEffect } from '@/types';
import type { BillChargeInput } from '@/types/api';
import { round2 } from '@/lib/money';

/** One charge/discount line on the bill being built (FR-19). `key` identifies the row in the UI. */
export interface BillChargeLine {
  key: number;
  chargeTypeId: number;
  name: string;
  effect: ChargeEffect;
  amount: number;
}

export interface BillTotalsView {
  itemsSubtotal: number;
  chargesTotal: number;
  discountsTotal: number;
  grandTotal: number;
}

/**
 * Mirrors the backend `bill_math::compute_bill_totals`: each sum is rounded, the grand total is
 * `items + charges - discounts` and may be negative (the UI blocks submit when it is).
 */
export function billTotals(itemsSubtotal: number, lines: Pick<BillChargeLine, 'effect' | 'amount'>[]): BillTotalsView {
  const sum = (e: ChargeEffect) =>
    round2(lines.filter((l) => l.effect === e).reduce((s, l) => s + round2(l.amount), 0));
  const items = round2(itemsSubtotal);
  const chargesTotal = sum('CHARGE');
  const discountsTotal = sum('DISCOUNT');
  return {
    itemsSubtotal: items,
    chargesTotal,
    discountsTotal,
    grandTotal: round2(items + chargesTotal - discountsTotal),
  };
}

export function toChargeInputs(lines: BillChargeLine[]): BillChargeInput[] {
  return lines.map((l) => ({ chargeTypeId: l.chargeTypeId, amount: round2(l.amount) }));
}

/** A charge amount is valid when finite and positive after rounding to paise. */
export function validChargeAmount(n: number): boolean {
  return typeof n === 'number' && Number.isFinite(n) && round2(n) > 0;
}
