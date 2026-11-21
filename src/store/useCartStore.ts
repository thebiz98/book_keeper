import { create } from 'zustand';
import type {
  CartItem,
  ItemInput,
  BundleInput,
  CustomerInput,
  CartState,
} from '@/types/store';
import type { PaymentStatus } from '@/types';

export type { CartItem, ItemInput, BundleInput, CustomerInput, CartState };

function computeTotals(
  items: CartItem[],
  status: 'Paid' | 'Partial' | 'Unpaid',
  paidAmount: number
) {
  const subtotal = items.reduce((sum, item) => sum + item.lineTotal, 0);
  const totalBaseUnits = items.reduce((sum, item) => sum + item.baseQuantity, 0);
  const totalItems = items.length;

  let balanceDue = 0;
  if (status === 'Paid') {
    balanceDue = 0;
  } else if (status === 'Partial') {
    balanceDue = Math.max(0, subtotal - paidAmount);
  } else if (status === 'Unpaid') {
    balanceDue = subtotal;
  }

  return { subtotal, totalBaseUnits, totalItems, balanceDue };
}

const initialState = {
  items: [] as CartItem[],
  subtotal: 0,
  totalBaseUnits: 0,
  totalItems: 0,
  selectedCustomer: null as CustomerInput | null,
  paymentStatus: 'Paid' as PaymentStatus,
  paidAmount: 0,
  balanceDue: 0,
};

export const useCartStore = create<CartState>((set, get) => ({
  ...initialState,

  addItem: (item: ItemInput, quantity = 1, bundle: BundleInput | null = null): boolean => {
    const availableStock = item.currentStock;
    if (availableStock <= 0 || quantity <= 0) {
      return false;
    }

    const multiplier = bundle
      ? bundle.unitsPerBundle
      : 1;
    const addedBaseQuantity = quantity * multiplier;

    const { items, paymentStatus, paidAmount } = get();
    const existingIndex = items.findIndex((i) => i.itemId === item.id);

    if (existingIndex >= 0) {
      const existing = items[existingIndex];
      const newBaseQuantity = existing.baseQuantity + addedBaseQuantity;
      if (newBaseQuantity > existing.availableStock) {
        return false;
      }

      const updatedQuantity = existing.quantity + quantity;
      const updatedLineTotal = updatedQuantity * existing.rate;

      const updatedItems = [...items];
      updatedItems[existingIndex] = {
        ...existing,
        quantity: updatedQuantity,
        baseQuantity: newBaseQuantity,
        lineTotal: updatedLineTotal,
      };

      const totals = computeTotals(updatedItems, paymentStatus, paidAmount);
      set({ items: updatedItems, ...totals });
      return true;
    }

    if (addedBaseQuantity > availableStock) {
      return false;
    }

    const defaultRate = item.defaultSalePrice;
    const bundleRate = bundle
      ? bundle.salePrice
      : undefined;
    const rate = bundleRate != null ? bundleRate : defaultRate;

    const newItem: CartItem = {
      itemId: item.id,
      name: item.name,
      barcode: item.barcode,
      baseUnit: item.baseUnit,
      unitsPerBundle: multiplier,
      bundleId: bundle?.id,
      bundleName: bundle ? bundle.bundleName : undefined,
      quantity,
      baseQuantity: addedBaseQuantity,
      rate,
      mrp: bundle?.mrp ?? item.mrp,
      lineTotal: quantity * rate,
      availableStock,
      defaultSalePrice: defaultRate,
    };

    const updatedItems = [...items, newItem];
    const totals = computeTotals(updatedItems, paymentStatus, paidAmount);
    set({ items: updatedItems, ...totals });
    return true;
  },

  updateQuantity: (itemId: number, quantity: number): boolean => {
    if (quantity < 0) {
      return false;
    }

    const { items, paymentStatus, paidAmount } = get();
    const index = items.findIndex((i) => i.itemId === itemId);
    if (index === -1) {
      return false;
    }

    const item = items[index];
    const newBaseQuantity = quantity * item.unitsPerBundle;
    if (newBaseQuantity > item.availableStock) {
      return false;
    }

    const updatedItems = [...items];
    updatedItems[index] = {
      ...item,
      quantity,
      baseQuantity: newBaseQuantity,
      lineTotal: quantity * item.rate,
    };

    const totals = computeTotals(updatedItems, paymentStatus, paidAmount);
    set({ items: updatedItems, ...totals });
    return true;
  },

  setBundle: (itemId: number, bundle: BundleInput | null): void => {
    const { items, paymentStatus, paidAmount } = get();
    const index = items.findIndex((i) => i.itemId === itemId);
    if (index === -1) {
      return;
    }

    const item = items[index];
    const updatedItems = [...items];

    if (bundle != null) {
      const multiplier = bundle.unitsPerBundle;
      const bundleRate = bundle.salePrice ?? item.rate;

      const baseQuantity = item.quantity * multiplier;
      const rate = bundleRate;
      const lineTotal = item.quantity * rate;

      updatedItems[index] = {
        ...item,
        unitsPerBundle: multiplier,
        bundleName: bundle.bundleName,
        rate,
        baseQuantity,
        lineTotal,
        mrp: bundle.mrp ?? item.mrp,
      };
    } else {
      const rate = item.defaultSalePrice ?? item.rate;
      const baseQuantity = item.quantity;
      const lineTotal = item.quantity * rate;

      updatedItems[index] = {
        ...item,
        unitsPerBundle: 1,
        bundleName: undefined,
        rate,
        baseQuantity,
        lineTotal,
      };
    }

    const totals = computeTotals(updatedItems, paymentStatus, paidAmount);
    set({ items: updatedItems, ...totals });
  },

  updateRate: (itemId: number, rate: number): void => {
    const { items, paymentStatus, paidAmount } = get();
    const index = items.findIndex((i) => i.itemId === itemId);
    if (index === -1) {
      return;
    }

    const item = items[index];
    const updatedItems = [...items];
    updatedItems[index] = {
      ...item,
      rate,
      lineTotal: item.quantity * rate,
    };

    const totals = computeTotals(updatedItems, paymentStatus, paidAmount);
    set({ items: updatedItems, ...totals });
  },

  removeItem: (itemId: number): void => {
    const { items, paymentStatus, paidAmount } = get();
    const updatedItems = items.filter((i) => i.itemId !== itemId);
    const totals = computeTotals(updatedItems, paymentStatus, paidAmount);
    set({ items: updatedItems, ...totals });
  },

  clearCart: (): void => {
    set({ ...initialState });
  },

  setCustomer: (customer: CustomerInput | null): void => {
    set({ selectedCustomer: customer });
  },

  setPaymentStatus: (
    status: 'Paid' | 'Partial' | 'Unpaid',
    paidAmount?: number
  ): void => {
    const { subtotal } = get();
    let currentPaid = get().paidAmount;

    if (status === 'Paid') {
      set({ paymentStatus: status, balanceDue: 0 });
    } else if (status === 'Partial') {
      if (paidAmount !== undefined) {
        currentPaid = paidAmount;
      }
      set({
        paymentStatus: status,
        paidAmount: currentPaid,
        balanceDue: Math.max(0, subtotal - currentPaid),
      });
    } else if (status === 'Unpaid') {
      set({
        paymentStatus: status,
        paidAmount: 0,
        balanceDue: subtotal,
      });
    }
  },

  setPaidAmount: (amount: number): void => {
    const { subtotal, paymentStatus } = get();
    let balanceDue = 0;
    if (paymentStatus === 'Paid') {
      balanceDue = 0;
    } else if (paymentStatus === 'Partial') {
      balanceDue = Math.max(0, subtotal - amount);
    } else if (paymentStatus === 'Unpaid') {
      balanceDue = subtotal;
    }
    set({ paidAmount: amount, balanceDue });
  },
}));
