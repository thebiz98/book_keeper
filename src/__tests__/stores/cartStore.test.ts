import { describe, it, expect, beforeEach } from 'vitest';
import { useCartStore } from '@/store/useCartStore';

// ============================================================================
// Test Suite: useCartStore
// Spec: docs/book_keeper_UI_LLD.md §5, TEST_INFRA.md §4.1, PROJECT.md F33, F41
// ============================================================================

describe('useCartStore - Retail Billing Cart State Management', () => {
  beforeEach(() => {
    // Reset cart state before every test
    useCartStore.getState().clearCart();
  });

  describe('1. addItem', () => {
    it('TC-CART-001: adds loose item to cart with default quantity 1 and default price', () => {
      const store = useCartStore.getState();
      const mockItem = {
        id: 1,
        name: 'Apsara Platinum Pencil',
        barcode: '8901030381023',
        baseUnit: 'PCS',
        defaultSalePrice: 10.0,
        currentStock: 50.0,
      };

      const success = store.addItem(mockItem);
      expect(success).toBe(true);

      const state = useCartStore.getState();
      expect(state.items).toHaveLength(1);
      expect(state.items[0]).toMatchObject({
        itemId: 1,
        name: 'Apsara Platinum Pencil',
        quantity: 1,
        baseQuantity: 1,
        rate: 10.0,
        lineTotal: 10.0,
        unitsPerBundle: 1,
      });
      expect(state.subtotal).toBe(10.0);
      expect(state.totalBaseUnits).toBe(1);
      expect(state.totalItems).toBe(1);
    });

    it('TC-CART-002: adds item with custom quantity and recalculates line total', () => {
      const store = useCartStore.getState();
      const mockItem = {
        id: 2,
        name: 'Classmate Spiral Notebook',
        baseUnit: 'PCS',
        defaultSalePrice: 65.0,
        currentStock: 20.0,
      };

      const success = store.addItem(mockItem, 4);
      expect(success).toBe(true);

      const state = useCartStore.getState();
      expect(state.items[0].quantity).toBe(4);
      expect(state.items[0].baseQuantity).toBe(4);
      expect(state.items[0].lineTotal).toBe(260.0); // 4 * 65.0
      expect(state.subtotal).toBe(260.0);
      expect(state.totalBaseUnits).toBe(4);
    });

    it('TC-CART-003: merges line item if same item is added again up to available stock', () => {
      const store = useCartStore.getState();
      const mockItem = {
        id: 3,
        name: 'Cello Gripper Pen',
        baseUnit: 'PCS',
        defaultSalePrice: 10.0,
        currentStock: 15.0,
      };

      store.addItem(mockItem, 3);
      store.addItem(mockItem, 2);

      const state = useCartStore.getState();
      expect(state.items).toHaveLength(1);
      expect(state.items[0].quantity).toBe(5);
      expect(state.items[0].baseQuantity).toBe(5);
      expect(state.items[0].lineTotal).toBe(50.0);
      expect(state.subtotal).toBe(50.0);
    });
  });

  describe('2. Bundle Multiplier Conversion', () => {
    it('TC-CART-004: converts bundle multiplier to base quantity (Box of 10 -> 2 Boxes = 20 base units)', () => {
      const store = useCartStore.getState();
      const mockItem = {
        id: 10,
        name: 'Camlin Exam Marker',
        baseUnit: 'PCS',
        defaultSalePrice: 25.0,
        currentStock: 100.0,
      };
      const bundle = {
        bundleName: 'Box of 10',
        unitsPerBundle: 10,
        salePrice: 240.0, // Discounted bundle rate (24 ₹/pc)
      };

      store.addItem(mockItem, 2, bundle);

      const state = useCartStore.getState();
      expect(state.items[0].quantity).toBe(2);
      expect(state.items[0].unitsPerBundle).toBe(10);
      expect(state.items[0].baseQuantity).toBe(20); // 2 * 10 = 20 base units
      expect(state.items[0].bundleName).toBe('Box of 10');
      expect(state.items[0].lineTotal).toBe(480.0); // 2 * 240.0
      expect(state.totalBaseUnits).toBe(20);
      expect(state.subtotal).toBe(480.0);
    });

    it('TC-CART-005: setBundle updates existing line item multiplier and line total', () => {
      const store = useCartStore.getState();
      const mockItem = {
        id: 11,
        name: 'Reynolds Ballpoint Pen',
        baseUnit: 'PCS',
        defaultSalePrice: 5.0,
        currentStock: 200.0,
      };

      // Add 2 loose pens
      store.addItem(mockItem, 2);
      expect(useCartStore.getState().items[0].baseQuantity).toBe(2);
      expect(useCartStore.getState().items[0].lineTotal).toBe(10.0);

      // Change packaging to Dozen (12 units, ₹55/dozen)
      store.setBundle(11, {
        bundleName: 'Dozen',
        unitsPerBundle: 12,
        salePrice: 55.0,
      });

      const updated = useCartStore.getState();
      expect(updated.items[0].unitsPerBundle).toBe(12);
      expect(updated.items[0].baseQuantity).toBe(24); // 2 * 12 = 24 PCS
      expect(updated.items[0].lineTotal).toBe(110.0); // 2 * 55.0
      expect(updated.subtotal).toBe(110.0);
      expect(updated.totalBaseUnits).toBe(24);
    });

    it('TC-CART-006: resetting bundle to null returns to loose base unit rate', () => {
      const store = useCartStore.getState();
      const mockItem = {
        id: 12,
        name: 'Sticky Notes Pad',
        baseUnit: 'PCS',
        defaultSalePrice: 40.0,
        currentStock: 50.0,
      };

      store.addItem(mockItem, 3, {
        bundleName: 'Pack of 5',
        unitsPerBundle: 5,
        salePrice: 180.0,
      });
      expect(useCartStore.getState().items[0].baseQuantity).toBe(15);

      // Revert to loose base unit
      store.setBundle(12, null);

      const state = useCartStore.getState();
      expect(state.items[0].unitsPerBundle).toBe(1);
      expect(state.items[0].baseQuantity).toBe(3); // 3 loose PCS
      expect(state.items[0].rate).toBe(40.0);
      expect(state.items[0].lineTotal).toBe(120.0); // 3 * 40.0
      expect(state.totalBaseUnits).toBe(3);
    });
  });

  describe('3. Negative Stock Protection (Cannot Exceed Available Stock)', () => {
    it('TC-CART-007: blocks adding item when requested quantity exceeds available stock', () => {
      const store = useCartStore.getState();
      const mockItem = {
        id: 20,
        name: 'Casio Scientific Calculator FX-991CW',
        baseUnit: 'PCS',
        defaultSalePrice: 1250.0,
        currentStock: 3.0, // Only 3 available
      };

      // Attempt to add 4 calculators (exceeds stock of 3)
      const success = store.addItem(mockItem, 4);
      expect(success).toBe(false);

      const state = useCartStore.getState();
      expect(state.items).toHaveLength(0);
      expect(state.subtotal).toBe(0);
    });

    it('TC-CART-008: blocks adding item with zero stock (Out of Stock)', () => {
      const store = useCartStore.getState();
      const mockItem = {
        id: 21,
        name: 'Fevicol MR 100g',
        baseUnit: 'BOTTLE',
        defaultSalePrice: 45.0,
        currentStock: 0.0,
      };

      const success = store.addItem(mockItem, 1);
      expect(success).toBe(false);
      expect(useCartStore.getState().items).toHaveLength(0);
    });

    it('TC-CART-009: blocks bundle selection if base units exceed available stock', () => {
      const store = useCartStore.getState();
      const mockItem = {
        id: 22,
        name: 'Pilot V5 Liquid Ink Pen',
        baseUnit: 'PCS',
        defaultSalePrice: 50.0,
        currentStock: 15.0, // Only 15 available
      };

      // Adding 2 Boxes of 10 requires 20 PCS, but only 15 available
      const bundle = {
        bundleName: 'Box of 10',
        unitsPerBundle: 10,
        salePrice: 480.0,
      };

      const success = store.addItem(mockItem, 2, bundle);
      expect(success).toBe(false);
      expect(useCartStore.getState().items).toHaveLength(0);
    });

    it('TC-CART-010: updateQuantity blocks increment exceeding available stock', () => {
      const store = useCartStore.getState();
      const mockItem = {
        id: 23,
        name: 'Kangaro Stapler No. 10',
        baseUnit: 'PCS',
        defaultSalePrice: 75.0,
        currentStock: 5.0,
      };

      store.addItem(mockItem, 3);
      expect(useCartStore.getState().items[0].quantity).toBe(3);

      // Attempt to update quantity to 6 (stock is 5)
      const success = store.updateQuantity(23, 6);
      expect(success).toBe(false);

      // Quantity remains unchanged at 3
      expect(useCartStore.getState().items[0].quantity).toBe(3);
    });

    it('TC-CART-011: allows purchasing exact stock boundary (quantity == availableStock)', () => {
      const store = useCartStore.getState();
      const mockItem = {
        id: 24,
        name: 'A4 Copier Paper Ream',
        baseUnit: 'REAM',
        defaultSalePrice: 320.0,
        currentStock: 5.0,
      };

      const success = store.addItem(mockItem, 5);
      expect(success).toBe(true);
      expect(useCartStore.getState().items[0].quantity).toBe(5);
      expect(useCartStore.getState().subtotal).toBe(1600.0);
    });
  });

  describe('4. updateQuantity and updateRate', () => {
    it('TC-CART-012: updateQuantity updates base units and subtotal', () => {
      const store = useCartStore.getState();
      store.addItem({
        id: 30,
        name: 'Highlighter Pen',
        baseUnit: 'PCS',
        defaultSalePrice: 20.0,
        currentStock: 25.0,
      }, 2);

      expect(useCartStore.getState().subtotal).toBe(40.0);

      store.updateQuantity(30, 4);
      const state = useCartStore.getState();
      expect(state.items[0].quantity).toBe(4);
      expect(state.items[0].baseQuantity).toBe(4);
      expect(state.items[0].lineTotal).toBe(80.0);
      expect(state.subtotal).toBe(80.0);
    });

    it('TC-CART-013: updateRate recalculates line total and cart subtotal', () => {
      const store = useCartStore.getState();
      store.addItem({
        id: 31,
        name: 'Binder Clip 32mm',
        baseUnit: 'PCS',
        defaultSalePrice: 15.0,
        currentStock: 100.0,
      }, 5);

      expect(useCartStore.getState().subtotal).toBe(75.0);

      // Apply discounted rate of ₹12.0
      store.updateRate(31, 12.0);
      const state = useCartStore.getState();
      expect(state.items[0].rate).toBe(12.0);
      expect(state.items[0].lineTotal).toBe(60.0); // 5 * 12.0
      expect(state.subtotal).toBe(60.0);
    });
  });

  describe('5. removeItem and clearCart', () => {
    it('TC-CART-014: removeItem removes line item and decrements subtotal', () => {
      const store = useCartStore.getState();
      store.addItem({
        id: 40,
        name: 'Item A',
        baseUnit: 'PCS',
        defaultSalePrice: 100.0,
        currentStock: 10.0,
      }, 1);
      store.addItem({
        id: 41,
        name: 'Item B',
        baseUnit: 'PCS',
        defaultSalePrice: 50.0,
        currentStock: 10.0,
      }, 2);

      expect(useCartStore.getState().subtotal).toBe(200.0);
      expect(useCartStore.getState().items).toHaveLength(2);

      store.removeItem(40);

      const state = useCartStore.getState();
      expect(state.items).toHaveLength(1);
      expect(state.items[0].itemId).toBe(41);
      expect(state.subtotal).toBe(100.0); // 2 * 50.0
      expect(state.totalBaseUnits).toBe(2);
    });

    it('TC-CART-015: clearCart resets all items, subtotal, and balances to clean initial state', () => {
      const store = useCartStore.getState();
      store.addItem({
        id: 50,
        name: 'Item C',
        baseUnit: 'PCS',
        defaultSalePrice: 30.0,
        currentStock: 50.0,
      }, 3);
      store.setCustomer({ id: 9, name: 'Sharmaji' });
      store.setPaymentStatus('Partial', 50.0);

      expect(useCartStore.getState().items).toHaveLength(1);
      expect(useCartStore.getState().selectedCustomer).not.toBeNull();

      store.clearCart();

      const cleared = useCartStore.getState();
      expect(cleared.items).toHaveLength(0);
      expect(cleared.subtotal).toBe(0);
      expect(cleared.totalBaseUnits).toBe(0);
      expect(cleared.totalItems).toBe(0);
      expect(cleared.selectedCustomer).toBeNull();
      expect(cleared.paymentStatus).toBe('Paid');
      expect(cleared.paidAmount).toBe(0);
      expect(cleared.balanceDue).toBe(0);
    });
  });

  describe('6. Customer Selection & Payment Balance Due', () => {
    it('TC-CART-016: calculates balance due accurately for PARTIAL and UNPAID sales', () => {
      const store = useCartStore.getState();
      store.addItem({
        id: 60,
        name: 'Desk Organiser',
        baseUnit: 'PCS',
        defaultSalePrice: 500.0,
        currentStock: 10.0,
      }, 2); // Subtotal = 1000

      // Case A: Default PAID -> balance due is 0
      expect(useCartStore.getState().balanceDue).toBe(0);

      // Case B: PARTIAL payment of ₹400 -> balance due = 600
      store.setPaymentStatus('Partial', 400.0);
      expect(useCartStore.getState().paidAmount).toBe(400.0);
      expect(useCartStore.getState().balanceDue).toBe(600.0);

      // Case C: Full UNPAID credit sale -> balance due = 1000
      store.setPaymentStatus('Unpaid');
      expect(useCartStore.getState().paidAmount).toBe(0.0);
      expect(useCartStore.getState().balanceDue).toBe(1000.0);
    });
  });
});
