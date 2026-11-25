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
