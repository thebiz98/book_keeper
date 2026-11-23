import { describe, it, expect, beforeEach } from 'vitest';
import { useCartStore } from '@/store/useCartStore';
import type { ItemInput, BundleInput } from '@/store/useCartStore';

const pencil: ItemInput = {
  id: 1,
  name: 'Pencil',
  baseUnit: 'PCS',
  defaultSalePrice: 10,
  mrp: 12,
  currentStock: 50,
};
const box10: BundleInput = { id: 5, bundleName: 'Box of 10', unitsPerBundle: 10, salePrice: 90 };
const box12: BundleInput = { id: 6, bundleName: 'Box of 12', unitsPerBundle: 12, salePrice: 100 };

const s = () => useCartStore.getState();

describe('useCartStore (F6 rules)', () => {
  beforeEach(() => s().clearCart());

  describe('addItem', () => {
    it('adds a loose line with a line id, quantity 1 and base price', () => {
      expect(s().addItem(pencil)).toBe(true);
      expect(s().items[0]).toMatchObject({ itemId: 1, quantity: 1, baseQuantity: 1, rate: 10, lineTotal: 10 });
      expect(s().items[0].lineId).toBeTruthy();
      expect(s().subtotal).toBe(10);
    });

    it('supports decimal quantity and explicit rate override', () => {
      expect(s().addItem(pencil, 2.5, null, 8)).toBe(true);
      expect(s().items[0]).toMatchObject({ quantity: 2.5, rate: 8, lineTotal: 20 });
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
      expect(s().items[2]).toMatchObject({ quantity: 2, baseQuantity: 20, bundleId: 5 });
    });

    it('keeps quantity * unitsPerBundle === baseQuantity after merging', () => {
      s().addItem(pencil, 1, box12);
      s().addItem(pencil, 2, box12);
      const line = s().items[0];
      expect(line.quantity).toBe(3);
      expect(line.baseQuantity).toBe(36);
    });

    it('checks stock aggregated across lines of the same item', () => {
      s().addItem(pencil, 1, box10); // 10
      s().addItem(pencil, 3, box10, 85); // +30 => 40, separate line (rate differs)
      expect(s().addItem(pencil, 11, null)).toBe(false); // 40 + 11 > 50
      expect(s().addItem(pencil, 10, null)).toBe(true); // exactly 50
      expect(s().addItem(pencil, 1e-12, null)).toBe(true); // within 1e-9 epsilon
    });

    it('rejects zero, negative and NaN quantity and out-of-stock items', () => {
      expect(s().addItem(pencil, 0)).toBe(false);
      expect(s().addItem(pencil, -1)).toBe(false);
      expect(s().addItem(pencil, NaN)).toBe(false);
      expect(s().addItem({ ...pencil, currentStock: 0 })).toBe(false);
      expect(s().items).toHaveLength(0);
    });

    it('uses bundle price, falling back to base price times units', () => {
      s().addItem(pencil, 2, box10);
      expect(s().items[0]).toMatchObject({ rate: 90, lineTotal: 180 });
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
    });

    it('rechecks aggregated stock including other lines', () => {
      s().addItem(pencil, 40);
      s().addItem(pencil, 1, null, 9);
      const second = s().items[1].lineId;
      expect(s().updateQuantity(second, 10)).toBe(true);
      expect(s().updateQuantity(second, 11)).toBe(false);
      expect(s().items[1].quantity).toBe(10);
    });
  });

  describe('setBundle', () => {
    it('sets bundleId, multiplier and rate', () => {
      s().addItem(pencil, 2);
      const id = s().items[0].lineId;
      expect(s().setBundle(id, box10)).toBe(true);
      expect(s().items[0]).toMatchObject({ bundleId: 5, unitsPerBundle: 10, baseQuantity: 20, rate: 90, lineTotal: 180 });
      expect(s().setBundle(id, null)).toBe(true);
      expect(s().items[0]).toMatchObject({ bundleId: undefined, unitsPerBundle: 1, baseQuantity: 2, rate: 10 });
    });

    it('refuses and keeps the previous unit when stock would be exceeded', () => {
      s().addItem({ ...pencil, currentStock: 15 }, 10);
      const id = s().items[0].lineId;
      expect(s().setBundle(id, box12)).toBe(false);
      expect(s().items[0]).toMatchObject({ bundleId: undefined, baseQuantity: 10 });
    });
  });

  describe('rate, remove, rounding', () => {
    it('updateRate rejects negative/NaN and rounds line totals with round2', () => {
      s().addItem(pencil, 3);
      const id = s().items[0].lineId;
      expect(s().updateRate(id, -1)).toBe(false);
      expect(s().updateRate(id, NaN)).toBe(false);
      expect(s().updateRate(id, 3.33)).toBe(true);
      expect(s().items[0].lineTotal).toBe(9.99);
      expect(s().subtotal).toBe(9.99);
    });

    it('subtotal is rounded across lines', () => {
      s().addItem(pencil, 1, null, 0.1);
      s().addItem({ ...pencil, id: 2 }, 1, null, 0.2);
      expect(s().subtotal).toBe(0.3);
    });

    it('removes by line id, leaving other lines of the same item', () => {
      s().addItem(pencil, 1);
      s().addItem(pencil, 1, box10);
      s().removeItem(s().items[0].lineId);
      expect(s().items).toHaveLength(1);
      expect(s().items[0].bundleId).toBe(5);
      expect(s().totalItems).toBe(1);
    });

    it('clearCart resets customer and totals', () => {
      s().addItem(pencil, 1);
      s().setCustomer({ id: 3, name: 'A' });
      s().clearCart();
      expect(s().items).toEqual([]);
      expect(s().selectedCustomer).toBeNull();
      expect(s().subtotal).toBe(0);
    });
  });

  describe('charges (FR-19)', () => {
    const transport = { chargeTypeId: 1, name: 'Transport', effect: 'CHARGE' as const, amount: 50 };
    const discount = { chargeTypeId: 2, name: 'Cash Discount', effect: 'DISCOUNT' as const, amount: 20 };

    it('recomputes the grand total on add, update and remove', () => {
      s().addItem(pencil, 2, null, 100);
      expect(s().addCharge(transport)).toBe(true);
      expect(s().addCharge(discount)).toBe(true);
      expect(s()).toMatchObject({ subtotal: 200, chargesTotal: 50, discountsTotal: 20, grandTotal: 230 });
      const key = s().charges[1].key;
      expect(s().updateChargeAmount(key, 30)).toBe(true);
      expect(s().grandTotal).toBe(220);
      s().removeCharge(key);
      expect(s()).toMatchObject({ discountsTotal: 0, grandTotal: 250 });
    });

    it('recomputes the grand total when an item changes', () => {
      s().addItem(pencil, 1, null, 100);
      s().addCharge(transport);
      const id = s().items[0].lineId;
      s().updateQuantity(id, 3);
      expect(s().grandTotal).toBe(350);
    });

    it('rejects invalid amounts and clears with the cart', () => {
      expect(s().addCharge({ ...transport, amount: 0 })).toBe(false);
      expect(s().addCharge({ ...transport, amount: Number.NaN })).toBe(false);
      expect(s().addCharge({ ...transport, amount: -5 })).toBe(false);
      expect(s().charges).toHaveLength(0);
      s().addCharge(transport);
      s().clearCart();
      expect(s()).toMatchObject({ charges: [], chargesTotal: 0, grandTotal: 0 });
    });
  });
});
