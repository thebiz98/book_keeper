import { describe, it, expect, beforeEach } from 'vitest';
import { useAppStore } from '@/store/useAppStore';

// ============================================================================
// Test Suite: useAppStore
// Spec: docs/book_keeper_UI_LLD.md §4.1, §5.0, docs/book_keeper_UI_UX_Specification.md §2.1
// Formula: Drawer = (Sales Cash + Customer Dues) - (Purchase Cash + Vendor Dues + Refunds)
// ============================================================================

describe('useAppStore - Desktop Shell, Hotkey Routes & Cash Drawer', () => {
  beforeEach(() => {
    // Reset app store state
    useAppStore.setState({
      activeRoute: 'dashboard',
      salesCash: 0,
      customerDuesSettled: 0,
      purchaseCash: 0,
      vendorDuesSettled: 0,
      refunds: 0,
      isDbConnected: true,
      shopProfile: {
        shopName: 'Gupta Stationery',
        phone: '9876543210',
        address: 'Main Bazaar',
        taxId: '07AAAAA0000A1Z5',
      },
    });
  });

  describe('1. Active Route Navigation (F1–F6 Desktop Hotkeys)', () => {
    it('TC-APP-001: defaults to dashboard route on startup', () => {
      const state = useAppStore.getState();
      expect(state.activeRoute).toBe('dashboard');
    });

    it('TC-APP-002: navigates to New Sale route (F2 hotkey target)', () => {
      const store = useAppStore.getState();
      store.setActiveRoute('new-sale');
      expect(useAppStore.getState().activeRoute).toBe('new-sale');
    });

    it('TC-APP-003: navigates to New Purchase route (F3 hotkey target)', () => {
      const store = useAppStore.getState();
      store.setActiveRoute('new-purchase');
      expect(useAppStore.getState().activeRoute).toBe('new-purchase');
    });

    it('TC-APP-004: navigates to Credit & Dues Khata route (F5 hotkey target)', () => {
      const store = useAppStore.getState();
      store.setActiveRoute('credit-dues');
      expect(useAppStore.getState().activeRoute).toBe('credit-dues');
    });

    it('TC-APP-005: navigates to Inventory Master route (F6 hotkey target)', () => {
      const store = useAppStore.getState();
      store.setActiveRoute('inventory');
      expect(useAppStore.getState().activeRoute).toBe('inventory');
    });

    it('TC-APP-006: supports secondary views (reports, settings, returns, inventory)', () => {
      const store = useAppStore.getState();
      store.setActiveRoute('reports');
      expect(useAppStore.getState().activeRoute).toBe('reports');

      store.setActiveRoute('settings');
      expect(useAppStore.getState().activeRoute).toBe('settings');

      store.setActiveRoute('returns');
      expect(useAppStore.getState().activeRoute).toBe('returns');

      store.setActiveRoute('inventory');
      expect(useAppStore.getState().activeRoute).toBe('inventory');
    });
  });

  describe('2. Live Cash Drawer Balancing Formula', () => {
    it('TC-APP-007: drawer equals 0 when cash flows are all zero (fresh counter shift)', () => {
      const store = useAppStore.getState();
      const balance = store.calculateCashDrawer();
      expect(balance).toBe(0);
      expect(useAppStore.getState().liveCashInDrawer).toBe(0);
    });

    it('TC-APP-008: calculates balanced drawer for standard retail day', () => {
      // Scenario:
      // Inflows:
      //   - Upfront cash sales = ₹4,000.00
      //   - Customer dues settled in cash = ₹500.00
      // Outflows:
      //   - Upfront cash purchases = ₹1,500.00
      //   - Vendor dues settled in cash = ₹200.00
      //   - Customer cash refunds = ₹100.00
      // Formula: (4000 + 500) - (1500 + 200 + 100) = 4500 - 1800 = 2,700.00
      const store = useAppStore.getState();
      store.recordCashFlow({
        salesCash: 4000.0,
        customerDuesSettled: 500.0,
        purchaseCash: 1500.0,
        vendorDuesSettled: 200.0,
        refunds: 100.0,
      });

      const drawer = store.calculateCashDrawer();
      expect(drawer).toBe(2700.0);
      expect(useAppStore.getState().liveCashInDrawer).toBe(2700.0);
    });

    it('TC-APP-009: calculates cash drawer with sales cash only', () => {
      const store = useAppStore.getState();
      store.recordCashFlow({ salesCash: 1200.0 });

      expect(store.calculateCashDrawer()).toBe(1200.0);
    });

    it('TC-APP-010: handles negative cash drawer position when cash outflows exceed inflows', () => {
      // Scenario: Morning supplier payout ₹3,000 when drawer has only ₹1,000 opening sales
      // (1000 + 0) - (3000 + 0 + 0) = -2000.0
      const store = useAppStore.getState();
      store.recordCashFlow({
        salesCash: 1000.0,
        purchaseCash: 3000.0,
      });

      const drawer = store.calculateCashDrawer();
      expect(drawer).toBe(-2000.0);
    });

    it('TC-APP-011: increments cash drawer sequentially over incremental counter events', () => {
      const store = useAppStore.getState();

      // Event 1: Cash sale ₹1,000 -> drawer = 1000
      store.recordCashFlow({ salesCash: 1000.0 });
      expect(store.calculateCashDrawer()).toBe(1000.0);

      // Event 2: Customer pays ₹300 credit due -> drawer = 1300
      store.recordCashFlow({ customerDuesSettled: 300.0 });
      expect(store.calculateCashDrawer()).toBe(1300.0);

      // Event 3: Cash purchase inward ₹500 -> drawer = 800
      store.recordCashFlow({ purchaseCash: 500.0 });
      expect(store.calculateCashDrawer()).toBe(800.0);

      // Event 4: Customer return cash refund ₹150 -> drawer = 650
      store.recordCashFlow({ refunds: 150.0 });
      expect(store.calculateCashDrawer()).toBe(650.0);

      // Event 5: Vendor dues paid ₹200 -> drawer = 450
      store.recordCashFlow({ vendorDuesSettled: 200.0 });
      expect(store.calculateCashDrawer()).toBe(450.0);
    });

    it('TC-APP-012: resetCashDrawer clears all cash flow buckets to zero', () => {
      const store = useAppStore.getState();
      store.recordCashFlow({
        salesCash: 2500.0,
        purchaseCash: 1000.0,
      });
      expect(store.calculateCashDrawer()).toBe(1500.0);

      store.resetCashDrawer();

      const state = useAppStore.getState();
      expect(state.salesCash).toBe(0);
      expect(state.customerDuesSettled).toBe(0);
      expect(state.purchaseCash).toBe(0);
      expect(state.vendorDuesSettled).toBe(0);
      expect(state.refunds).toBe(0);
      expect(store.calculateCashDrawer()).toBe(0);
    });
  });

  describe('3. Shop Profile & Local Database Status', () => {
    it('TC-APP-013: setShopProfile updates header shop profile fields', () => {
      const store = useAppStore.getState();
      store.setShopProfile({
        shopName: 'National Book Depot',
        phone: '011-23456789',
        taxId: '07BBBBB1111B2Z6',
      });

      const updated = useAppStore.getState().shopProfile;
      expect(updated.shopName).toBe('National Book Depot');
      expect(updated.phone).toBe('011-23456789');
      expect(updated.taxId).toBe('07BBBBB1111B2Z6');
      expect(updated.address).toBe('Main Bazaar'); // Preserved
    });

    it('TC-APP-014: setDbConnected toggles local SQLite health flag', () => {
      const store = useAppStore.getState();
      expect(useAppStore.getState().isDbConnected).toBe(true);

      store.setDbConnected(false);
      expect(useAppStore.getState().isDbConnected).toBe(false);

      store.setDbConnected(true);
      expect(useAppStore.getState().isDbConnected).toBe(true);
    });
  });
});
