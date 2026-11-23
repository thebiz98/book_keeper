import { describe, it, expect } from 'vitest';
import { billTotals, toChargeInputs, validChargeAmount, type BillChargeLine } from '@/lib/billMath';

const line = (key: number, effect: 'CHARGE' | 'DISCOUNT', amount: number): BillChargeLine => ({
  key,
  chargeTypeId: key,
  name: `c${key}`,
  effect,
  amount,
});

describe('billMath', () => {
  it('adds charges and subtracts discounts', () => {
    const t = billTotals(200, [line(1, 'CHARGE', 50), line(2, 'DISCOUNT', 20)]);
    expect(t).toEqual({ itemsSubtotal: 200, chargesTotal: 50, discountsTotal: 20, grandTotal: 230 });
  });

  it('rounds sums without float drift (0.1 + 0.2)', () => {
    const t = billTotals(100, [line(1, 'CHARGE', 0.1), line(2, 'CHARGE', 0.2)]);
    expect(t.chargesTotal).toBe(0.3);
    expect(t.grandTotal).toBe(100.3);
  });

  it('lets the grand total go negative so the UI can block submit', () => {
    expect(billTotals(100, [line(1, 'DISCOUNT', 150)]).grandTotal).toBe(-50);
  });

  it('maps lines to payload inputs with rounded amounts', () => {
    expect(toChargeInputs([line(7, 'CHARGE', 0.105)])).toEqual([{ chargeTypeId: 7, amount: 0.11 }]);
  });

  it('validates amounts after rounding', () => {
    expect(validChargeAmount(0.004)).toBe(false);
    expect(validChargeAmount(0)).toBe(false);
    expect(validChargeAmount(NaN)).toBe(false);
    expect(validChargeAmount(0.01)).toBe(true);
  });
});
