import type { BillEditGuard, ItemSearchResult, PurchaseDetailView, SaleDetailView } from '@/types';

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
  settlementsToMove: { count: 0, amount: 0 },
  ...over,
});

export const PEN: ItemSearchResult = {
  itemId: 1, name: 'Pen', category: null, barcode: null, baseUnit: 'PCS', itemType: 'PHYSICAL', mrp: null,
  defaultSalePrice: 12, currentStock: 6, isLowStock: false, lowStockThreshold: 0, matchedBundleId: null,
  bundles: [{ bundleId: 5, bundleName: 'Box', unitsPerBundle: 10, bundlePrice: 100, defaultRate: 100, barcode: null }],
};
export const INK: ItemSearchResult = { ...PEN, itemId: 2, name: 'Ink', bundles: [] };

export const search = async (q: string) => (q === 'Pen' ? [PEN] : q === 'Ink' ? [INK] : []);

/** E4: the edit drafts load each line's item by id (never a name search). */
export const getItem = async (id: number): Promise<ItemSearchResult> => {
  if (id === PEN.itemId) return PEN;
  if (id === INK.itemId) return INK;
  throw new Error(`no item ${id}`);
};

export const saleDetail = (): SaleDetailView =>
  ({
    sale: {
      id: 5, transactionCode: 'SAL-1005', transactionDate: '2026-11-20', partyId: null, partyName: null, paymentMode: 'CASH',
      paymentStatus: 'PAID', totalAmount: 100, paidAmount: 100, balanceDue: 0, notes: 'hi', createdAt: '', itemsSubtotal: 100,
      chargesTotal: 0, discountsTotal: 0, updatedAt: '2026-11-21T10:00:00Z',
    },
    party: null, settlements: [], charges: [],
    items: [
      {
        saleItem: { id: 1, saleTransactionId: 5, itemId: 1, itemName: 'Pen', packLabel: null, packQuantity: 2, quantity: 2, unitPrice: 50, totalAmount: 100 },
        itemName: 'Pen', bundleName: null, returnedQuantity: 0, allocations: [], netUnitPrice: 50,
      },
      {
        saleItem: { id: 2, saleTransactionId: 5, itemId: 2, itemName: 'Ink', packLabel: null, packQuantity: 1, quantity: 1, unitPrice: 30, totalAmount: 30 },
        itemName: 'Ink', bundleName: null, returnedQuantity: 0, allocations: [], netUnitPrice: 30,
      },
    ],
  }) as unknown as SaleDetailView;

export const saleGuard = (over: Partial<BillEditGuard> = {}): BillEditGuard => ({
  target: 'SALE', id: 5, code: 'SAL-1005', canEdit: true, canDelete: true, canChangeParty: true, reasons: [],
  upfrontAmount: 130, appliedAmount: 0, removedCashAmount: 130, removedCreditAmount: 0, lotLimits: [], settlementsToMove: { count: 0, amount: 0 }, ...over,
});
