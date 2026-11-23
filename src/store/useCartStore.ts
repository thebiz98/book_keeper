import { create } from 'zustand';
import type {
  CartItem,
  ItemInput,
  BundleInput,
  CustomerInput,
} from '@/types/store';
import { round2 } from '@/lib/money';
import { billTotals, validChargeAmount, type BillChargeLine } from '@/lib/billMath';

export type { ItemInput, BundleInput, CustomerInput };

/** Stock comparison tolerance (DD-10). */
export const STOCK_EPS = 1e-9;

/** A cart line. Lines are keyed by `lineId`, never by item id (one item can have several lines). */
export type CartLine = CartItem & { lineId: string };
export type { CartLine as CartItemLine };

export interface CartState {
  items: CartLine[];
  /** Items subtotal (before charges and discounts). */
  subtotal: number;
  /** Bill-level charge/discount lines (FR-19). */
  charges: BillChargeLine[];
  chargesTotal: number;
  discountsTotal: number;
  /** items subtotal + charges - discounts; the amount the customer owes. */
  grandTotal: number;
  totalBaseUnits: number;
  totalItems: number;
  selectedCustomer: CustomerInput | null;
  /** FR-20: set while a sale is being edited; null when creating a new one. */
  editing: { saleId: number; code: string } | null;
  /**
   * FR-20: base units per item that the bill being edited already holds. The edit reverses them
   * first, so they count as available on top of the item's current stock.
   */
  editCredit: Record<number, number>;

  /**
   * Add a line. Merges into an existing line only when item, unit (bundle) and
   * rate are all identical. Rejects (returns false) a non-positive/NaN quantity
   * or when the item's total base quantity across all lines would exceed stock.
   * `rate` overrides the bundle/base price.
   */
  addItem: (
    item: ItemInput,
    quantity?: number,
    bundle?: BundleInput | null,
    rate?: number
  ) => boolean;
  updateQuantity: (lineId: string, quantity: number) => boolean;
  /** Switch a line's unit; sets bundleId, reprices and rechecks aggregated stock. */
  setBundle: (lineId: string, bundle: BundleInput | null) => boolean;
  updateRate: (lineId: string, rate: number) => boolean;
  removeItem: (lineId: string) => void;
  /** Adds a charge/discount line; rejects (false) an amount that is not positive after rounding. */
  addCharge: (line: Omit<BillChargeLine, 'key'>) => boolean;
  updateChargeAmount: (key: number, amount: number) => boolean;
  removeCharge: (key: number) => void;
  /** Resets everything, including edit mode. */
  clearCart: () => void;
  setCustomer: (customer: CustomerInput | null) => void;
  /**
   * FR-20: replaces the cart with an existing bill's lines and enters edit mode. Lines must
   * already carry `availableStock = currentStock + credit`.
   */
  loadForEdit: (
    lines: Omit<CartLine, 'lineId'>[],
    customer: CustomerInput | null,
    editing: { saleId: number; code: string },
    credit: Record<number, number>,
    charges?: Omit<BillChargeLine, 'key'>[]
  ) => void;
}

let lineSeq = 0;
const nextLineId = () => `line-${++lineSeq}`;

const validQty = (q: number) => typeof q === 'number' && Number.isFinite(q) && q > 0;
const validRate = (r: number) => typeof r === 'number' && Number.isFinite(r) && r >= 0;

let chargeSeq = 0;

function totalsFor(items: CartLine[], charges: BillChargeLine[] = []) {
  const subtotal = round2(items.reduce((sum, i) => sum + i.lineTotal, 0));
  const t = billTotals(subtotal, charges);
  return {
    subtotal,
    chargesTotal: t.chargesTotal,
    discountsTotal: t.discountsTotal,
    grandTotal: t.grandTotal,
    totalBaseUnits: items.reduce((sum, i) => sum + i.baseQuantity, 0),
    totalItems: items.length,
  };
}

/** Base quantity of `itemId` across all lines, optionally excluding one line. */
export function cartBaseQuantity(items: CartLine[], itemId: number, excludeLineId?: string): number {
  return items
    .filter((i) => i.itemId === itemId && i.lineId !== excludeLineId)
    .reduce((s, i) => s + i.baseQuantity, 0);
}

const initialState = {
  items: [] as CartLine[],
  subtotal: 0,
  charges: [] as BillChargeLine[],
  chargesTotal: 0,
  discountsTotal: 0,
  grandTotal: 0,
  totalBaseUnits: 0,
  totalItems: 0,
  selectedCustomer: null as CustomerInput | null,
  editing: null as { saleId: number; code: string } | null,
  editCredit: {} as Record<number, number>,
};

export const useCartStore = create<CartState>((set, get) => ({
  ...initialState,

  addItem: (item, quantity = 1, bundle = null, rate) => {
    if (!validQty(quantity)) return false;
    const credit = get().editCredit[item.id] ?? 0;
    const stock = item.currentStock + credit;
    if (stock <= 0) return false;
    if (rate !== undefined && !validRate(rate)) return false;

    const multiplier = bundle ? bundle.unitsPerBundle : 1;
    if (!validQty(multiplier)) return false;
    const addedBase = quantity * multiplier;
    const { items } = get();

    if (cartBaseQuantity(items, item.id) + addedBase > stock + STOCK_EPS) {
      return false;
    }

    const unitRate =
      rate ?? (bundle ? bundle.salePrice ?? item.defaultSalePrice * multiplier : item.defaultSalePrice);
    const bundleId = bundle?.id;

    const existingIndex = items.findIndex(
      (i) => i.itemId === item.id && i.bundleId === bundleId && i.rate === unitRate
    );

    let updated: CartLine[];
    if (existingIndex >= 0) {
      const existing = items[existingIndex];
      const q = existing.quantity + quantity;
      updated = [...items];
      updated[existingIndex] = {
        ...existing,
        quantity: q,
        baseQuantity: q * existing.unitsPerBundle,
        lineTotal: round2(q * existing.rate),
        availableStock: stock,
      };
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
          baseQuantity: addedBase,
          rate: unitRate,
          mrp: bundle?.mrp ?? item.mrp,
          lineTotal: round2(quantity * unitRate),
          availableStock: stock,
          defaultSalePrice: item.defaultSalePrice,
        },
      ];
    }
    set({ items: updated, ...totalsFor(updated, get().charges) });
    return true;
  },

  updateQuantity: (lineId, quantity) => {
    if (!validQty(quantity)) return false;
    const { items } = get();
    const index = items.findIndex((i) => i.lineId === lineId);
    if (index === -1) return false;
    const line = items[index];
    const base = quantity * line.unitsPerBundle;
    if (cartBaseQuantity(items, line.itemId, lineId) + base > line.availableStock + STOCK_EPS) {
      return false;
    }
    const updated = [...items];
    updated[index] = {
      ...line,
      quantity,
      baseQuantity: base,
      lineTotal: round2(quantity * line.rate),
    };
    set({ items: updated, ...totalsFor(updated, get().charges) });
    return true;
  },

  setBundle: (lineId, bundle) => {
    const { items } = get();
    const index = items.findIndex((i) => i.lineId === lineId);
    if (index === -1) return false;
    const line = items[index];
    const multiplier = bundle ? bundle.unitsPerBundle : 1;
    if (!validQty(multiplier)) return false;
    const base = line.quantity * multiplier;
    if (cartBaseQuantity(items, line.itemId, lineId) + base > line.availableStock + STOCK_EPS) {
      return false;
    }
    const baseRate = line.defaultSalePrice ?? line.rate / line.unitsPerBundle;
    const rate = bundle ? bundle.salePrice ?? baseRate * multiplier : baseRate;
    const updated = [...items];
    updated[index] = {
      ...line,
      bundleId: bundle?.id,
      bundleName: bundle ? bundle.bundleName : undefined,
      unitsPerBundle: multiplier,
      baseQuantity: base,
      rate,
      mrp: bundle?.mrp ?? line.mrp,
      lineTotal: round2(line.quantity * rate),
    };
    set({ items: updated, ...totalsFor(updated, get().charges) });
    return true;
  },

  updateRate: (lineId, rate) => {
    if (!validRate(rate)) return false;
    const { items } = get();
    const index = items.findIndex((i) => i.lineId === lineId);
    if (index === -1) return false;
    const updated = [...items];
    updated[index] = { ...items[index], rate, lineTotal: round2(items[index].quantity * rate) };
    set({ items: updated, ...totalsFor(updated, get().charges) });
    return true;
  },

  removeItem: (lineId) => {
    const updated = get().items.filter((i) => i.lineId !== lineId);
    set({ items: updated, ...totalsFor(updated, get().charges) });
  },

  addCharge: (line) => {
    if (!validChargeAmount(line.amount)) return false;
    const charges = [...get().charges, { ...line, amount: round2(line.amount), key: ++chargeSeq }];
    set({ charges, ...totalsFor(get().items, charges) });
    return true;
  },

  updateChargeAmount: (key, amount) => {
    if (!validChargeAmount(amount)) return false;
    const { charges, items } = get();
    if (!charges.some((c) => c.key === key)) return false;
    const next = charges.map((c) => (c.key === key ? { ...c, amount: round2(amount) } : c));
    set({ charges: next, ...totalsFor(items, next) });
    return true;
  },

  removeCharge: (key) => {
    const charges = get().charges.filter((c) => c.key !== key);
    set({ charges, ...totalsFor(get().items, charges) });
  },

  clearCart: () => set({ ...initialState }),

  setCustomer: (customer) => set({ selectedCustomer: customer }),

  loadForEdit: (lines, customer, editing, credit, chargeLines = []) => {
    const items: CartLine[] = lines.map((l) => ({ ...l, lineId: nextLineId() }));
    const charges: BillChargeLine[] = chargeLines.map((c) => ({ ...c, key: ++chargeSeq }));
    set({
      ...initialState,
      items,
      charges,
      selectedCustomer: customer,
      editing,
      editCredit: credit,
      ...totalsFor(items, charges),
    });
  },
}));
