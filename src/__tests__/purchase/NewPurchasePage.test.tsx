import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render } from '../testUtils';
import { click, flush, installApi, key, makeItem, typeInto, byText } from '../inventory/testHelpers';
import type { ItemSearchResult } from '@/types';
import { fakeBillPreview } from '../pages/helpers';

/**
 * DD-56: the screen shows what `preview_purchase` returns. The double here only supplies totals
 * (plain arithmetic) and canned payment verdicts per scenario; the real rules are tested in Rust.
 */
type PreviewIn = Parameters<typeof fakeBillPreview>[0] & { partyId?: number | null };
const previewWith = (verdict: (p: PreviewIn) => { field: string; message: string }[] = () => []) =>
  vi.fn(async (p: PreviewIn) => fakeBillPreview(p, { errors: verdict(p) }));

// Stub the pickers: the real ones are covered by their own tests.
let pickItem: (i: ItemSearchResult) => void = () => {};
let pickParty: (p: { id: number } | null) => void = () => {};
vi.mock('@/components/domain/ItemSearchTypeahead', () => ({
  ItemSearchTypeahead: (p: { onSelectItem: (i: ItemSearchResult) => void }) => {
    pickItem = p.onSelectItem;
    return <input data-testid="stub-search" />;
  },
}));
vi.mock('@/components/domain/PartySelector', () => ({
  PartySelector: (p: { onSelectParty: (x: unknown) => void }) => {
    pickParty = p.onSelectParty as typeof pickParty;
    return <div data-testid="stub-party" />;
  },
}));

import { NewPurchasePage } from '@/pages/NewPurchasePage';

const box = { bundleId: 5, bundleName: 'Box', unitsPerBundle: 25, bundlePrice: null, barcode: null };

function mount(over: Record<string, unknown> = {}) {
  const api = installApi({
    createPurchase: vi.fn().mockResolvedValue({ purchaseId: 1, transactionCode: 'PUR-0042' }),
    previewPurchase: previewWith(),
    ...over,
  });
  const r = render(<NewPurchasePage />);
  return { api, ...r };
}

async function addBoxLine(container: HTMLElement, cost = '100') {
  const { act } = await import('react');
  act(() => pickItem(makeItem({ bundles: [box] })));
  const unit = container.querySelector('select') as HTMLSelectElement;
  typeInto(unit, '5');
  const qty = container.querySelector('input[type="number"][step="any"]') as HTMLInputElement;
  typeInto(qty, '4');
  const costInput = Array.from(container.querySelectorAll<HTMLInputElement>('input[type="number"]')).find((i) =>
    (i.id && container.querySelector(`label[for="${i.id}"]`)?.textContent?.includes('Purchase Price'))
  ) as HTMLInputElement;
  typeInto(costInput, cost);
  click(byText(container, 'button', /Add Line/));
  await settle();
}

/** Lets the debounced previews (totals, then payment) come back. */
const settle = async () => {
  for (let i = 0; i < 4; i++) await flush(120); // separate act scopes: totals first, then the payment preview
};

describe('NewPurchasePage', () => {
  beforeEach(() => {
    pickItem = () => {};
    pickParty = () => {};
  });

  it('shows the multiplier preview and normalised line math for a bundle', async () => {
    const { container } = mount();
    await flush();
    const { act } = await import('react');
    act(() => pickItem(makeItem({ bundles: [box] })));
    typeInto(container.querySelector('select') as HTMLSelectElement, '5');
    typeInto(container.querySelector('input[type="number"][step="any"]') as HTMLInputElement, '4');
    expect(container.textContent).toContain('4 Box × 25 = 100 PCS');

    await addBoxLine(container, '100');
    const row = container.querySelector('[data-testid="purchase-line"]') as HTMLElement;
    expect(row.textContent).toContain('100 PCS');
    expect(row.textContent).toContain('4.0000'); // 100 per box / 25
    expect(row.textContent).toContain('400.00'); // the line total is the preview's
    expect(container.querySelector('[data-testid="purchase-total"]')?.textContent).toContain('400.00');
  });

  it('submits the contract payload (no code, own supplierInvoiceRef, bundle id and qty)', async () => {
    const { container, api } = mount();
    await flush();
    await addBoxLine(container, '100');
    typeInto(container.querySelector('input[placeholder="e.g. INV-9841"]') as HTMLInputElement, ' INV-77 ');
    await settle();

    click(byText(container, 'button', /Record Inward Stock/));
    await flush();

    expect(api.createPurchase).toHaveBeenCalledTimes(1);
    const payload = api.createPurchase.mock.calls[0][0];
    expect(payload).toMatchObject({
      partyId: null,
      supplierInvoiceRef: 'INV-77',
      paymentStatus: 'PAID',
      paidAmount: 400,
      notes: null,
      items: [{ itemId: 1, bundleId: 5, bundleQuantity: 4, unitCost: 100 }],
    });
    expect(payload).not.toHaveProperty('transactionCode');
    expect(payload.transactionDate).toMatch(/^\d{4}-\d{2}-\d{2}$/);

    expect(container.querySelector('[data-testid="purchase-success"]')?.textContent).toContain('PUR-0042');
    expect(container.querySelectorAll('[data-testid="purchase-line"]').length).toBe(0);
  });

  it('requires a vendor for UNPAID and PARTIAL and blocks the call', async () => {
    const { container, api } = mount({
      previewPurchase: previewWith((p) =>
        p.paymentStatus && p.paymentStatus !== 'PAID' && p.partyId == null
          ? [{ field: 'party_id', message: 'vendor required for credit' }]
          : []
      ),
    });
    await flush();
    await addBoxLine(container);
    click(byText(container, 'button', 'UNPAID'));
    await settle();
    click(byText(container, 'button', /Record Inward Stock/));
    await flush();
    expect(api.createPurchase).not.toHaveBeenCalled();
    expect(container.querySelector('[role="alert"]')?.textContent).toMatch(/vendor/i);

    // With a vendor it goes through, paid = 0.
    const { act } = await import('react');
    act(() => pickParty({ id: 7 }));
    await settle();
    click(byText(container, 'button', /Record Inward Stock/));
    await flush();
    expect(api.createPurchase).toHaveBeenCalledTimes(1);
    expect(api.createPurchase.mock.calls[0][0]).toMatchObject({
      partyId: 7,
      paymentStatus: 'UNPAID',
      paidAmount: 0,
    });
  });

  it('rejects PARTIAL with paid >= total or 0 and overpayment', async () => {
    const { container, api } = mount({
      previewPurchase: previewWith((p) =>
        p.paymentStatus !== 'PARTIAL' || p.paidAmount == null
          ? []
          : p.paidAmount > 400
            ? [{ field: 'paid_amount', message: 'paid_amount cannot exceed the purchase total' }]
            : p.paidAmount >= 400 || p.paidAmount <= 0
              ? [{ field: 'paid_amount', message: 'PARTIAL needs 0 < paid_amount < total' }]
              : []
      ),
    });
    await flush();
    await addBoxLine(container); // total 400
    const { act } = await import('react');
    act(() => pickParty({ id: 7 }));
    click(byText(container, 'button', 'PARTIAL'));
    const paid = container.querySelector('input[aria-label="Paid amount"]') as HTMLInputElement;

    typeInto(paid, '400');
    await settle();
    click(byText(container, 'button', /Record Inward Stock/));
    await flush();
    expect(api.createPurchase).not.toHaveBeenCalled();
    expect(container.querySelector('[role="alert"]')?.textContent).toMatch(/PARTIAL|exceed/);

    typeInto(paid, '500');
    await settle();
    click(byText(container, 'button', /Record Inward Stock/));
    await flush();
    expect(container.querySelector('[role="alert"]')?.textContent).toMatch(/exceed/i);
    expect(api.createPurchase).not.toHaveBeenCalled();

    typeInto(paid, '150');
    await settle();
    click(byText(container, 'button', /Record Inward Stock/));
    await flush();
    expect(api.createPurchase.mock.calls[0][0]).toMatchObject({ paymentStatus: 'PARTIAL', paidAmount: 150, partyId: 7 });
  });

  it('shows backend errors via errorMessage', async () => {
    const { container } = mount({
      createPurchase: vi.fn().mockRejectedValue({
        type: 'ValidationError',
        details: { field: 'paid_amount', message: 'PAID needs paid_amount = total' },
      }),
    });
    await flush();
    await addBoxLine(container);
    click(byText(container, 'button', /Record Inward Stock/));
    await flush();
    expect(container.querySelector('[role="alert"]')?.textContent).toBe('PAID needs paid_amount = total');
  });

  it('Ctrl+Enter records once even when fired twice (double-submit guard)', async () => {
    let resolve: (v: unknown) => void = () => {};
    const createPurchase = vi.fn().mockImplementation(() => new Promise((r) => (resolve = r)));
    const { container } = mount({ createPurchase });
    await flush();
    await addBoxLine(container);
    key(window, { key: 'Enter', ctrlKey: true });
    key(window, { key: 'Enter', ctrlKey: true });
    expect(createPurchase).toHaveBeenCalledTimes(1);
    resolve({ purchaseId: 1, transactionCode: 'PUR-0001' });
    await flush();
  });

  it('Enter on the draft row adds the line and returns focus to item search', async () => {
    const { container } = mount();
    await flush();
    const { act } = await import('react');
    act(() => pickItem(makeItem()));
    const cost = Array.from(container.querySelectorAll<HTMLInputElement>('input[type="number"]')).find((i) =>
      container.querySelector(`label[for="${i.id}"]`)?.textContent?.includes('Purchase Price')
    ) as HTMLInputElement;
    typeInto(cost, '12.5');
    key(cost, { key: 'Enter' });
    await flush(10);
    expect(container.querySelectorAll('[data-testid="purchase-line"]').length).toBe(1);
    expect(document.activeElement).toBe(container.querySelector('[data-testid="stub-search"]'));
  });

  it('Alt+B opens the bundle modal for the selected item and auto-selects the new bundle', async () => {
    const created = { id: 9, itemId: 1, bundleName: 'Dozen', unitsPerBundle: 12, mrp: null, defaultBundlePrice: null, barcode: null };
    const { container, api } = mount({ createBundle: vi.fn().mockResolvedValue(created) });
    await flush();
    const { act } = await import('react');
    act(() => pickItem(makeItem()));
    key(window, { key: 'b', altKey: true });
    await flush(60);
    expect(document.body.textContent).toContain('Create Packaging Bundle');
    typeInto(container.querySelector('input[placeholder^="e.g. Box"]') as HTMLInputElement, 'Dozen');
    typeInto(container.querySelector('input[type="number"][min="2"]') as HTMLInputElement, '12');
    click(byText(container, 'button', /Create Bundle/));
    await flush();
    expect(api.createBundle).toHaveBeenCalledWith(expect.objectContaining({ itemId: 1, bundleName: 'Dozen', unitsPerBundle: 12 }));
    expect((container.querySelector('select') as HTMLSelectElement).value).toBe('9');
  });

  it('Alt+X adds a charge and a discount: grand total, PAID follows it, payload carries charges', async () => {
    const carrier = { id: 4, name: 'Carrier Fee', effect: 'CHARGE', appliesTo: 'PURCHASE', defaultAmount: 100, isActive: true, createdAt: '' };
    const disc = { id: 3, name: 'Cash Discount', effect: 'DISCOUNT', appliesTo: 'BOTH', defaultAmount: null, isActive: true, createdAt: '' };
    const { container, api } = mount({
      searchChargeTypes: vi.fn(async (q: string) =>
        [carrier, disc].filter((t) => t.name.toLowerCase().startsWith(q.toLowerCase()))
      ),
    });
    await flush();
    await addBoxLine(container, '100'); // 400
    const tid = (id: string) => container.querySelector(`[data-testid="${id}"]`) as HTMLInputElement;
    const addOne = async (prefix: string, amount?: string) => {
      key(window, { key: 'x', altKey: true });
      await flush(10);
      const input = tid('charge-type-input');
      typeInto(input, prefix);
      await flush(120);
      key(input, { key: 'Enter' });
      await flush(10);
      const amt = tid('charge-amount');
      if (amount) typeInto(amt, amount);
      key(amt, { key: 'Enter' });
      await flush(10);
    };
    await addOne('car');
    await settle();
    expect(tid('purchase-total').textContent).toContain('500.00');
    await addOne('cash', '30');
    await settle();
    expect(tid('purchase-total').textContent).toContain('470.00');
    expect(tid('purchase-items-subtotal').textContent).toBe('400');

    click(byText(container, 'button', /Record Inward Stock/));
    await flush();
    const payload = api.createPurchase.mock.calls[0][0];
    expect(payload.paidAmount).toBe(470);
    expect(payload.charges).toEqual([
      { chargeTypeId: 4, amount: 100 },
      { chargeTypeId: 3, amount: 30 },
    ]);
  });
});
