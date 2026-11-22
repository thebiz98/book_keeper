import { round2 } from '@/lib/money';
import type { PaymentStatus } from '@/types';

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
}

export const lineBaseQuantity = (l: Pick<PurchaseLine, 'quantity' | 'unitsPerBundle'>) =>
  round6(l.quantity * l.unitsPerBundle);

export const lineCostPerBase = (l: Pick<PurchaseLine, 'unitCost' | 'unitsPerBundle'>) =>
  l.unitCost / l.unitsPerBundle;

export const lineTotal = (l: Pick<PurchaseLine, 'quantity' | 'unitCost'>) =>
  round2(l.quantity * l.unitCost);

export const purchaseTotal = (lines: PurchaseLine[]) =>
  round2(lines.reduce((acc, l) => acc + lineTotal(l), 0));

/** Mirrors the backend cross-check (DD-21). Returns an error message or null. */
export function validatePayment(
  status: PaymentStatus,
  total: number,
  paid: number,
  vendorId: number | null
): string | null {
  const t = round2(total);
  const p = round2(paid);
  if (!Number.isFinite(p) || p < 0) return 'Paid amount cannot be negative';
  if (p > t) return 'Paid amount cannot exceed the purchase total';
  if ((status === 'PARTIAL' || status === 'UNPAID') && vendorId === null) {
    return 'Select a vendor for partial or unpaid purchases';
  }
  if (status === 'PAID' && p !== t) return 'PAID requires paid amount equal to the total';
  if (status === 'PARTIAL' && !(p > 0 && p < t)) {
    return 'PARTIAL requires a paid amount greater than 0 and less than the total';
  }
  if (status === 'UNPAID' && p !== 0) return 'UNPAID requires a paid amount of 0';
  return null;
}

/** Paid amount actually sent for a status (PAID = total, UNPAID = 0, PARTIAL = entered). */
export function effectivePaid(status: PaymentStatus, total: number, entered: number): number {
  if (status === 'PAID') return round2(total);
  if (status === 'UNPAID') return 0;
  return round2(entered);
}
