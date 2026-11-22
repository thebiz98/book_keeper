/**
 * Pre-fill hand-off for New Purchase (dashboard "Inward (F3)" action).
 * The router (App.tsx ROUTES) renders pages without props, so the caller queues the item
 * here and then navigates:
 *
 *   import { queuePurchasePrefill } from '@/pages/purchasePrefill';
 *   queuePurchasePrefill({ itemId: item.id, name: item.name });
 *   setActiveRoute('purchase');
 *
 * NewPurchasePage consumes the queued value on mount. A `?item=<id|name>` query (in
 * `location.search` or after `#...?`) and the `initialItemId` / `initialItemName` props
 * are also honoured.
 */
export interface PurchasePrefill {
  itemId?: number;
  name?: string;
}

let pending: PurchasePrefill | null = null;

export function queuePurchasePrefill(p: PurchasePrefill): void {
  pending = p;
}

export function takePurchasePrefill(): PurchasePrefill | null {
  const p = pending;
  pending = null;
  return p;
}

/** Parse `?item=` from the current location (search or hash query). */
export function prefillFromLocation(loc: Pick<Location, 'search' | 'hash'> = window.location): PurchasePrefill | null {
  const hashQuery = loc.hash.includes('?') ? loc.hash.slice(loc.hash.indexOf('?')) : '';
  for (const q of [loc.search, hashQuery]) {
    const v = new URLSearchParams(q).get('item');
    if (v && v.trim()) {
      const n = Number(v);
      return Number.isInteger(n) && n > 0 ? { itemId: n } : { name: v.trim() };
    }
  }
  return null;
}
