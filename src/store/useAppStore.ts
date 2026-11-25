import { create } from 'zustand';
import { api } from '@/api';
import type { ShopProfile, AppState } from '@/types/store';

export type { ShopProfile, AppState };

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
  /** Refetch `getDashboardMetrics().cashInHand` into `cashInHand`. */
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
  cashInHand: 0,

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
      set({ cashInHand: m.cashInHand });
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
}));
