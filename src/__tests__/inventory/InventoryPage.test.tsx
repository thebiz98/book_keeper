import { describe, it, expect, vi } from 'vitest';
import { render } from '../testUtils';
import { act } from 'react';
import { byText, click, flush, inputByLabel, installApi, key, makeItem, typeInto } from './testHelpers';
import { InventoryPage } from '@/pages/InventoryPage';

const dozen = { bundleId: 7, bundleName: 'Dozen', unitsPerBundle: 12, bundlePrice: 500, barcode: 'B-1' };
const rows = [
  makeItem({ itemId: 1, name: 'Classmate Notebook', bundles: [dozen], mrp: 50, currentStock: 3, isLowStock: true }),
  makeItem({ itemId: 2, name: 'Gel Pen', category: 'Pens', mrp: null, bundles: [], currentStock: 0 }),
];

function mount(over: Record<string, unknown> = {}) {
  const api = installApi({
    getItemsCatalog: vi.fn().mockResolvedValue({ items: rows, total: 120, page: 1, limit: 50 }),
    listCategories: vi.fn().mockResolvedValue(['Notebooks', 'Pens']),
    updateItem: vi.fn().mockResolvedValue(undefined),
    createItem: vi.fn().mockResolvedValue(3),
    ...over,
  });
  const r = render(<InventoryPage />);
  return { api, ...r };
}

describe('InventoryPage catalog', () => {
  it('fills the page height with a sticky header (FR-10 rev.)', async () => {
    const { container } = mount();
    await flush();
    expect(container.querySelector('[data-table-scroll]')?.getAttribute('data-table-scroll')).toBe('fill');
    expect(container.querySelector('th')?.className).toContain('sticky');
  });

  it('loads server-side page 1 and renders spec columns with itemId rows', async () => {
    const { api, container } = mount();
    await flush();
    expect(api.getItemsCatalog).toHaveBeenCalledWith({
      category: null,
      lowStockOnly: null,
      search: null,
      page: 1,
      limit: 50,
    });
    const head = container.querySelector('thead')?.textContent ?? '';
    for (const h of ['Item Name', 'Category', 'Base Unit', 'Current Stock', 'Low Stock Level', 'MRP', 'Sale Price', 'Bundles']) {
      expect(head).toContain(h);
    }
    expect(head).not.toContain('Barcode');
    const trs = container.querySelectorAll('[data-testid="item-row"]');
    expect(trs.length).toBe(2);
    expect(trs[0].textContent).toContain('Low Stock: 3 PCS');
    expect(trs[1].textContent).toContain('Out of Stock');
    expect(trs[1].textContent).toContain('—'); // null MRP
    expect(container.querySelector('[data-testid="pager-summary"]')?.textContent).toContain('Page 1 of 3');
  });

  it('populates categories from listCategories and filters server-side, resetting to page 1', async () => {
    const { api, container } = mount();
    await flush();
    const select = container.querySelector('select[aria-label="Category filter"]') as HTMLSelectElement;
    expect(Array.from(select.options).map((o) => o.value)).toEqual(['ALL', 'Notebooks', 'Pens']);

    click(byText(container, 'button', 'Next'));
    await flush();
    expect(api.getItemsCatalog).toHaveBeenLastCalledWith(expect.objectContaining({ page: 2 }));

    typeInto(select, 'Pens');
    await flush();
    expect(api.getItemsCatalog).toHaveBeenLastCalledWith(expect.objectContaining({ category: 'Pens', page: 1 }));

    click(container.querySelector('input[aria-label="Low stock only"]') as HTMLInputElement);
    await flush();
    expect(api.getItemsCatalog).toHaveBeenLastCalledWith(
      expect.objectContaining({ category: 'Pens', lowStockOnly: true, page: 1 })
    );
  });

  it('searches live (150 ms after typing, no Search button)', async () => {
    const { api, container } = mount();
    await flush();
    expect(Array.from(container.querySelectorAll('button')).some((b) => b.textContent === 'Search')).toBe(false);
    typeInto(container.querySelector('input[aria-label="Search items"]') as HTMLInputElement, ' note ');
    await flush(40);
    expect(api.getItemsCatalog).not.toHaveBeenLastCalledWith(expect.objectContaining({ search: 'note' }));
    await flush(200);
    expect(api.getItemsCatalog).toHaveBeenLastCalledWith(expect.objectContaining({ search: 'note', page: 1 }));
  });

  it('pager buttons are disabled at the ends', async () => {
    const { container } = mount();
    await flush();
    expect((byText(container, 'button', 'Previous') as HTMLButtonElement).disabled).toBe(true);
    expect((byText(container, 'button', 'Next') as HTMLButtonElement).disabled).toBe(false);
  });

  it('expands a row into the lot inspector: active lots first with source badge and vendor', async () => {
    const lots = [
      { purchaseItemId: 1, purchaseTransactionId: 1, transactionCode: 'PUR-0001', source: 'PURCHASE', transactionDate: '2026-01-01', vendorName: 'Old Vendor', quantity: 10, remainingQuantity: 0, unitCost: 5 },
      { purchaseItemId: 2, purchaseTransactionId: 2, transactionCode: 'ADJ-0001', source: 'ADJUSTMENT', transactionDate: '2026-03-01', vendorName: null, quantity: 4, remainingQuantity: 3, unitCost: 6 },
      { purchaseItemId: 3, purchaseTransactionId: 3, transactionCode: 'PUR-0002', source: 'PURCHASE', transactionDate: '2026-02-01', vendorName: 'Acme', quantity: 8, remainingQuantity: 8, unitCost: 7 },
    ];
    const { api, container } = mount({ getItemLots: vi.fn().mockResolvedValue(lots) });
    await flush();
    click(container.querySelector('button[aria-label="Show lots for Classmate Notebook"]') as HTMLElement);
    await flush();
    expect(api.getItemLots).toHaveBeenCalledWith(1);
    const lotRows = Array.from(container.querySelectorAll('[data-testid="lot-row"]'));
    expect(lotRows.map((r) => r.querySelectorAll('td')[1].textContent)).toEqual(['PUR-0002', 'ADJ-0001', 'PUR-0001']);
    expect(lotRows[0].textContent).toContain('Purchase');
    expect(lotRows[0].textContent).toContain('Acme');
    expect(lotRows[1].textContent).toContain('Adjustment');
    expect(lotRows[1].textContent).toContain('—');
  });
});

describe('Low Stock Level inline edit', () => {
  const lsl = (c: HTMLElement, name: string) =>
    c.querySelector(`input[aria-label="Low stock level for ${name}"]`) as HTMLInputElement;
  const saved = (over = {}) => ({ ...rows[0], lowStockThreshold: 8, isLowStock: true, ...over });

  it('Enter saves, shows the returned row and focuses the next row input', async () => {
    const fn = vi.fn().mockResolvedValue(saved({ isLowStock: false, currentStock: 3 }));
    const { container } = mount({ updateItemLowStockThreshold: fn });
    await flush();
    const first = lsl(container, 'Classmate Notebook');
    expect(first.value).toBe('5');
    act(() => first.focus());
    typeInto(first, '8');
    key(first, { key: 'Enter' });
    await flush();
    expect(fn).toHaveBeenCalledWith(1, 8);
    expect(first.value).toBe('8');
    expect(document.activeElement).toBe(lsl(container, 'Gel Pen'));
    expect(container.querySelectorAll('[data-testid="item-row"]')[0].textContent).not.toContain('Low Stock:');
  });

  it('Esc reverts without a call', async () => {
    const fn = vi.fn();
    const { container } = mount({ updateItemLowStockThreshold: fn });
    await flush();
    const first = lsl(container, 'Classmate Notebook');
    act(() => first.focus());
    typeInto(first, '9');
    key(first, { key: 'Escape' });
    await flush();
    expect(first.value).toBe('5');
    expect(fn).not.toHaveBeenCalled();
  });

  it('negative input shows an alert and is not saved', async () => {
    const fn = vi.fn();
    const { container } = mount({ updateItemLowStockThreshold: fn });
    await flush();
    const first = lsl(container, 'Classmate Notebook');
    act(() => first.focus());
    typeInto(first, '-2');
    key(first, { key: 'Enter' });
    await flush();
    expect(fn).not.toHaveBeenCalled();
    expect(container.querySelector('[role="alert"]')?.textContent).toBe('Enter 0 or more');
  });

  it('blur saves; an unchanged value makes no call', async () => {
    const fn = vi.fn().mockResolvedValue(saved());
    const { container } = mount({ updateItemLowStockThreshold: fn });
    await flush();
    const first = lsl(container, 'Classmate Notebook');
    act(() => first.focus());
    act(() => first.blur());
    await flush();
    expect(fn).not.toHaveBeenCalled();
    act(() => first.focus());
    typeInto(first, '8');
    act(() => first.blur());
    await flush();
    expect(fn).toHaveBeenCalledTimes(1);
    expect(fn).toHaveBeenCalledWith(1, 8);
  });

  it('a rejected call shows the error message', async () => {
    const fn = vi.fn().mockRejectedValue({ type: 'ValidationError', details: { field: 'low_stock_threshold', message: 'Nope' } });
    const { container } = mount({ updateItemLowStockThreshold: fn });
    await flush();
    const first = lsl(container, 'Classmate Notebook');
    act(() => first.focus());
    typeInto(first, '8');
    key(first, { key: 'Enter' });
    await flush();
    expect(container.querySelector('[role="alert"]')?.textContent).toContain('Nope');
  });
});

describe('Item edit drawer', () => {
  async function openEdit(container: HTMLElement) {
    click(container.querySelector('button[aria-label="Edit Classmate Notebook"]') as HTMLElement);
    await flush();
  }

  it('clearing MRP sends null (never 0); stock is read-only', async () => {
    const { api, container } = mount();
    await flush();
    await openEdit(container);
    expect(container.querySelector('[data-testid="readonly-stock"]')?.textContent).toContain('3 PCS');

    const mrp = inputByLabel(container, /^MRP/);
    expect(mrp.value).toBe('50');
    typeInto(mrp, '');
    click(byText(container, 'button', /Save Changes/));
    await flush();

    expect(api.updateItem).toHaveBeenCalledTimes(1);
    expect(api.updateItem).toHaveBeenCalledWith(1, {
      name: 'Classmate Notebook',
      category: 'Notebooks',
      barcode: '8901',
      baseUnit: 'PCS',
      mrp: null,
      defaultSalePrice: 45,
      lowStockThreshold: 5,
    });
  });

  it('edit sends the changed Low Stock Level; negative shows a field error', async () => {
    const { api, container } = mount();
    await flush();
    await openEdit(container);
    const low = inputByLabel(container, /Low Stock Level/);
    expect(low.value).toBe('5');
    typeInto(low, '-1');
    click(byText(container, 'button', /Save Changes/));
    await flush();
    expect(api.updateItem).not.toHaveBeenCalled();
    expect(container.textContent).toContain('Low stock level must be 0 or more');
    typeInto(low, '12');
    click(byText(container, 'button', /Save Changes/));
    await flush();
    expect(api.updateItem).toHaveBeenCalledWith(1, expect.objectContaining({ lowStockThreshold: 12 }));
  });

  it('rejects MRP 0 on the client', async () => {
    const { api, container } = mount();
    await flush();
    await openEdit(container);
    typeInto(inputByLabel(container, /^MRP/), '0');
    click(byText(container, 'button', /Save Changes/));
    await flush();
    expect(api.updateItem).not.toHaveBeenCalled();
    expect(container.textContent).toContain('MRP must be greater than 0');
  });

  it('shows backend ValidationError under the offending field', async () => {
    const { container } = mount({
      updateItem: vi.fn().mockRejectedValue({
        type: 'ValidationError',
        details: { field: 'barcode', message: 'Barcode already in use' },
      }),
    });
    await flush();
    await openEdit(container);
    click(byText(container, 'button', /Save Changes/));
    await flush();
    const err = Array.from(container.querySelectorAll('[role="alert"]')).find((e) => e.textContent === 'Barcode already in use');
    expect(err).toBeTruthy();
    // sits in the barcode field's wrapper
    expect(err?.parentElement?.querySelector('input')?.value).toBe('8901');
  });

  it('shows a read-only Physical type line in the item drawer (FR-21)', async () => {
    const { container } = mount();
    await flush();
    click(byText(container, 'button', /Add Item/));
    await flush();
    expect(container.querySelector('[data-testid="item-type"]')?.textContent).toContain('Physical');
  });

  it('creates an item with MRP null when empty', async () => {
    const { api, container } = mount();
    await flush();
    click(byText(container, 'button', /Add Item/));
    await flush();
    typeInto(inputByLabel(container, /Item Name/), 'New Ruler');
    typeInto(inputByLabel(container, /Default Sale Price/), '10');
    click(byText(container, 'button', /Save Item/));
    await flush();
    expect(api.createItem).toHaveBeenCalledWith(
      expect.objectContaining({ name: 'New Ruler', mrp: null, defaultSalePrice: 10, initialStock: null, bundles: null })
    );
  });
});

describe('Bundles sub-table', () => {
  async function openEdit(container: HTMLElement) {
    click(container.querySelector('button[aria-label="Edit Classmate Notebook"]') as HTMLElement);
    await flush();
  }

  it('lists bundles and deletes after confirmation', async () => {
    const { api, container } = mount({ deleteBundle: vi.fn().mockResolvedValue(undefined) });
    await flush();
    await openEdit(container);
    const row = container.querySelector('[data-testid="bundle-row"]') as HTMLElement;
    expect(row.textContent).toContain('Dozen');
    expect(row.textContent).toContain('× 12');
    click(container.querySelector('button[aria-label="Delete bundle Dozen"]') as HTMLElement);
    expect(api.deleteBundle).not.toHaveBeenCalled();
    click(byText(container, 'button', 'Confirm delete'));
    await flush();
    expect(api.deleteBundle).toHaveBeenCalledWith(7);
  });

  it('adds a bundle via createBundle with the item id', async () => {
    const created = { id: 8, itemId: 1, bundleName: 'Box', unitsPerBundle: 50, mrp: null, defaultBundlePrice: null, barcode: null };
    const { api, container } = mount({ createBundle: vi.fn().mockResolvedValue(created) });
    await flush();
    await openEdit(container);
    click(byText(container, 'button', /Add Bundle/));
    await flush(60);
    typeInto(container.querySelector('input[placeholder^="e.g. Box"]') as HTMLInputElement, 'Box');
    typeInto(container.querySelector('input[type="number"][min="2"]') as HTMLInputElement, '50');
    click(byText(container, 'button', /Create Bundle/));
    await flush();
    expect(api.createBundle).toHaveBeenCalledWith({
      itemId: 1,
      bundleName: 'Box',
      unitsPerBundle: 50,
      defaultBundlePrice: null,
      mrp: null,
      barcode: null,
    });
  });

  it('edits a bundle via updateBundle (multiplier included)', async () => {
    const updated = { id: 7, itemId: 1, bundleName: 'Dozen', unitsPerBundle: 10, mrp: null, defaultBundlePrice: 500, barcode: 'B-1' };
    const { api, container } = mount({ updateBundle: vi.fn().mockResolvedValue(updated) });
    await flush();
    await openEdit(container);
    click(container.querySelector('button[aria-label="Edit bundle Dozen"]') as HTMLElement);
    await flush(60);
    expect(document.body.textContent).toContain('Edit Packaging Bundle');
    typeInto(container.querySelector('input[type="number"][min="2"]') as HTMLInputElement, '10');
    click(byText(container, 'button', /Save Bundle/));
    await flush();
    expect(api.updateBundle).toHaveBeenCalledWith(7, {
      bundleName: 'Dozen',
      unitsPerBundle: 10,
      defaultBundlePrice: 500,
      mrp: null,
      barcode: 'B-1',
    });
  });
});

describe('Set stock entry points (DD-40)', () => {
  const stockApi = {
    getItemLots: vi.fn().mockResolvedValue([]), setItemStock: vi.fn().mockResolvedValue({
      adjustmentId: 1, delta: 5, reason: 'UNRECORDED_STOCK', previousStock: 3, newStock: 8, unitCost: 4, valueChange: 20,
    }),
  };

  it('row action opens the dialog and a save refreshes the catalog; SERVICE rows have no button', async () => {
    const service = makeItem({ itemId: 3, name: 'Lamination', itemType: 'SERVICE', currentStock: 0 });
    const api = installApi({
      getItemsCatalog: vi.fn().mockResolvedValue({ items: [...rows, service], total: 3, page: 1, limit: 50 }),
      listCategories: vi.fn().mockResolvedValue([]),
      ...stockApi,
    });
    const { container } = render(<InventoryPage />);
    await flush();
    expect(container.querySelector('[aria-label="Set stock for Lamination"]')).toBeNull();
    click(container.querySelector('[aria-label="Set stock for Classmate Notebook"]') as HTMLElement);
    await flush();
    expect(container.textContent).toContain('Set stock on hand');
    const input = container.querySelector('input[type="number"]') as HTMLInputElement;
    typeInto(input, '8');
    typeInto(container.querySelectorAll('input[type="number"]')[1], '4');
    await flush();
    click(byText(container, 'button', /Save stock/));
    await flush();
    expect(api.setItemStock).toHaveBeenCalledTimes(1);
    expect(api.getItemsCatalog).toHaveBeenCalledTimes(2);
  });

  it('drawer shows stock on hand with Set stock; Cancel closes only the dialog', async () => {
    installApi({
      getItemsCatalog: vi.fn().mockResolvedValue({ items: rows, total: 2, page: 1, limit: 50 }),
      listCategories: vi.fn().mockResolvedValue([]),
      ...stockApi,
    });
    const { container } = render(<InventoryPage />);
    await flush();
    click(container.querySelector('button[aria-label="Edit Classmate Notebook"]') as HTMLElement);
    await flush();
    expect(container.textContent).toContain('Stock on hand');
    click(container.querySelector('[data-testid="set-stock-button"]') as HTMLElement);
    await flush();
    expect(container.textContent).toContain('Set stock on hand');
    click(byText(container, 'button', /^Cancel/));
    await flush();
    expect(container.textContent).not.toContain('Set stock on hand');
    expect(container.textContent).toContain('Edit Item: Classmate Notebook');
  });
});
