import { describe, it, expect } from 'vitest';
import { toChargeInputs, validChargeAmount, type BillChargeLine } from '@/lib/billMath';

// DD-56: bill totals are the backend's; only the pure mapping and the format check live here.

const line = (key: number, effect: 'CHARGE' | 'DISCOUNT', amount: number): BillChargeLine => ({
  key,
  chargeTypeId: key,
  name: `c${key}`,
  effect,
  amount,
});

describe('billMath', () => {
  it('maps lines to payload inputs unchanged (the backend rounds)', () => {
    expect(toChargeInputs([line(7, 'CHARGE', 0.105), line(8, 'DISCOUNT', 20)])).toEqual([
      { chargeTypeId: 7, amount: 0.105 },
      { chargeTypeId: 8, amount: 20 },
    ]);
  });

  it('format check: a finite number above zero', () => {
    expect(validChargeAmount(0)).toBe(false);
    expect(validChargeAmount(-1)).toBe(false);
    expect(validChargeAmount(NaN)).toBe(false);
    expect(validChargeAmount(Infinity)).toBe(false);
    expect(validChargeAmount(0.01)).toBe(true);
  });
});
