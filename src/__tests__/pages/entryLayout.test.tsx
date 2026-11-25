import { describe, it, expect, vi, beforeEach } from 'vitest';
import { act } from 'react';
import { render } from '../testUtils';
import { click, flush, installApi, key, makeItem, typeInto } from '../inventory/testHelpers';
import { __resetHotkeys } from '@/hooks/useHotkeys';
import { useCartStore } from '@/store/useCartStore';
import { NEW_BUNDLE } from '@/lib/bundleSentinel';
import { fakeBillPreview, installApi as installPageApi } from './helpers';
import type { ItemSearchResult } from '@/types';

// Stubs: the pickers and the bundle modal have their own tests; here only the wiring matters.
let pickItem: (i: ItemSearchResult) => void = () => {};
let searchPlaceholder = '';
vi.mock('@/components/domain/ItemSearchTypeahead', () => ({
  ItemSearchTypeahead: (p: {
    onSelectItem: (i: ItemSearchResult) => void;
    onSelect?: (i: ItemSearchResult, info: { matchedBundleId: number | null }) => void;
    placeholder?: string;
  }) => {
    pickItem = (i) => {
      p.onSelectItem(i);
      p.onSelect?.(i, { matchedBundleId: null });
    };
    searchPlaceholder = p.placeholder ?? '';
    return <input data-testid="stub-search" placeholder={p.placeholder} />;
  },
}));
vi.mock('@/components/domain/PartySelector', () => ({
  PartySelector: () => <div data-testid="stub-party" />,
}));
const bundleModalProps: { isOpen: boolean; initialItemId?: number; baseUnit?: string }[] = [];
vi.mock('@/components/domain/BundleCreationModal', () => ({
  BundleCreationModal: (p: { isOpen: boolean; initialItemId?: number; baseUnit?: string }) => {
    bundleModalProps.push(p);
    return p.isOpen ? <div data-testid="bundle-modal">modal for {p.initialItemId}</div> : null;
  },
}));

import { RapidBillingRow } from '@/components/domain/RapidBillingRow';
import { NewPurchasePage } from '@/pages/NewPurchasePage';
import { NewSalePage } from '@/pages/NewSalePage';
import { InventoryPage } from '@/pages/InventoryPage';

const box = { bundleId: 5, bundleName: 'Box', unitsPerBundle: 25, bundlePrice: 100, defaultRate: 100, barcode: null };
const item = () => makeItem({ itemId: 9, name: 'Pencil', bundles: [box] });
const q = (root: ParentNode, sel: string) => root.querySelector<HTMLElement>(sel);

beforeEach(() => {
  document.body.innerHTML = '';
  __resetHotkeys();
  bundleModalProps.length = 0;
});

describe('Sale entry row: "+ New bundle…" (FR-29)', () => {
  function mountRow() {
    installApi({});
    const onOpenBundleModal = vi.fn();
    const r = render(<RapidBillingRow onOpenBundleModal={onOpenBundleModal} />);
    return { onOpenBundleModal, ...r };
  }
  const unitSelect = (c: HTMLElement) => q(c, '[data-testid="bundle-select"]') as HTMLSelectElement;

  it('has no sentinel before an item is chosen and no Bundle button', async () => {
    const { container } = mountRow();
    expect(Array.from(unitSelect(container).options).map((o) => o.value)).not.toContain(NEW_BUNDLE);
    expect(Array.from(container.querySelectorAll('button')).some((b) => /bundle/i.test(b.textContent ?? ''))).toBe(false);
  });

  it('opens the modal for the item and leaves the unit unchanged', async () => {
    const { container, onOpenBundleModal } = mountRow();
    act(() => pickItem(item()));
    await flush();
    const sel = unitSelect(container);
    expect(sel.options[sel.options.length - 1].value).toBe(NEW_BUNDLE);
    typeInto(sel, '5');
    expect(sel.value).toBe('5');
    typeInto(sel, NEW_BUNDLE);
    expect(onOpenBundleModal).toHaveBeenCalledTimes(1);
    expect(onOpenBundleModal.mock.calls[0][0].itemId).toBe(9);
    expect(sel.value).toBe('5');
  });

  it('arrow keys step through the units and skip the sentinel', async () => {
    const { container } = mountRow();
    act(() => pickItem(item()));
    await flush();
    const sel = unitSelect(container);
    key(sel, { key: 'ArrowDown' });
    expect(sel.value).toBe('5');
    key(sel, { key: 'ArrowDown' });
    expect(sel.value).toBe('5');
    key(sel, { key: 'ArrowUp' });
    expect(sel.value).toBe('base');
  });
});

describe('NewPurchasePage entry row (FR-29, FR-30)', () => {
  function mount() {
    installApi({ createPurchase: vi.fn(), previewPurchase: vi.fn(async (p) => fakeBillPreview(p)) });
    return render(<NewPurchasePage />);
  }

  it('shows the new placeholder and no New Bundle button', async () => {
    const { container } = mount();
    await flush();
    expect(searchPlaceholder).toBe('Search item by name or barcode…');
    expect(Array.from(container.querySelectorAll('button')).some((b) => /new bundle/i.test(b.textContent ?? ''))).toBe(false);
    expect(container.textContent).not.toContain('Alt+B new');
  });

  it('the sentinel opens the bundle modal and keeps the unit', async () => {
    const { container } = mount();
    await flush();
    act(() => pickItem(item()));
    const sel = container.querySelector('select[aria-label="Unit"]') as HTMLSelectElement;
    expect(sel.options[sel.options.length - 1].value).toBe(NEW_BUNDLE);
    typeInto(sel, '5');
    expect(sel.value).toBe('5');
    typeInto(sel, NEW_BUNDLE);
    await flush();
    expect(q(document.body, '[data-testid="bundle-modal"]')?.textContent).toContain('9');
    expect(sel.value).toBe('5');
  });

  it('keeps label and helper slots so the row height never changes', async () => {
    const { container } = mount();
    await flush();
    act(() => pickItem(item()));
    const label = container.querySelector('label[for="purchase-price"]') as HTMLElement;
    expect(label.textContent).toBe('Purchase Price / PCS *');
    expect(label.className).toContain('truncate');
    expect(label.getAttribute('title')).toBe('Purchase Price / PCS *');
    typeInto(container.querySelector('select[aria-label="Unit"]')!, '5');
    expect(q(container, 'label[for="purchase-price"]')?.textContent).toBe('Purchase Price / Box *');
    expect(container.textContent).toContain('1 Box × 25 = 25 PCS');
  });
});

describe('InventoryPage Add bundle (FR-29)', () => {
  it('opens the bundle modal for that item, physical items only', async () => {
    const rows = [
      makeItem({ itemId: 1, name: 'Notebook', bundles: [] }),
      makeItem({ itemId: 2, name: 'Repair work', itemType: 'SERVICE', bundles: [] }),
    ];
    installApi({
      getItemsCatalog: vi.fn().mockResolvedValue({ items: rows, total: 2, page: 1, limit: 50 }),
      listCategories: vi.fn().mockResolvedValue([]),
    });
    const { container } = render(<InventoryPage />);
    await flush();
    expect(container.querySelector('button[aria-label="Add bundle for Repair work"]')).toBeNull();
    const btn = container.querySelector('button[aria-label="Add bundle for Notebook"]') as HTMLElement;
    expect(btn.getAttribute('title')).toBe('Add packaging bundle');
    click(btn);
    await flush();
    expect(q(document.body, '[data-testid="bundle-modal"]')?.textContent).toContain('1');
  });
});

describe('NewSalePage layout (FR-30)', () => {
  beforeEach(() => {
    useCartStore.getState().clearCart();
    useCartStore.getState().addItem(
      { id: 1, name: 'Pencil', baseUnit: 'PCS', defaultSalePrice: 100, mrp: null },
      2,
      null,
      100
    );
  });

  it('puts the customer before the invoice pill and the totals with Complete Sale', async () => {
    installPageApi({
      getNextSaleCode: 'SAL-0002',
      searchItemsFts: [],
      getDashboardMetrics: { liveCashInDrawer: 0 },
      previewSale: async (p: Parameters<typeof fakeBillPreview>[0]) => fakeBillPreview(p),
    });
    const { container } = render(<NewSalePage />);
    await flush(200);
    const party = q(container, '[data-testid="stub-party"]')!;
    const pill = q(container, '[data-testid="invoice-preview"]')!;
    expect(party.compareDocumentPosition(pill) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    const footer = q(container, '[data-testid="sale-footer-actions"]')!;
    expect(footer.contains(q(container, '[data-testid="cart-subtotal"]'))).toBe(true);
    expect(footer.contains(q(container, '[data-testid="complete-sale-btn"]'))).toBe(true);
  });
});
