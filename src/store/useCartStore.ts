import { create } from 'zustand';
import type {
  CartItem,
  ItemInput,
  BundleInput,
  CustomerInput,
} from '@/types/store';
import { validChargeAmount, type BillChargeLine } from '@/lib/billMath';

export type { ItemInput, BundleInput, CustomerInput };

/**
 * A cart line. Lines are keyed by `lineId`, never by item id (one item can have several lines).
 * The cart holds what the cashier entered and nothing more: line totals, bill totals, payment
 * and stock checks all come from `preview_sale` (DD-56).
 */
export type CartLine = CartItem & { lineId: string };
export type { CartLine as CartItemLine };

export interface CartState {
  items: CartLine[];
  /** Bill-level charge/discount lines (FR-19). */
  charges: BillChargeLine[];
  /** Units shown in the footer: Σ quantity x pack size. */
  totalBaseUnits: number;
  totalItems: number;
  selectedCustomer: CustomerInput | null;

  /**
   * Add a line. Merges into an existing line only when item, unit (bundle) and rate are all
   * identical. Rejects (returns false) a quantity or rate that is not a number in range
   * (a format check); stock is checked by the backend preview. `rate` overrides the bundle/base price.
   */
  addItem: (
    item: ItemInput,
    quantity?: number,
    bundle?: BundleInput | null,
    rate?: number,
    /** FR-47: typed line total; such a line never merges with another. */
    enteredTotal?: number | null
  ) => boolean;
  updateQuantity: (lineId: string, quantity: number) => boolean;
  /** Switch a line's unit; sets bundleId and the suggested rate. */
  setBundle: (lineId: string, bundle: BundleInput | null) => boolean;
  updateRate: (lineId: string, rate: number) => boolean;
  removeItem: (lineId: string) => void;
  /** Adds a charge/discount line; rejects (false) an amount that is not a positive number. */
  addCharge: (line: Omit<BillChargeLine, 'key'>) => boolean;
  updateChargeAmount: (key: number, amount: number) => boolean;
  removeCharge: (key: number) => void;
  /** Resets everything. */
  clearCart: () => void;
  setCustomer: (customer: CustomerInput | null) => void;
}

let lineSeq = 0;
const nextLineId = () => `line-${++lineSeq}`;

const validQty = (q: number) => typeof q === 'number' && Number.isFinite(q) && q > 0;
const validRate = (r: number) => typeof r === 'number' && Number.isFinite(r) && r >= 0;

let chargeSeq = 0;

function countsFor(items: CartLine[]) {
  return {
    totalBaseUnits: items.reduce((sum, i) => sum + i.quantity * i.unitsPerBundle, 0),
    totalItems: items.length,
  };
}

const initialState = {
  items: [] as CartLine[],
  charges: [] as BillChargeLine[],
  totalBaseUnits: 0,
  totalItems: 0,
  selectedCustomer: null as CustomerInput | null,
};

export const useCartStore = create<CartState>((set, get) => ({
  ...initialState,

  addItem: (item, quantity = 1, bundle = null, rate, enteredTotal = null) => {
    if (!validQty(quantity)) return false;
    if (rate !== undefined && !validRate(rate)) return false;
    if (enteredTotal != null && !validRate(enteredTotal)) return false;

    const multiplier = bundle ? bundle.unitsPerBundle : 1;
    if (!validQty(multiplier)) return false;
    const { items } = get();

    const unitRate =
      rate ?? (bundle ? bundle.salePrice ?? item.defaultSalePrice * multiplier : item.defaultSalePrice);
    const bundleId = bundle?.id;

    const existingIndex =
      enteredTotal != null
        ? -1
        : items.findIndex(
            (i) => i.itemId === item.id && i.bundleId === bundleId && i.rate === unitRate && i.enteredTotal == null
          );

    let updated: CartLine[];
    if (existingIndex >= 0) {
      updated = [...items];
      updated[existingIndex] = { ...items[existingIndex], quantity: items[existingIndex].quantity + quantity };
    } else {
      updated = [
        ...items,
        {
          lineId: nextLineId(),
          itemId: item.id,
          name: item.name,
          barcode: item.barcode,
          baseUnit: item.baseUnit,
          unitsPerBundle: multiplier,
          bundleId,
          bundleName: bundle ? bundle.bundleName : undefined,
          quantity,
          rate: unitRate,
          mrp: bundle?.mrp ?? item.mrp,
          defaultSalePrice: item.defaultSalePrice,
          enteredTotal,
        },
      ];
    }
    set({ items: updated, ...countsFor(updated) });
    return true;
  },

  updateQuantity: (lineId, quantity) => {
    if (!validQty(quantity)) return false;
    const { items } = get();
    const index = items.findIndex((i) => i.lineId === lineId);
    if (index === -1) return false;
    const updated = [...items];
    // FR-47: an edited total belonged to the old quantity, so the adjustment is dropped.
    const line = items[index];
    updated[index] = { ...line, quantity, enteredTotal: quantity === line.quantity ? line.enteredTotal : null };
    set({ items: updated, ...countsFor(updated) });
    return true;
  },

  setBundle: (lineId, bundle) => {
    const { items } = get();
    const index = items.findIndex((i) => i.lineId === lineId);
    if (index === -1) return false;
    const line = items[index];
    const multiplier = bundle ? bundle.unitsPerBundle : 1;
    if (!validQty(multiplier)) return false;
    const baseRate = line.defaultSalePrice ?? line.rate / line.unitsPerBundle;
    const rate = bundle ? bundle.salePrice ?? baseRate * multiplier : baseRate;
    const updated = [...items];
    updated[index] = {
      ...line,
      bundleId: bundle?.id,
      bundleName: bundle ? bundle.bundleName : undefined,
      unitsPerBundle: multiplier,
      rate,
      mrp: bundle?.mrp ?? line.mrp,
      enteredTotal: null,
    };
    set({ items: updated, ...countsFor(updated) });
    return true;
  },

  updateRate: (lineId, rate) => {
    if (!validRate(rate)) return false;
    const { items } = get();
    const index = items.findIndex((i) => i.lineId === lineId);
    if (index === -1) return false;
    const updated = [...items];
    const line = items[index];
    updated[index] = { ...line, rate, enteredTotal: rate === line.rate ? line.enteredTotal : null };
    set({ items: updated, ...countsFor(updated) });
    return true;
  },

  removeItem: (lineId) => {
    const updated = get().items.filter((i) => i.lineId !== lineId);
    set({ items: updated, ...countsFor(updated) });
  },

  addCharge: (line) => {
    if (!validChargeAmount(line.amount)) return false;
    set({ charges: [...get().charges, { ...line, key: ++chargeSeq }] });
    return true;
  },

  updateChargeAmount: (key, amount) => {
    if (!validChargeAmount(amount)) return false;
    const { charges } = get();
    if (!charges.some((c) => c.key === key)) return false;
    set({ charges: charges.map((c) => (c.key === key ? { ...c, amount } : c)) });
    return true;
  },

  removeCharge: (key) => set({ charges: get().charges.filter((c) => c.key !== key) }),

  clearCart: () => set({ ...initialState }),

  setCustomer: (customer) => set({ selectedCustomer: customer }),
}));
