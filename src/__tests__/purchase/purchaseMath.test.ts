import { describe, it, expect } from 'vitest';
import {
  effectivePaid,
  lineBaseQuantity,
  lineCostPerBase,
  lineTotal,
  purchaseTotal,
  validatePayment,
  type PurchaseLine,
} from '@/components/domain/purchaseMath';
import { prefillFromLocation, queuePurchasePrefill, takePurchasePrefill } from '@/pages/purchasePrefill';

const line = (over: Partial<PurchaseLine> = {}): PurchaseLine => ({
  key: 1,
  itemId: 1,
  itemName: 'Pen',
  baseUnit: 'PCS',
  bundleId: 5,
  packLabel: 'Box',
  unitsPerBundle: 25,
  quantity: 4,
  unitCost: 100,
  ...over,
});

describe('purchase line math', () => {
  it('converts bundles to base units and normalises cost', () => {
    const l = line();
    expect(lineBaseQuantity(l)).toBe(100);
    expect(lineCostPerBase(l)).toBe(4);
    expect(lineTotal(l)).toBe(400);
  });

  it('supports decimal quantities and rounds line totals to 2 dp', () => {
    const l = line({ bundleId: null, unitsPerBundle: 1, quantity: 2.5, unitCost: 33.333 });
    expect(lineBaseQuantity(l)).toBe(2.5);
    expect(lineTotal(l)).toBe(83.33);
    expect(purchaseTotal([l, line()])).toBe(483.33);
  });

  it('avoids float noise in base units (0.1 * 3)', () => {
    expect(lineBaseQuantity(line({ quantity: 0.1, unitsPerBundle: 3 }))).toBe(0.3);
  });
});

describe('payment cross-check', () => {
  it('PAID needs paid = total', () => {
    expect(validatePayment('PAID', 100, 100, null)).toBeNull();
    expect(validatePayment('PAID', 100, 50, null)).not.toBeNull();
  });
  it('PARTIAL needs 0 < paid < total and a vendor', () => {
    expect(validatePayment('PARTIAL', 100, 40, 3)).toBeNull();
    expect(validatePayment('PARTIAL', 100, 0, 3)).not.toBeNull();
    expect(validatePayment('PARTIAL', 100, 100, 3)).not.toBeNull();
    expect(validatePayment('PARTIAL', 100, 40, null)).toMatch(/vendor/i);
  });
  it('UNPAID needs paid = 0 and a vendor', () => {
    expect(validatePayment('UNPAID', 100, 0, 3)).toBeNull();
    expect(validatePayment('UNPAID', 100, 0, null)).toMatch(/vendor/i);
    expect(validatePayment('UNPAID', 100, 10, 3)).not.toBeNull();
  });
  it('paid can never exceed total', () => {
    expect(validatePayment('PAID', 100, 100.01, 3)).toMatch(/exceed/i);
    expect(validatePayment('PARTIAL', 100, 150, 3)).toMatch(/exceed/i);
  });
  it('effectivePaid derives the amount from the status', () => {
    expect(effectivePaid('PAID', 99.999, 5)).toBe(100);
    expect(effectivePaid('UNPAID', 100, 5)).toBe(0);
    expect(effectivePaid('PARTIAL', 100, 40)).toBe(40);
  });
});

describe('purchase prefill hand-off', () => {
  it('queue is consumed once', () => {
    queuePurchasePrefill({ itemId: 9, name: 'Pen' });
    expect(takePurchasePrefill()).toEqual({ itemId: 9, name: 'Pen' });
    expect(takePurchasePrefill()).toBeNull();
  });
  it('parses ?item= from search or hash', () => {
    expect(prefillFromLocation({ search: '?item=12', hash: '' })).toEqual({ itemId: 12 });
    expect(prefillFromLocation({ search: '', hash: '#/purchase?item=Gel%20Pen' })).toEqual({ name: 'Gel Pen' });
    expect(prefillFromLocation({ search: '', hash: '' })).toBeNull();
  });
});
