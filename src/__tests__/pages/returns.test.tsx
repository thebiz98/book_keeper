import { describe, it, expect, beforeEach, vi } from 'vitest';
import { render } from '../testUtils';
import { ReturnFormCustomer } from '@/components/domain/ReturnFormCustomer';
import { ReturnFormVendor } from '@/components/domain/ReturnFormVendor';
import { installApi, flush, waitFor, click, setInput, buttonByText, bodyText } from './helpers';
import type { ItemSearchResult, SaleDetailView, PurchaseDetailView } from '@/types';

// Decouple from the shared typeahead/selector: a stub that picks a fixed item.
const ITEM: ItemSearchResult = {
  itemId: 9,
  name: 'Gel Pen',
  category: null,
  barcode: null,
  baseUnit: 'PCS', itemType: 'PHYSICAL',
  mrp: null,
  defaultSalePrice: 10,
  currentStock: 5,
  isLowStock: false, lowStockThreshold: 5,
  bundles: [],
  matchedBundleId: null,
};
vi.mock('@/components/domain/ItemSearchTypeahead', () => ({
  ItemSearchTypeahead: (p: { onSelectItem: (i: ItemSearchResult) => void; placeholder?: string }) => (
    <button type="button" data-testid="pick-item" onClick={() => p.onSelectItem(ITEM)}>
      {p.placeholder ?? 'pick'}
    </button>
  ),
}));
vi.mock('@/components/domain/PartySelector', () => ({
  PartySelector: () => <span>party-selector</span>,
}));

const saleDetail = (netUnitPrice = 50): SaleDetailView =>
  ({
    sale: { id: 5, transactionCode: 'SAL-5', transactionDate: '2026-01-01', partyId: 3, partyName: 'Ram', balanceDue: 100 },
    party: null,
    settlements: [],
    items: [
      {
        saleItem: { id: 11, saleTransactionId: 5, itemId: 2, itemName: 'Notebook', packLabel: null, packQuantity: 10, quantity: 10, unitPrice: 50, totalAmount: 500 },
        itemName: 'Notebook',
        bundleName: null,
        returnedQuantity: 4,
        allocations: [],
        netUnitPrice,
      },
    ],
  }) as unknown as SaleDetailView;

const purchaseDetail = (): PurchaseDetailView =>
  ({
    purchase: { id: 8, transactionCode: 'PUR-8', transactionDate: '2026-01-01', partyId: 4, partyName: 'Vendor', balanceDue: 40 },
    party: null,
    settlements: [],
    items: [
      {
        purchaseItem: { id: 21, purchaseTransactionId: 8, itemId: 2, itemName: 'Notebook', packLabel: null, packQuantity: 10, quantity: 10, unitCost: 30, totalAmount: 300, remainingQuantity: 6 },
        itemName: 'Notebook',
        bundleName: null,
      },
    ],
  }) as unknown as PurchaseDetailView;

const result = { returnId: 1, returnCode: 'RET-1', creditedAmount: 100, cashRefunded: 50, totalAmount: 150, issuedValue: 0 };

/** Lets the debounced preview come back. */
const settle = async () => {
  for (let i = 0; i < 3; i++) await flush(120);
};

// DD-56: values, the credited/cash split and every refusal are the backend's preview. These tests
// assert that the forms send the right preview input and show what comes back.
describe('ReturnFormCustomer', () => {
  beforeEach(() => {
    document.body.innerHTML = '';
  });

  it('sends a linked payload, shows the credited/cash preview and the result', async () => {
    const api = installApi({
      getSaleDetails: async () => saleDetail(),
      previewCustomerReturn: async () => result,
      createCustomerReturn: async () => result,
    });
    render(<ReturnFormCustomer initialBillId={5} />);
    await waitFor(() => bodyText().includes('Notebook'));
    setInput(document.querySelector('input[aria-label="Return quantity for Notebook"]')!, '3');
    await settle();
    const preview = document.querySelector('[data-testid="refund-preview"]')!.textContent ?? '';
    expect(preview).toContain('Preview');
    expect(preview).toContain('₹100.00 credited');
    expect(preview).toContain('₹50.00 cash');
    expect(preview).toContain('₹150.00'); // the preview's total value
    expect(api.previewCustomerReturn.mock.calls.at(-1)![0]).toMatchObject({ originalSaleId: 5, items: [{ quantity: 3 }] });
    click(buttonByText('Record Return'));
    await flush();
    expect(api.createCustomerReturn.mock.calls[0][0]).toEqual({
      originalSaleId: 5,
      partyId: null,
      returnDate: expect.stringMatching(/^\d{4}-\d{2}-\d{2}$/),
      settlementType: 'REFUND',
      reason: null,
      notes: null,
      items: [{ itemId: 2, saleItemId: 11, quantity: 3, unitPrice: 50, condition: 'RESTOCKABLE', unitCostOverride: null }],
      replacementItems: [],
    });
    expect(document.querySelector('[data-testid="return-result"]')!.textContent).toContain('RET-1');
  });

  it('defaults the refund rate to the net rate and says it is net of the bill discount (FR-19)', async () => {
    installApi({ getSaleDetails: async () => saleDetail(45), createCustomerReturn: async () => result });
    render(<ReturnFormCustomer initialBillId={5} />);
    await waitFor(() => bodyText().includes('Notebook'));
    const rate = document.querySelector('input[aria-label="Return rate for Notebook"]') as HTMLInputElement;
    expect(rate.value).toBe('45');
    expect(document.querySelector('[data-testid="net-rate-hint"]')!.textContent).toContain('net of bill discount');
  });

  it('shows the backend refusal and does not save', async () => {
    const api = installApi({
      getSaleDetails: async () => saleDetail(),
      previewCustomerReturn: async () => {
        throw { type: 'ValidationError', details: { field: 'quantity', message: 'Cannot return 7 of sale line 11: sold 10, already returned 4' } };
      },
      createCustomerReturn: async () => result,
    });
    render(<ReturnFormCustomer initialBillId={5} />);
    await waitFor(() => bodyText().includes('Notebook'));
    setInput(document.querySelector('input[aria-label="Return quantity for Notebook"]')!, '7');
    await settle();
    expect(bodyText()).toContain('already returned 4');
    click(buttonByText('Record Return'));
    await flush();
    expect(api.createCustomerReturn).not.toHaveBeenCalled();
  });

  it('builds a replacement payload and shows the issued value the backend reports', async () => {
    const api = installApi({
      getSaleDetails: async () => saleDetail(),
      previewCustomerReturn: async () => ({ ...result, totalAmount: 100, issuedValue: 10, creditedAmount: 0, cashRefunded: 0 }),
      createCustomerReturn: async () => result,
    });
    render(<ReturnFormCustomer initialBillId={5} />);
    await waitFor(() => bodyText().includes('Notebook'));
    setInput(document.querySelector('input[aria-label="Return quantity for Notebook"]')!, '2');
    click(document.querySelectorAll('input[name="settlement"]')[1]);
    await flush();
    click(document.querySelector('[data-testid="replacement-section"] [data-testid="pick-item"]')!);
    await settle();
    expect(document.querySelector('[data-testid="value-difference"]')).not.toBeNull();
    click(buttonByText('Record Return'));
    await flush();
    const p = api.createCustomerReturn.mock.calls[0][0];
    expect(p.settlementType).toBe('REPLACEMENT');
    expect(p.replacementItems).toEqual([{ itemId: 9, quantity: 1 }]);
    expect(p.items[0].saleItemId).toBe(11);
  });

  it('no-receipt mode sends the entered unit cost; a missing cost is the backend refusal', async () => {
    let refuse = true;
    const api = installApi({
      getItemLots: [],
      previewCustomerReturn: async () => {
        if (refuse) throw { type: 'ValidationError', details: { field: 'unit_cost', message: 'unit cost required: item has no prior lot' } };
        return result;
      },
      createCustomerReturn: async () => result,
    });
    render(<ReturnFormCustomer />);
    click(document.querySelectorAll('input[name="return-mode"]')[1]);
    await flush();
    click(document.querySelector('[data-testid="pick-item"]')!);
    await settle();
    click(buttonByText('Record Return'));
    await flush();
    expect(bodyText()).toContain('unit cost required');
    expect(api.createCustomerReturn).not.toHaveBeenCalled();
    refuse = false;
    setInput(document.querySelector('input[aria-label="Unit cost for Gel Pen"]')!, '4');
    await settle();
    click(buttonByText('Record Return'));
    await flush();
    const p = api.createCustomerReturn.mock.calls[0][0];
    expect(p.originalSaleId).toBeNull();
    expect(p.items[0]).toMatchObject({ itemId: 9, saleItemId: null, unitCostOverride: 4, unitPrice: 10 });
  });
});

describe('ReturnFormVendor', () => {
  beforeEach(() => {
    document.body.innerHTML = '';
  });

  it('sends a REFUND payload without unitCost; the backend refuses more than remaining', async () => {
    const api = installApi({
      getPurchaseDetails: async () => purchaseDetail(),
      previewVendorReturn: async (p: { items: { quantity: number }[] }) => {
        if (p.items[0].quantity > 6) {
          throw { type: 'ValidationError', details: { field: 'quantity', message: 'Cannot return 7 from lot 21: only 6 remaining' } };
        }
        return { ...result, totalAmount: 60 };
      },
      createVendorReturn: async () => result,
    });
    render(<ReturnFormVendor initialBillId={8} />);
    await waitFor(() => bodyText().includes('Notebook'));
    const qty = document.querySelector('input[aria-label="Return quantity for Notebook"]')!;
    setInput(qty, '7');
    await settle();
    expect(bodyText()).toContain('only 6 remaining');
    click(buttonByText('Record Vendor Return'));
    await flush();
    expect(api.createVendorReturn).not.toHaveBeenCalled();
    setInput(qty, '2');
    await settle();
    click(buttonByText('Record Vendor Return'));
    await flush();
    const p = api.createVendorReturn.mock.calls[0][0];
    expect(p).toEqual({
      originalPurchaseId: 8,
      partyId: null,
      returnDate: expect.any(String),
      settlementType: 'REFUND',
      reason: null,
      notes: null,
      items: [{ itemId: 2, purchaseItemId: 21, quantity: 2 }],
    });
    expect(JSON.stringify(p)).not.toContain('unitCost');
  });
});
