import { describe, it, expect, beforeEach } from 'vitest';
import { useCartStore } from '@/store/useCartStore';
import type { ItemInput, BundleInput } from '@/store/useCartStore';

// DD-56: the cart holds what was entered. Line totals, bill totals and stock checks are the backend's
// (`preview_sale`), so there is nothing money- or stock-related to assert here.

const pencil: ItemInput = {
  id: 1,
  name: 'Pencil',
  baseUnit: 'PCS',
  defaultSalePrice: 10,
  mrp: 12,
};
const box10: BundleInput = { id: 5, bundleName: 'Box of 10', unitsPerBundle: 10, salePrice: 90 };

const s = () => useCartStore.getState();

describe('useCartStore (entry rules only)', () => {
  beforeEach(() => s().clearCart());

  describe('addItem', () => {
    it('adds a loose line with a line id, quantity 1 and the base price as the rate', () => {
      expect(s().addItem(pencil)).toBe(true);
      expect(s().items[0]).toMatchObject({ itemId: 1, quantity: 1, rate: 10, unitsPerBundle: 1 });
      expect(s().items[0].lineId).toBeTruthy();
      expect(s().totalItems).toBe(1);
    });

    it('supports decimal quantity and explicit rate override', () => {
      expect(s().addItem(pencil, 2.5, null, 8)).toBe(true);
      expect(s().items[0]).toMatchObject({ quantity: 2.5, rate: 8 });
    });

    it('merges only when item, unit and rate are all the same', () => {
      s().addItem(pencil, 2);
      s().addItem(pencil, 3);
      expect(s().items).toHaveLength(1);
      expect(s().items[0].quantity).toBe(5);

      s().addItem(pencil, 1, null, 9); // different rate
      expect(s().items).toHaveLength(2);

      s().addItem(pencil, 1, box10); // different unit
      expect(s().items).toHaveLength(3);

      s().addItem(pencil, 1, box10); // same bundle + same rate merges
      expect(s().items).toHaveLength(3);
      expect(s().items[2]).toMatchObject({ quantity: 2, unitsPerBundle: 10, bundleId: 5 });
    });

    it('rejects zero, negative and NaN quantity and a negative or NaN rate (format checks)', () => {
      expect(s().addItem(pencil, 0)).toBe(false);
      expect(s().addItem(pencil, -1)).toBe(false);
      expect(s().addItem(pencil, NaN)).toBe(false);
      expect(s().addItem(pencil, 1, null, -2)).toBe(false);
      expect(s().addItem(pencil, 1, null, NaN)).toBe(false);
      expect(s().items).toHaveLength(0);
    });

    it('does not check stock: that is the backend preview', () => {
      expect(s().addItem(pencil, 100000)).toBe(true);
    });

    it('uses the bundle price as the rate, falling back to base price times units', () => {
      s().addItem(pencil, 2, box10);
      expect(s().items[0].rate).toBe(90);
      s().addItem(pencil, 1, { id: 9, bundleName: 'Pack of 5', unitsPerBundle: 5 });
      expect(s().items[1].rate).toBe(50);
    });
  });

  describe('updateQuantity', () => {
    it('rejects <= 0 and NaN, keeps the line', () => {
      s().addItem(pencil, 2);
      const id = s().items[0].lineId;
      expect(s().updateQuantity(id, 0)).toBe(false);
      expect(s().updateQuantity(id, -3)).toBe(false);
      expect(s().updateQuantity(id, NaN)).toBe(false);
      expect(s().items[0].quantity).toBe(2);
      expect(s().updateQuantity(id, 7)).toBe(true);
      expect(s().items[0].quantity).toBe(7);
    });
  });

  describe('setBundle', () => {
    it('sets bundleId, multiplier and the suggested rate', () => {
      s().addItem(pencil, 2);
      const id = s().items[0].lineId;
      expect(s().setBundle(id, box10)).toBe(true);
      expect(s().items[0]).toMatchObject({ bundleId: 5, unitsPerBundle: 10, rate: 90 });
      expect(s().totalBaseUnits).toBe(20);
      expect(s().setBundle(id, null)).toBe(true);
      expect(s().items[0]).toMatchObject({ bundleId: undefined, unitsPerBundle: 1, rate: 10 });
    });
  });

  describe('rate, remove, clear', () => {
    it('updateRate rejects negative/NaN', () => {
      s().addItem(pencil, 3);
      const id = s().items[0].lineId;
      expect(s().updateRate(id, -1)).toBe(false);
      expect(s().updateRate(id, NaN)).toBe(false);
      expect(s().updateRate(id, 3.33)).toBe(true);
      expect(s().items[0].rate).toBe(3.33);
    });

    it('removes by line id, leaving other lines of the same item', () => {
      s().addItem(pencil, 1);
      s().addItem(pencil, 1, box10);
      s().removeItem(s().items[0].lineId);
      expect(s().items).toHaveLength(1);
      expect(s().items[0].bundleId).toBe(5);
      expect(s().totalItems).toBe(1);
    });

    it('clearCart resets customer and lines', () => {
      s().addItem(pencil, 1);
      s().setCustomer({ id: 3, name: 'A' });
      s().clearCart();
      expect(s().items).toEqual([]);
      expect(s().selectedCustomer).toBeNull();
      expect(s().totalItems).toBe(0);
    });
  });

  describe('charges (FR-19)', () => {
    const transport = { chargeTypeId: 1, name: 'Transport', effect: 'CHARGE' as const, amount: 50 };
    const discount = { chargeTypeId: 2, name: 'Cash Discount', effect: 'DISCOUNT' as const, amount: 20 };

    it('adds, updates and removes lines; totals are not kept here', () => {
      expect(s().addCharge(transport)).toBe(true);
      expect(s().addCharge(discount)).toBe(true);
      expect(s().charges.map((c) => c.amount)).toEqual([50, 20]);
      const key = s().charges[1].key;
      expect(s().updateChargeAmount(key, 30)).toBe(true);
      expect(s().charges[1].amount).toBe(30);
      s().removeCharge(key);
      expect(s().charges).toHaveLength(1);
    });

    it('rejects amounts that are not a positive number and clears with the cart', () => {
      expect(s().addCharge({ ...transport, amount: 0 })).toBe(false);
      expect(s().addCharge({ ...transport, amount: Number.NaN })).toBe(false);
      expect(s().addCharge({ ...transport, amount: -5 })).toBe(false);
      expect(s().charges).toHaveLength(0);
      s().addCharge(transport);
      s().clearCart();
      expect(s().charges).toEqual([]);
    });
  });
});
