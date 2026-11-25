/**
 * The purchase entry line and display-only unit conversions. Line totals, bill totals and the
 * payment rules are the backend's (`preview_purchase`, DD-56); nothing here decides money.
 */

/** Float noise guard for base-unit quantities (decimals allowed). */
export function round6(n: number): number {
  return Math.round((n + Number.EPSILON) * 1e6) / 1e6;
}

export interface PurchaseLine {
  key: number;
  itemId: number;
  itemName: string;
  baseUnit: string;
  bundleId: number | null;
  /** "BASE" label uses the base unit; otherwise the bundle name. */
  packLabel: string;
  unitsPerBundle: number;
  /** Quantity in the selected unit (bundles or base units). */
  quantity: number;
  /** Cost per selected unit (per bundle, or per base unit). */
  unitCost: number;
  /** Edit mode (FR-20): the existing lot this line keeps; absent/null = a new line. */
  purchaseItemId?: number | null;
  /** Edit mode: base units already used from the lot (shown as a hint; the backend enforces it). */
  minBaseQty?: number;
  /** Edit mode: the lot is referenced, so its item cannot change and the line cannot be removed. */
  locked?: boolean;
  /** Edit mode: a vendor return used the lot, so the cost is read-only. */
  costLocked?: boolean;
}

/** Display: the line's quantity in base units. */
export const lineBaseQuantity = (l: Pick<PurchaseLine, 'quantity' | 'unitsPerBundle'>) =>
  round6(l.quantity * l.unitsPerBundle);

/** Display: the line's cost per base unit. */
export const lineCostPerBase = (l: Pick<PurchaseLine, 'unitCost' | 'unitsPerBundle'>) =>
  l.unitCost / l.unitsPerBundle;
