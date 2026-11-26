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
  PartySelector: (p: { onSelectParty?: (party: { id: number; name: string }) => void }) => (
    <span>
      party-selector
      {[
        { id: 4, name: 'Vendor A' },
        { id: 7, name: 'Vendor B' },
      ].map((v) => (
        <button key={v.id} type="button" data-testid={`pick-party-${v.id}`} onClick={() => p.onSelectParty?.(v)}>
          {v.name}
        </button>
      ))}
    </span>
  ),
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

const result = { returnId: 1, returnCode: 'RET-1', creditedAmount: 100, cashRefunded: 50, totalAmount: 150, issuedValue: 0, discountReversed: 0 };

const vlot = (id: number, code: string, remaining: number, net = 30) => ({
  purchaseItemId: id,
  purchaseId: id * 10,
  purchaseCode: code,
  transactionDate: '2026-11-12',
  unitCost: 30,
  netUnitCost: net,
  remainingQuantity: remaining,
  baseUnit: 'PCS',
});

const byTestId = (id: string) => document.querySelector(`[data-testid="${id}"]`)!;
const vendorMode = () => {
  const radios = document.querySelectorAll('input[name="vreturn-mode"]');
  click(radios[1]);
};

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

  it('does not offer service lines of a bill for return (DD-67)', async () => {
    const d = saleDetail();
    const svc = {
      ...d.items[0],
      saleItem: { ...d.items[0].saleItem, id: 12, itemId: 3, itemName: 'Photo print' },
      itemName: 'Photo print',
      isService: true,
    };
    installApi({ getSaleDetails: async () => ({ ...d, items: [d.items[0], svc] }) as unknown as SaleDetailView });
    render(<ReturnFormCustomer initialBillId={5} />);
    await waitFor(() => bodyText().includes('Notebook'));
    expect(bodyText()).not.toContain('Photo print');
    expect(document.querySelector('input[aria-label="Return quantity for Photo print"]')).toBeNull();
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

  it('feeds the lot select from listVendorReturnLots and preselects a single lot (no FIFO option)', async () => {
    const api = installApi({
      listVendorReturnLots: async () => [vlot(21, 'PUR-0012', 8, 27)],
      previewVendorReturn: async () => ({ ...result, totalAmount: 27, discountReversed: 3 }),
      createVendorReturn: async () => result,
    });
    render(<ReturnFormVendor />);
    vendorMode();
    await flush();
    click(byTestId('pick-party-4'));
    click(byTestId('pick-item'));
    await waitFor(() => bodyText().includes('PUR-0012'));
    expect(api.listVendorReturnLots).toHaveBeenCalledWith(9, 4, null);
    expect(bodyText()).toContain('PUR-0012 · 2026-11-12 · ₹30.00 (net ₹27.00) · 8 PCS left');
    expect(bodyText()).toContain('Qty (max 8)');
    expect(bodyText()).not.toContain('FIFO');
    await settle();
    expect(byTestId('discount-reversed').textContent).toContain('Purchase discount reversed');
    click(buttonByText('Record Vendor Return'));
    await flush();
    expect(api.createVendorReturn.mock.calls[0][0]).toMatchObject({
      originalPurchaseId: null,
      partyId: 4,
      items: [{ itemId: 9, purchaseItemId: 21, quantity: 1 }],
    });
  });

  it('asks for a lot when several are eligible and shows the backend refusal', async () => {
    installApi({
      listVendorReturnLots: async () => [vlot(21, 'PUR-0012', 8), vlot(22, 'PUR-0013', 2)],
      previewVendorReturn: async (p: { items: { purchaseItemId: number | null }[] }) => {
        if (p.items[0].purchaseItemId === null) {
          throw { type: 'BillLocked', details: { code: 'VENDOR_LOT_REQUIRED', message: "Choose the lot of 'Gel Pen' to return to the vendor" } };
        }
        return result;
      },
    });
    render(<ReturnFormVendor />);
    vendorMode();
    await flush();
    click(byTestId('pick-party-4'));
    click(byTestId('pick-item'));
    await waitFor(() => bodyText().includes('PUR-0013'));
    expect(bodyText()).toContain('Choose a lot...');
    await settle();
    expect(bodyText()).toContain("Choose the lot of 'Gel Pen'");
    const select = document.querySelector('select[aria-label="Lot for Gel Pen"]') as HTMLSelectElement;
    setInput(select, '22');
    await settle();
    expect(bodyText()).toContain('Qty (max 2)');
    expect(bodyText()).not.toContain("Choose the lot of 'Gel Pen'");
  });

  it('reloads the lots when the vendor changes and says when there is nothing to return', async () => {
    const api = installApi({
      listVendorReturnLots: async (_item: number, party: number) => (party === 4 ? [vlot(21, 'PUR-0012', 8)] : []),
      previewVendorReturn: async () => result,
    });
    render(<ReturnFormVendor />);
    vendorMode();
    await flush();
    click(byTestId('pick-party-4'));
    click(byTestId('pick-item'));
    await waitFor(() => bodyText().includes('PUR-0012'));
    click(byTestId('pick-party-7'));
    await waitFor(() => bodyText().includes('No stock of this item from Vendor B to return'));
    expect(api.listVendorReturnLots).toHaveBeenLastCalledWith(9, 7, null);
    expect(bodyText()).not.toContain('PUR-0012');
  });
});
