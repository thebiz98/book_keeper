import type { ChargeEffect } from '@/types';
import type { BillChargeInput } from '@/types/api';

/**
 * One charge/discount line on the bill being built (FR-19). `key` identifies the row in the UI.
 * The bill totals are never computed here: they come from the backend preview (DD-56).
 */
export interface BillChargeLine {
  key: number;
  chargeTypeId: number;
  name: string;
  effect: ChargeEffect;
  amount: number;
}

/** Pure mapping of the UI lines to the command payload. */
export function toChargeInputs(lines: BillChargeLine[]): BillChargeInput[] {
  return lines.map((l) => ({ chargeTypeId: l.chargeTypeId, amount: l.amount }));
}

/** Format check only: a charge amount is a finite number above zero (the backend rounds and validates). */
export function validChargeAmount(n: number): boolean {
  return typeof n === 'number' && Number.isFinite(n) && n > 0;
}

// TEMPORARY (removed with the purchase and lot-price commits of DD-56): the old TS totals rule.
import { round2 } from '@/lib/money';
export interface BillTotalsView {
  itemsSubtotal: number;
  chargesTotal: number;
  discountsTotal: number;
  grandTotal: number;
}
export function billTotals(itemsSubtotal: number, lines: Pick<BillChargeLine, 'effect' | 'amount'>[]): BillTotalsView {
  const sum = (e: ChargeEffect) =>
    round2(lines.filter((l) => l.effect === e).reduce((s, l) => s + round2(l.amount), 0));
  const items = round2(itemsSubtotal);
  const chargesTotal = sum('CHARGE');
  const discountsTotal = sum('DISCOUNT');
  return { itemsSubtotal: items, chargesTotal, discountsTotal, grandTotal: round2(items + chargesTotal - discountsTotal) };
}
