import { create } from 'zustand';
import type {
  ShopProfile,
  CashDrawerFlow,
  AppState,
} from '@/types/store';

export type { ShopProfile, CashDrawerFlow, AppState };

const defaultShopProfile: ShopProfile = {
  shopName: 'Gupta Stationery',
  phone: '9876543210',
  address: 'Main Bazaar',
  taxId: '07AAAAA0000A1Z5',
};

const initialCashState = {
  salesCash: 0,
  customerDuesSettled: 0,
  purchaseCash: 0,
  vendorDuesSettled: 0,
  refunds: 0,
  liveCashInDrawer: 0,
};

export const useAppStore = create<AppState>((set, get) => ({
  activeRoute: 'dashboard',
  shopProfile: defaultShopProfile,
  isDbConnected: true,
  ...initialCashState,

  setActiveRoute: (route: string): void => {
    set({ activeRoute: route });
  },

  setShopProfile: (profile: Partial<ShopProfile>): void => {
    set((state) => ({
      shopProfile: {
        ...state.shopProfile,
        ...profile,
      },
    }));
  },

  setDbConnected: (connected: boolean): void => {
    set({ isDbConnected: connected });
  },

  recordCashFlow: (flow: CashDrawerFlow): void => {
    const state = get();
    const salesCash = state.salesCash + (flow.salesCash ?? 0);
    const customerDuesSettled =
      state.customerDuesSettled + (flow.customerDuesSettled ?? 0);
    const purchaseCash = state.purchaseCash + (flow.purchaseCash ?? 0);
    const vendorDuesSettled =
      state.vendorDuesSettled + (flow.vendorDuesSettled ?? 0);
    const refunds = state.refunds + (flow.refunds ?? 0);

    const liveCashInDrawer =
      salesCash + customerDuesSettled - (purchaseCash + vendorDuesSettled + refunds);

    set({
      salesCash,
      customerDuesSettled,
      purchaseCash,
      vendorDuesSettled,
      refunds,
      liveCashInDrawer,
    });
  },

  resetCashDrawer: (): void => {
    set({ ...initialCashState });
  },

  calculateCashDrawer: (): number => {
    const state = get();
    const balance =
      state.salesCash +
      state.customerDuesSettled -
      (state.purchaseCash + state.vendorDuesSettled + state.refunds);

    set({ liveCashInDrawer: balance });
    return balance;
  },
}));
