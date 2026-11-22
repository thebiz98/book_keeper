import { create } from 'zustand';
import { api } from '@/api';
import type { ShopProfile, CashDrawerFlow, AppState } from '@/types/store';

export type { ShopProfile, CashDrawerFlow, AppState };

/** AppState (frozen contract in src/types/store.ts) plus the F6 shell additions. */
export interface AppStoreState extends AppState {
  /** True once `getShopSettings` has resolved (or failed). */
  shopLoaded: boolean;
  /** DB file label for the header; `null` renders "custom path" (path cannot be known). */
  dbLabel: string | null;
  /** Item lookup popover (F4 / Ctrl+F / header button). */
  lookupOpen: boolean;
  /**
   * Ctrl+B contract: `requestBackup()` routes to Settings and sets this flag.
   * SettingsPage reacts with `useEffect(() => { if (consumeBackupRequest()) runBackup(); }, [backupRequested])`.
   */
  backupRequested: boolean;

  loadShopSettings: () => Promise<void>;
  /** Refetch `getDashboardMetrics().liveCashInDrawer` into `liveCashInDrawer`. */
  refreshMetrics: () => Promise<void>;
  setDbLabel: (label: string | null) => void;
  openLookup: () => void;
  closeLookup: () => void;
  requestBackup: () => void;
  /** Returns true (and clears the flag) when a backup was requested. */
  consumeBackupRequest: () => boolean;
}

export const useAppStore = create<AppStoreState>((set, get) => ({
  activeRoute: 'dashboard',
  shopProfile: { shopName: '' },
  shopLoaded: false,
  isDbConnected: true,
  dbLabel: 'shop.db',
  lookupOpen: false,
  backupRequested: false,
  salesCash: 0,
  customerDuesSettled: 0,
  purchaseCash: 0,
  vendorDuesSettled: 0,
  refunds: 0,
  liveCashInDrawer: 0,

  setActiveRoute: (route: string): void => {
    set({ activeRoute: route });
  },

  setShopProfile: (profile: Partial<ShopProfile>): void => {
    set((state) => ({ shopProfile: { ...state.shopProfile, ...profile } }));
  },

  setDbConnected: (connected: boolean): void => {
    set({ isDbConnected: connected });
  },

  setDbLabel: (label: string | null): void => set({ dbLabel: label }),

  loadShopSettings: async (): Promise<void> => {
    try {
      const s = await api.getShopSettings();
      set({
        shopProfile: {
          shopName: s.shopName,
          phone: s.phone ?? undefined,
          address: s.address ?? undefined,
          taxId: s.taxId ?? undefined,
        },
        shopLoaded: true,
      });
    } catch {
      set({ shopLoaded: true });
    }
  },

  refreshMetrics: async (): Promise<void> => {
    try {
      const m = await api.getDashboardMetrics();
      set({ liveCashInDrawer: m.liveCashInDrawer });
    } catch {
      /* keep the last known value */
    }
  },

  openLookup: (): void => set({ lookupOpen: true }),
  closeLookup: (): void => set({ lookupOpen: false }),

  requestBackup: (): void => set({ activeRoute: 'settings', backupRequested: true }),
  consumeBackupRequest: (): boolean => {
    if (!get().backupRequested) return false;
    set({ backupRequested: false });
    return true;
  },

  /** @deprecated The backend ledger is the source of truth; use `refreshMetrics()`. */
  recordCashFlow: (_flow: CashDrawerFlow): void => {
    void get().refreshMetrics();
  },

  /** @deprecated Use `refreshMetrics()`. */
  resetCashDrawer: (): void => {
    set({
      salesCash: 0,
      customerDuesSettled: 0,
      purchaseCash: 0,
      vendorDuesSettled: 0,
      refunds: 0,
      liveCashInDrawer: 0,
    });
  },

  /** @deprecated Returns the last metric value; use `refreshMetrics()`. */
  calculateCashDrawer: (): number => get().liveCashInDrawer,
}));
