import { describe, it, expect, beforeEach, vi } from 'vitest';
import { render } from '../testUtils';
import { installApi, flush, waitFor, click, setInput, fakeBillPreview } from '../pages/helpers';
import { BillDetailDrawer } from '@/components/domain/BillDetailDrawer';
import { NEW_BUNDLE } from '@/lib/bundleSentinel';
import { purchaseDetail, purchaseGuard, saleDetail, saleGuard, search } from './fixtures';

(globalThis as unknown as { IS_REACT_ACT_ENVIRONMENT: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

vi.mock('@/components/domain/ItemSearchTypeahead', async () => {
  const { INK: ink } = await import('./fixtures');
  return {
    ItemSearchTypeahead: ({ onSelectItem }: { onSelectItem: (i: unknown) => void }) => (
      <button type="button" data-testid="stub-pick" onClick={() => onSelectItem({ ...ink, defaultSalePrice: 11 })}>
        pick
      </button>
    ),
  };
});
vi.mock('@/components/domain/PartySelector', () => ({ PartySelector: () => <div data-testid="stub-party" /> }));

const q = (sel: string) => document.querySelector(sel) as HTMLInputElement;
const qa = (sel: string) => Array.from(document.querySelectorAll<HTMLSelectElement>(sel));
const settle = async () => {
  for (let i = 0; i < 4; i++) await flush(120);
};
const UNIT = 'select[aria-label="Unit Ink"]';

const saved = { id: 9, itemId: 2, bundleName: 'Carton', unitsPerBundle: 20, mrp: null, defaultBundlePrice: 70, barcode: null, defaultRate: 77 };

function mount(kind: 'PURCHASE' | 'SALE') {
  const api = installApi({
    getPurchaseDetails: purchaseDetail(),
    getSaleDetails: saleDetail(),
    getBillEditGuard: kind === 'PURCHASE' ? purchaseGuard() : saleGuard(),
    searchItemsFts: search,
    previewPurchase: async (p: Parameters<typeof fakeBillPreview>[0]) => fakeBillPreview(p),
    previewSale: async (p: Parameters<typeof fakeBillPreview>[0]) => fakeBillPreview(p),
    getBundleSuggestions: [],
    createBundle: saved,
    getDashboardMetrics: { cashInHand: 0 },
  });
  render(<BillDetailDrawer kind={kind} billId={kind === 'PURCHASE' ? 8 : 5} onClose={() => {}} />);
  return api;
}

async function openEdit() {
  await waitFor(() => !!document.querySelector('[data-testid="bill-edit"]:not([disabled])'));
  click(q('[data-testid="bill-edit"]'));
  await waitFor(() => document.querySelectorAll('[data-testid="edit-line"]').length > 0);
  await settle();
}

describe.each([{ kind: 'PURCHASE' as const }, { kind: 'SALE' as const }])('$kind edit form: "+ New bundle" (C8, FR-29)', ({ kind }) => {
  beforeEach(() => {
    document.body.innerHTML = '';
  });

  it('the sentinel is the last option and opens the modal for that line item', async () => {
    mount(kind);
    await openEdit();
    const sel = qa(UNIT)[0];
    expect(Array.from(sel.options).at(-1)!.value).toBe(NEW_BUNDLE);
    setInput(sel, NEW_BUNDLE);
    await settle();
    expect(document.body.textContent).toContain('Bundle Name');
    expect(document.body.textContent).toContain('PCS per Bundle');
    expect(qa(UNIT)[0].value).toBe('0'); // unit unchanged
  });

  it('Cancel keeps the unit', async () => {
    const api = mount(kind);
    await openEdit();
    setInput(qa(UNIT)[0], NEW_BUNDLE);
    await settle();
    const cancel = Array.from(document.querySelectorAll('button')).find(
      (b) => /^Cancel/.test(b.textContent ?? '') && !b.dataset.testid
    )!;
    click(cancel);
    await settle();
    expect(document.body.textContent).not.toContain('Bundle Name');
    expect(qa(UNIT)[0].value).toBe('0');
    expect(api.createBundle).not.toHaveBeenCalled();
  });

  it('Save selects the new bundle on that line only; the same item elsewhere only gains the option', async () => {
    const api = mount(kind);
    await openEdit();
    click(q('[data-testid="stub-pick"]')); // a second Ink line
    await settle();
    expect(qa(UNIT)).toHaveLength(2);
    setInput(qa(UNIT)[0], NEW_BUNDLE);
    await settle();
    setInput(document.querySelector('input[placeholder^="e.g. Box"]')!, 'Carton');
    setInput(document.querySelector('input[type="number"][min="2"]')!, '20');
    const create = Array.from(document.querySelectorAll('button')).find((b) => /Create Bundle/.test(b.textContent ?? ''))!;
    click(create);
    await settle();
    expect(api.createBundle).toHaveBeenCalledTimes(1);
    const [a, b] = qa(UNIT);
    expect(a.value).toBe('9');
    expect(b.value).toBe('0');
    expect(Array.from(b.options).map((o) => o.value)).toContain('9');
    if (kind === 'SALE') {
      // the backend-suggested rate, as a normal unit change does
      expect(document.querySelectorAll<HTMLInputElement>('input[aria-label="Rate Ink"]')[0].value).toBe('77');
      expect(document.querySelectorAll<HTMLInputElement>('input[aria-label="Rate Ink"]')[1].value).toBe('11');
    }
  });

  it('a select has the sentinel exactly when it is enabled', async () => {
    mount(kind);
    await openEdit();
    const selects = qa('select[aria-label^="Unit "]');
    expect(selects.length).toBeGreaterThan(0);
    for (const sel of selects) {
      const hasSentinel = Array.from(sel.options).some((o) => o.value === NEW_BUNDLE);
      expect(hasSentinel).toBe(!sel.disabled);
    }
  });
});
