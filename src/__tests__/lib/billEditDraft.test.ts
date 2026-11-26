import { describe, it, expect } from 'vitest';
import { lineBundle, purchaseLineToDraft, saleLineToCart } from '@/lib/billEditDraft';
import type { ItemSearchResult, PurchaseItem, SaleItem } from '@/types';

const item = (bundles: ItemSearchResult['bundles'] = []): ItemSearchResult => ({
  itemId: 1, name: 'Pen', category: null, barcode: 'B1', baseUnit: 'PCS', itemType: 'PHYSICAL', mrp: 12,
  defaultSalePrice: 10, currentStock: 5, isLowStock: false, lowStockThreshold: 5, bundles, matchedBundleId: null,
});
const box = { bundleId: 7, bundleName: 'Box of 10', unitsPerBundle: 10, bundlePrice: null, defaultRate: 0, barcode: null };

const saleLine = (over: Partial<SaleItem> = {}): SaleItem => ({
  id: 1, saleTransactionId: 1, itemId: 1, itemName: 'Pen', packLabel: 'Box of 10', packQuantity: 3, quantity: 30,
  unitPrice: 9, totalAmount: 270, ...over,
});

describe('billEditDraft', () => {
  it('uses the bundle the backend named for the line and rebuilds the rate', () => {
    expect(lineBundle(7, item([box]))?.bundleId).toBe(7);
    const c = saleLineToCart(saleLine(), item([box]), 7);
    expect(c).toMatchObject({ bundleId: 7, bundleName: 'Box of 10', quantity: 3, unitsPerBundle: 10, rate: 90 });
  });

  it('E4: a renamed bundle or item keeps the pack, because the match is by id, not by name', () => {
    const renamed = { ...box, bundleName: 'Carton' };
    const c = saleLineToCart(saleLine({ itemName: 'Old name' }), { ...item([renamed]), name: 'New name' }, 7);
    expect(c).toMatchObject({ bundleId: 7, quantity: 3, unitsPerBundle: 10, rate: 90 });
  });

  it('falls back to base units when the backend named no bundle or the item is unknown', () => {
    for (const [i, id] of [[item([box]), null], [item([]), 7], [null, 7]] as const) {
      const c = saleLineToCart(saleLine(), i, id);
      expect(c).toMatchObject({ bundleId: undefined, quantity: 30, unitsPerBundle: 1, rate: 9 });
    }
  });

  it('purchase drafts keep the lot id and bundle', () => {
    const lot: PurchaseItem = {
      id: 42, purchaseTransactionId: 1, itemId: 1, itemName: 'Pen', packLabel: 'Box of 10', packQuantity: 2,
      quantity: 20, unitCost: 4, totalAmount: 80, remainingQuantity: 15,
    };
    expect(purchaseLineToDraft(lot, item([box]), 7)).toMatchObject({ purchaseItemId: 42, bundleId: 7, quantity: 2, unitCost: 40, unitsPerBundle: 10 });
    expect(purchaseLineToDraft(lot, null, 7)).toMatchObject({ purchaseItemId: 42, bundleId: null, quantity: 20, unitCost: 4, packLabel: 'BASE' });
  });
});
