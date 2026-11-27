import { describe, it, expect } from 'vitest';
import { lineBaseQuantity, type PurchaseLine } from '@/components/domain/purchaseMath';
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

// DD-56: line totals, bill totals and the payment rules are the backend's (preview_purchase, tested in
// Rust). Only the display-only unit conversions remain here.
describe('purchase line unit conversions (display only)', () => {
  it('converts bundles to base units (the cost per base unit is the backend preview, DD-97)', () => {
    const l = line();
    expect(lineBaseQuantity(l)).toBe(100);
  });

  it('supports decimal quantities', () => {
    expect(lineBaseQuantity(line({ bundleId: null, unitsPerBundle: 1, quantity: 2.5 }))).toBe(2.5);
  });

  it('avoids float noise in base units (0.1 * 3)', () => {
    expect(lineBaseQuantity(line({ quantity: 0.1, unitsPerBundle: 3 }))).toBe(0.3);
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
