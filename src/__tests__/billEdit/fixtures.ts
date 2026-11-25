import type { BillEditGuard, ItemSearchResult, PurchaseDetailView } from '@/types';

export const purchaseDetail = (): PurchaseDetailView =>
  ({
    purchase: {
      id: 8, transactionCode: 'PUR-1008', source: 'PURCHASE', supplierInvoiceRef: 'INV-1', transactionDate: '2026-11-20',
      partyId: null, partyName: null, paymentMode: 'CASH', paymentStatus: 'PAID', totalAmount: 100, paidAmount: 100,
      balanceDue: 0, notes: null, createdAt: '', itemsSubtotal: 100, chargesTotal: 0, discountsTotal: 0, updatedAt: null,
    },
    party: null, settlements: [], charges: [],
    items: [
      {
        purchaseItem: { id: 21, purchaseTransactionId: 8, itemId: 1, itemName: 'Pen', packLabel: null, packQuantity: 10, quantity: 10, unitCost: 10, totalAmount: 100, remainingQuantity: 6 },
        itemName: 'Pen', bundleName: null,
      },
      {
        purchaseItem: { id: 22, purchaseTransactionId: 8, itemId: 2, itemName: 'Ink', packLabel: null, packQuantity: 5, quantity: 5, unitCost: 20, totalAmount: 100, remainingQuantity: 5 },
        itemName: 'Ink', bundleName: null,
      },
    ],
  }) as unknown as PurchaseDetailView;

export const purchaseGuard = (over: Partial<BillEditGuard> = {}): BillEditGuard => ({
  target: 'PURCHASE', id: 8, code: 'PUR-1008', canEdit: true, canDelete: false, canChangeParty: true, reasons: [],
  upfrontAmount: 200, appliedAmount: 0, removedCashAmount: 200, removedCreditAmount: 0,
  lotLimits: [
    { purchaseItemId: 21, consumedQuantity: 4, referenced: true, vendorReturned: true },
    { purchaseItemId: 22, consumedQuantity: 0, referenced: false, vendorReturned: false },
  ],
  ...over,
});

export const PEN: ItemSearchResult = {
  itemId: 1, name: 'Pen', category: null, barcode: null, baseUnit: 'PCS', itemType: 'PHYSICAL', mrp: null,
  defaultSalePrice: 12, currentStock: 6, isLowStock: false, lowStockThreshold: 0, matchedBundleId: null,
  bundles: [{ bundleId: 5, bundleName: 'Box', unitsPerBundle: 10, bundlePrice: 100, defaultRate: 100, barcode: null }],
};
export const INK: ItemSearchResult = { ...PEN, itemId: 2, name: 'Ink', bundles: [] };

export const search = async (q: string) => (q === 'Pen' ? [PEN] : q === 'Ink' ? [INK] : []);
