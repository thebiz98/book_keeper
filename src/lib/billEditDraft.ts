/**
 * Pure helpers that turn a stored bill into entry-screen drafts for edit mode (FR-20).
 * A stored line keeps only base-unit quantity, a pack snapshot and the line total, so the
 * original pack and rate are reconstructed: a pack is reused when the item still has a bundle with
 * the same name and multiplier, otherwise the line falls back to base units. Line totals are
 * preserved either way.
 */
import type { ItemBundleSearchResult, ItemSearchResult, PurchaseItem, SaleItem } from '@/types';
import type { CartLine } from '@/store/useCartStore';
import type { PurchaseLine } from '@/components/domain/purchaseMath';
import { round2 } from '@/lib/money';

type StoredLine = Pick<SaleItem, 'itemId' | 'itemName' | 'packLabel' | 'packQuantity' | 'quantity' | 'totalAmount'>;

const MULT_EPS = 1e-6;

/** The item's bundle that reproduces the stored pack snapshot, or null (base units). */
export function matchBundle(line: StoredLine, item: ItemSearchResult | null): ItemBundleSearchResult | null {
  if (!item || !line.packLabel || line.packQuantity <= 0) return null;
  const multiplier = line.quantity / line.packQuantity;
  return (
    item.bundles.find((b) => b.bundleName === line.packLabel && Math.abs(b.unitsPerBundle - multiplier) < MULT_EPS) ??
    null
  );
}

/** Quantity, multiplier and rate to show for a stored line. */
function unitView(line: StoredLine, item: ItemSearchResult | null) {
  const bundle = matchBundle(line, item);
  if (bundle) {
    return {
      bundle,
      quantity: line.packQuantity,
      unitsPerBundle: bundle.unitsPerBundle,
      rate: line.totalAmount / line.packQuantity,
    };
  }
  return { bundle: null, quantity: line.quantity, unitsPerBundle: 1, rate: line.totalAmount / line.quantity };
}

/** A sale line as a cart line. `availableStock` is current stock plus the bill's own units. */
export function saleLineToCart(
  line: SaleItem,
  item: ItemSearchResult | null,
  availableStock: number
): Omit<CartLine, 'lineId'> {
  const v = unitView(line, item);
  return {
    itemId: line.itemId,
    name: line.itemName,
    barcode: item?.barcode ?? null,
    baseUnit: item?.baseUnit ?? 'PCS',
    unitsPerBundle: v.unitsPerBundle,
    bundleId: v.bundle?.bundleId,
    bundleName: v.bundle?.bundleName,
    quantity: v.quantity,
    baseQuantity: line.quantity,
    rate: v.rate,
    mrp: item?.mrp ?? null,
    lineTotal: round2(v.quantity * v.rate),
    availableStock,
    defaultSalePrice: item?.defaultSalePrice,
  };
}

/** A purchase lot as a purchase draft line; keeps the lot id so the edit updates it in place. */
export function purchaseLineToDraft(lot: PurchaseItem, item: ItemSearchResult | null): Omit<PurchaseLine, 'key'> {
  const v = unitView(
    {
      itemId: lot.itemId,
      itemName: lot.itemName,
      packLabel: lot.packLabel,
      packQuantity: lot.packQuantity,
      quantity: lot.quantity,
      totalAmount: lot.totalAmount,
    },
    item
  );
  return {
    itemId: lot.itemId,
    itemName: lot.itemName,
    baseUnit: item?.baseUnit ?? 'PCS',
    bundleId: v.bundle?.bundleId ?? null,
    packLabel: v.bundle ? v.bundle.bundleName : 'BASE',
    unitsPerBundle: v.unitsPerBundle,
    quantity: v.quantity,
    unitCost: v.rate,
    purchaseItemId: lot.id,
  };
}

/** Base units per item that a bill already holds (they are available again once it is reversed). */
export function editStockCredit(lines: { itemId: number; quantity: number }[]): Record<number, number> {
  const credit: Record<number, number> = {};
  for (const l of lines) credit[l.itemId] = (credit[l.itemId] ?? 0) + l.quantity;
  return credit;
}
