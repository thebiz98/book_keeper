/**
 * Store-specific DTOs and Zustand state contracts for Book Keeper.
 * Compatible with docs/book_keeper_UI_LLD.md and src/__tests__/modules.d.ts.
 */

import type { PaymentStatus } from './index';

export interface CartItem {
  itemId: number;
  name: string;
  barcode?: string | null;
  baseUnit: string;
  unitsPerBundle: number;
  bundleId?: number;
  bundleName?: string;
  quantity: number;
  rate: number;
  mrp?: number | null;
  defaultSalePrice?: number;
  /** FR-47: the line total the cashier typed (null = qty × rate); cleared when qty, unit or rate change. */
  enteredTotal?: number | null;
}

export interface ItemInput {
  id: number;
  name: string;
  barcode?: string | null;
  baseUnit: string;
  defaultSalePrice: number;
  mrp?: number | null;
}

export interface BundleInput {
  id?: number;
  bundleName: string;
  unitsPerBundle: number;
  /** Price per bundle. */
  salePrice?: number | null;
  mrp?: number | null;
}

export interface CustomerInput {
  id: number;
  name: string;
  phone?: string | null;
}

export interface CartState {
  items: CartItem[];
  totalBaseUnits: number;
  totalItems: number;
  selectedCustomer: CustomerInput | null;
  paymentStatus: PaymentStatus;
  paidAmount: number;
  balanceDue: number;

  addItem: (item: ItemInput, quantity?: number, bundle?: BundleInput | null) => boolean;
  updateQuantity: (itemId: number, quantity: number) => boolean;
  setBundle: (itemId: number, bundle: BundleInput | null) => void;
  updateRate: (itemId: number, rate: number) => void;
  removeItem: (itemId: number) => void;
  clearCart: () => void;
  setCustomer: (customer: CustomerInput | null) => void;
  setPaymentStatus: (status: PaymentStatus, paidAmount?: number) => void;
  setPaidAmount: (amount: number) => void;
}

export interface ShopProfile {
  shopName: string;
  phone?: string;
  address?: string;
  taxId?: string;
}

export interface AppState {
  activeRoute: string;
  shopProfile: ShopProfile;
  isDbConnected: boolean;
  /** All-time cash in hand from the backend (FR-33); refreshed by `refreshMetrics()`. */
  cashInHand: number;

  setActiveRoute: (route: string) => void;
  setShopProfile: (profile: Partial<ShopProfile>) => void;
  setDbConnected: (connected: boolean) => void;
}

export interface ThemeState {
  theme: 'light' | 'dark';
  toggleTheme: () => void;
  setTheme: (theme: 'light' | 'dark') => void;
}
