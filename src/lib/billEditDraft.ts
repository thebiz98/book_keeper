/**
 * Pure helpers that turn a stored bill into entry-screen drafts for edit mode (FR-20).
 * A stored line keeps only base-unit quantity, a pack snapshot and the line total. The backend
 * names the line's bundle (`bundleId` on the bill detail, E4 / DD-89) and the item is loaded by id
 * (`getItem`, deleted items included), so the line shows in its pack whenever that bundle still
 * exists, else in base units. Line totals are preserved either way.
 */
import type { ItemBundleSearchResult, ItemSearchResult, PurchaseItem, SaleItem, SavedBundle } from '@/types';
import type { CartLine } from '@/store/useCartStore';
import type { PurchaseLine } from '@/components/domain/purchaseMath';

type StoredLine = Pick<SaleItem, 'itemId' | 'itemName' | 'packLabel' | 'packQuantity' | 'quantity' | 'totalAmount'>;

/** The item's bundle the backend resolved for the line (by id), or null (base units). */
export function lineBundle(bundleId: number | null | undefined, item: ItemSearchResult | null): ItemBundleSearchResult | null {
  if (bundleId == null || !item) return null;
  return item.bundles.find((b) => b.bundleId === bundleId) ?? null;
}

/** Quantity, multiplier and rate to show for a stored line. */
function unitView(line: StoredLine, item: ItemSearchResult | null, bundleId: number | null | undefined) {
  const bundle = line.packQuantity > 0 ? lineBundle(bundleId, item) : null;
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

/** A sale line as a cart line (the form row only; totals and stock come from the preview). */
export function saleLineToCart(
  line: SaleItem,
  item: ItemSearchResult | null,
  bundleId: number | null | undefined
): Omit<CartLine, 'lineId'> {
  const v = unitView(line, item, bundleId);
  return {
    itemId: line.itemId,
    name: line.itemName,
    barcode: item?.barcode ?? null,
    baseUnit: item?.baseUnit ?? 'PCS',
    unitsPerBundle: v.unitsPerBundle,
    bundleId: v.bundle?.bundleId,
    bundleName: v.bundle?.bundleName,
    quantity: v.quantity,
    rate: v.rate,
    mrp: item?.mrp ?? null,
    defaultSalePrice: item?.defaultSalePrice,
  };
}

/** A purchase lot as a purchase draft line; keeps the lot id so the edit updates it in place. */
export function purchaseLineToDraft(
  lot: PurchaseItem,
  item: ItemSearchResult | null,
  bundleId: number | null | undefined
): Omit<PurchaseLine, 'key'> {
  const v = unitView(
    {
      itemId: lot.itemId,
      itemName: lot.itemName,
      packLabel: lot.packLabel,
      packQuantity: lot.packQuantity,
      quantity: lot.quantity,
      totalAmount: lot.totalAmount,
    },
    item,
    bundleId
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


/** Row mapping only: a bundle just saved by `BundleCreationModal` as an entry of a line's unit list. */
export function savedBundleToSearchResult(b: SavedBundle): ItemBundleSearchResult {
  return {
    bundleId: b.id,
    bundleName: b.bundleName,
    unitsPerBundle: b.unitsPerBundle,
    bundlePrice: b.defaultBundlePrice,
    defaultRate: b.defaultRate,
    barcode: b.barcode,
  };
}
