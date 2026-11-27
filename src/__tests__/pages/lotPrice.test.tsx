import { describe, it, expect, beforeEach, vi } from 'vitest';
import { render } from '../testUtils';
import { LotPriceEditModal } from '@/components/domain/LotPriceEditModal';
import type { ItemLot, PaymentSettlement, PurchaseDetailView } from '@/types';
import { installApi, flush, waitFor, click, setInput, buttonByText, bodyText } from './helpers';

const lot = (over: Partial<ItemLot> = {}): ItemLot => ({
  purchaseItemId: 11, purchaseTransactionId: 5, transactionCode: 'PUR-1005', source: 'PURCHASE',
  transactionDate: '2026-11-01', vendorName: 'V', packLabel: null, packQuantity: 10, quantity: 10,
  remainingQuantity: 10, unitCost: 10, ...over,
});

const settlement = (id: number, amount: number): PaymentSettlement => ({
  id, settlementDate: '2026-11-02', partyId: 1, kind: 'SETTLEMENT', transactionType: 'PURCHASE', transactionId: 5,
  returnId: null, amountPaid: amount, paymentMode: 'CASH', notes: null, createdAt: '2026-11-02T10:00:00',
});

function detail(over: { paid: number; balance: number; status: 'PAID' | 'PARTIAL' | 'UNPAID'; settlements?: PaymentSettlement[] }): PurchaseDetailView {
  return {
    purchase: {
      id: 5, transactionCode: 'PUR-1005', source: 'PURCHASE', supplierInvoiceRef: null, transactionDate: '2026-11-01',
      partyId: 1, partyName: 'V', paymentMode: 'CASH', paymentStatus: over.status, totalAmount: 100,
      paidAmount: over.paid, balanceDue: over.balance, notes: null, createdAt: '2026-11-01T10:00:00',
      itemsSubtotal: 100, chargesTotal: 0, discountsTotal: 0, updatedAt: null,
    },
    party: null,
    items: [
      {
        purchaseItem: {
          id: 11, purchaseTransactionId: 5, itemId: 1, itemName: 'Pen', packLabel: null, packQuantity: 10,
          quantity: 10, unitCost: 10, totalAmount: 100, remainingQuantity: 10,
        },
        itemName: 'Pen',
        bundleName: null,
      },
    ],
    settlements: over.settlements ?? [],
    charges: [],
  };
}


const priceInput = () => document.querySelector('input[aria-label="Purchase price"]') as HTMLInputElement;

describe('LotPriceEditModal', () => {
  beforeEach(() => {
    document.body.innerHTML = '';
  });

  it('previews the bill effect and saves the new price', async () => {
    // DD-56: the bill effect is whatever the backend preview returns (canned here).
    const api = installApi({
      getPurchaseDetails: detail({ paid: 100, balance: 0, status: 'PAID' }),
      previewLotPurchasePrice: async (p: { unitPrice: number }) => ({
        purchaseItemId: 11,
        oldUnitCost: 10,
        newUnitCost: p.unitPrice,
        bill: {
          purchaseId: 5, transactionCode: 'PUR-1005', oldTotal: 100, newTotal: 120, oldPaid: 100, newPaid: 120,
          balanceDue: 0, paymentStatus: 'PAID',
        },
      }),
      updateLotPurchasePrice: { purchaseItemId: 11, oldUnitCost: 10, newUnitCost: 12, bill: null },
    });
    const onSaved = vi.fn();
    render(<LotPriceEditModal lot={lot()} baseUnit="PCS" onClose={() => {}} onSaved={onSaved} />);
    await waitFor(() => bodyText().includes('Bill PUR-1005'));
    expect(bodyText()).toContain("sale bills don't change");
    setInput(priceInput(), 12);
    await waitFor(() => (document.querySelector('[data-testid="lot-price-preview"]')?.textContent ?? '').includes('₹120.00'));
    expect(api.previewLotPurchasePrice).toHaveBeenLastCalledWith({ purchaseItemId: 11, unitPrice: 12 });
    await waitFor(() => !buttonByText('Save').disabled);
    click(buttonByText('Save'));
    await flush();
    expect(api.updateLotPurchasePrice).toHaveBeenCalledWith({ purchaseItemId: 11, unitPrice: 12 });
    expect(onSaved).toHaveBeenCalled();
  });

  it('lists the bill settlements with Edit when the new total is below what was applied', async () => {
    let d = detail({ paid: 80, balance: 20, status: 'PARTIAL', settlements: [settlement(31, 80)] });
    let blocked = true; // the backend refuses until the payments are corrected
    const api = installApi({
      getPurchaseDetails: async () => d,
      previewLotPurchasePrice: async (p: { unitPrice: number }) => {
        if (blocked) {
          throw {
            type: 'BillLocked',
            details: { code: 'TOTAL_BELOW_APPLIED', message: 'The new total ₹50.00 is below the ₹80.00 already applied to this bill' },
          };
        }
        return { purchaseItemId: 11, oldUnitCost: 10, newUnitCost: p.unitPrice, bill: null };
      },
      updateSettlement: settlement(31, 50),
    });
    render(<LotPriceEditModal lot={lot()} baseUnit="PCS" onClose={() => {}} onSaved={() => {}} />);
    await waitFor(() => bodyText().includes('Bill PUR-1005'));
    setInput(priceInput(), 5);
    await waitFor(() => !!document.querySelector('[data-testid="lot-price-blocked"]'));
    expect(bodyText()).toContain('is below the ₹80.00 already applied');
    expect(document.querySelectorAll('[data-testid="blocked-settlement"]').length).toBe(1);
    expect(buttonByText('Save').disabled).toBe(true);
    d = detail({ paid: 50, balance: 50, status: 'PARTIAL', settlements: [settlement(31, 50)] });
    blocked = false;
    click(document.querySelector('button[aria-label="Edit payment 31"]') as Element);
    await waitFor(() => !!document.querySelector('[data-testid="settlement-edit"]'));
    const amt = document.querySelector('input[aria-label="Settlement amount"]') as HTMLInputElement;
    setInput(amt, 50);
    click(buttonByText('Save', document.querySelector('[data-testid="settlement-edit"]') as Element));
    await flush();
    expect(api.updateSettlement).toHaveBeenCalledWith(31, expect.objectContaining({ amountPaid: 50 }));
    await waitFor(() => !document.querySelector('[data-testid="lot-price-blocked"]'));
    expect(buttonByText('Save').disabled).toBe(false);
  });

  it('edits an opening lot without any bill preview', async () => {
    const api = installApi({
      previewLotPurchasePrice: async (p: { unitPrice: number }) => ({ purchaseItemId: 11, oldUnitCost: 10, newUnitCost: p.unitPrice, bill: null }),
      updateLotPurchasePrice: { purchaseItemId: 11, oldUnitCost: 10, newUnitCost: 7, bill: null },
    });
    render(<LotPriceEditModal lot={lot({ source: 'OPENING' })} baseUnit="PCS" onClose={() => {}} onSaved={() => {}} />);
    await flush();
    expect(document.querySelector('[data-testid="lot-price-preview"]')).toBeNull();
    setInput(priceInput(), 7);
    await waitFor(() => !buttonByText('Save').disabled);
    click(buttonByText('Save'));
    await flush();
    expect(api.updateLotPurchasePrice).toHaveBeenCalledWith({ purchaseItemId: 11, unitPrice: 7 });
  });

  it('asks for the price per pack when the lot has a pack label', async () => {
    installApi({ previewLotPurchasePrice: async (p: { unitPrice: number }) => ({ purchaseItemId: 11, oldUnitCost: 10, newUnitCost: p.unitPrice, bill: null }) });
    render(
      <LotPriceEditModal lot={lot({ source: 'OPENING', packLabel: 'Box of 5', packQuantity: 2, quantity: 10 })} baseUnit="PCS" onClose={() => {}} onSaved={() => {}} />
    );
    await flush();
    expect(bodyText()).toContain('Purchase price per Box of 5');
    expect(priceInput().value).toBe('50');
  });

  it('says past COGS and losses change only when the backend reports a recost of history (S11)', async () => {
    let recosts = false;
    installApi({
      previewLotPurchasePrice: async (p: { unitPrice: number }) => ({
        purchaseItemId: 11, oldUnitCost: 10, newUnitCost: p.unitPrice, bill: null, recostsHistory: recosts,
      }),
    });
    render(<LotPriceEditModal lot={lot({ source: 'OPENING' })} baseUnit="PCS" onClose={() => {}} onSaved={() => {}} />);
    await flush();
    setInput(priceInput(), 7);
    await waitFor(() => !buttonByText('Save').disabled);
    expect(document.querySelector('[data-testid="lot-price-recost-note"]')).toBeNull();
    recosts = true;
    setInput(priceInput(), 8);
    await waitFor(() => document.querySelector('[data-testid="lot-price-recost-note"]') !== null);
    expect(bodyText()).toContain('past COGS and inventory losses');
  });
});
