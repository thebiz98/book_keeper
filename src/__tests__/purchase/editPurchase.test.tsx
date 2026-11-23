import { describe, it, expect, beforeEach, vi } from 'vitest';
import { render } from '../testUtils';
import { installApi, flush, waitFor, click, setInput, bodyText } from '../pages/helpers';
import { queueBillEdit, takeBillEdit } from '@/pages/billEditHandoff';
import type { BillEditGuard, PurchaseDetailView } from '@/types';

vi.mock('@/components/domain/ItemSearchTypeahead', () => ({ ItemSearchTypeahead: () => <input data-testid="stub-search" /> }));
vi.mock('@/components/domain/PartySelector', () => ({ PartySelector: () => <div data-testid="stub-party" /> }));

import { NewPurchasePage } from '@/pages/NewPurchasePage';

const detail = (): PurchaseDetailView =>
  ({
    purchase: {
      id: 8, transactionCode: 'PUR-1008', source: 'PURCHASE', supplierInvoiceRef: 'INV-1', transactionDate: '2026-11-20', partyId: null,
      partyName: null, paymentMode: 'CASH', paymentStatus: 'PAID', totalAmount: 100, paidAmount: 100, balanceDue: 0, notes: null,
      createdAt: '', itemsSubtotal: 100, chargesTotal: 0, discountsTotal: 0, updatedAt: null,
    },
    party: null, settlements: [], charges: [],
    items: [
      { purchaseItem: { id: 21, purchaseTransactionId: 8, itemId: 1, itemName: 'Pen', packLabel: null, packQuantity: 10, quantity: 10, unitCost: 10, totalAmount: 100, remainingQuantity: 6 }, itemName: 'Pen', bundleName: null },
    ],
  }) as unknown as PurchaseDetailView;

const guard = (over: Partial<BillEditGuard> = {}): BillEditGuard => ({
  target: 'PURCHASE', id: 8, code: 'PUR-1008', canEdit: true, canDelete: false, canChangeParty: true, reasons: [],
  upfrontAmount: 100, appliedAmount: 0, removedCashAmount: 100, removedCreditAmount: 0,
  lotLimits: [{ purchaseItemId: 21, consumedQuantity: 4, referenced: true, vendorReturned: false }], ...over,
});

function mount(g: BillEditGuard = guard()) {
  const api = installApi({
    getPurchaseDetails: detail(),
    getBillEditGuard: g,
    searchItemsFts: [],
    getNextPurchaseCode: 'PUR-9999',
    updatePurchase: { purchaseId: 8, transactionCode: 'PUR-1008' },
  });
  queueBillEdit({ kind: 'PURCHASE', id: 8 });
  render(<NewPurchasePage />);
  return api;
}

describe('NewPurchasePage edit mode (FR-20)', () => {
  beforeEach(() => {
    document.body.innerHTML = '';
    takeBillEdit();
  });

  it('prefills lines with the lot limits, locks the line and sends purchaseItemId via updatePurchase', async () => {
    const api = mount();
    await waitFor(() => !!document.querySelector('[data-testid="edit-banner"]'));
    expect(bodyText()).toContain('Editing PUR-1008');
    expect(document.querySelector('[data-testid="code-preview"]')?.textContent).toBe('PUR-1008');
    expect(document.querySelector('[data-testid="used-hint"]')?.textContent).toContain('Used: 4');
    expect((document.querySelector('button[aria-label="Remove Pen"]') as HTMLButtonElement).disabled).toBe(true);
    click(Array.from(document.querySelectorAll('button')).find((b) => b.textContent?.includes('Update Purchase'))!);
    await flush();
    expect(api.updatePurchase).toHaveBeenCalledTimes(1);
    const [id, payload] = api.updatePurchase.mock.calls[0];
    expect(id).toBe(8);
    expect(payload.items).toEqual([{ itemId: 1, bundleId: null, bundleQuantity: 10, unitCost: 10, purchaseItemId: 21 }]);
    expect(api.createPurchase).toBeUndefined();
  });

  it('rejects a quantity below what was already used', async () => {
    const api = mount();
    await waitFor(() => !!document.querySelector('[data-testid="edit-banner"]'));
    setInput(document.querySelector('input[aria-label="Quantity Pen"]')!, '3');
    click(Array.from(document.querySelectorAll('button')).find((b) => b.textContent?.includes('Update Purchase'))!);
    await flush();
    expect(bodyText()).toContain('at least 4 already used');
    expect(api.updatePurchase).not.toHaveBeenCalled();
  });

  it('makes the cost read-only on a vendor-returned lot', async () => {
    mount(guard({ lotLimits: [{ purchaseItemId: 21, consumedQuantity: 2, referenced: true, vendorReturned: true }] }));
    await waitFor(() => !!document.querySelector('[data-testid="edit-banner"]'));
    expect((document.querySelector('input[aria-label="Unit cost Pen"]') as HTMLInputElement).disabled).toBe(true);
  });
});
