import { describe, it, expect, vi } from 'vitest';
import { render } from '../testUtils';
import { byText, click, flush, installApi, key, makeItem } from './testHelpers';
import { InventoryPage } from '@/pages/InventoryPage';
import { ItemDetailsDrawer } from '@/components/domain/ItemDetailsDrawer';

const bills = (n: number, start = 1) =>
  Array.from({ length: n }, (_, i) => ({
    billId: start + i,
    transactionCode: `SAL-${1000 + start + i}`,
    transactionDate: '2026-11-01',
    partyName: i === 0 ? null : 'Asha',
  }));

const rows = [
  makeItem({ itemId: 1, name: 'Notebook' }),
  makeItem({ itemId: 2, name: 'Gel Pen' }),
  makeItem({ itemId: 3, name: 'Lamination', itemType: 'SERVICE', currentStock: 0 }),
];

describe('ItemDetailsDrawer tabs', () => {
  function mount(over: Record<string, unknown> = {}) {
    const api = installApi({
      getItemSales: vi.fn().mockImplementation(async (_id: number, page: number) => ({
        items: page === 1 ? bills(10) : bills(2, 11),
        total: 12,
        page,
        limit: 10,
      })),
      getItemPurchases: vi.fn().mockResolvedValue({
        items: [{ billId: 5, transactionCode: 'PUR-1005', transactionDate: '2026-10-01', partyName: 'Vendor Co' }],
        total: 1,
        page: 1,
        limit: 10,
      }),
      getSaleDetails: vi.fn().mockResolvedValue(undefined),
      getPurchaseDetails: vi.fn().mockResolvedValue(undefined),
      getBillEditGuard: vi.fn().mockResolvedValue(undefined),
      ...over,
    });
    const onEdit = vi.fn();
    const r = render(<ItemDetailsDrawer item={rows[0]} onClose={() => {}} onEdit={onEdit} />);
    return { api, onEdit, ...r };
  }

  it('shows the summary, loads tabs lazily and pages sales 10 at a time', async () => {
    const { api } = mount();
    await flush();
    const body = document.body.textContent ?? '';
    for (const l of ['Stock', 'Status', 'MRP', 'Sale Price', 'Low Stock Level']) expect(body).toContain(l);
    expect(api.getItemLots).toHaveBeenCalled();
    expect(api.getItemSales).not.toHaveBeenCalled();
    click(byText(document.body, 'button', 'Sales'));
    await flush();
    expect(api.getItemSales).toHaveBeenCalledWith(1, 1, 10);
    expect(api.getItemPurchases).not.toHaveBeenCalled();
    expect(document.querySelectorAll('[data-testid="bill-row"]').length).toBe(10);
    expect(document.body.textContent).toContain('Walk-in');
    click(document.querySelector('[aria-label="Next page"]') as HTMLElement);
    await flush();
    expect(api.getItemSales).toHaveBeenLastCalledWith(1, 2, 10);
    expect(document.querySelectorAll('[data-testid="bill-row"]').length).toBe(2);
  });

  it('View opens the matching bill drawer; Purchases tab shows vendors; Edit calls back', async () => {
    const { api, onEdit } = mount();
    await flush();
    click(byText(document.body, 'button', 'Purchases'));
    await flush();
    expect(api.getItemPurchases).toHaveBeenCalledWith(1, 1, 10);
    expect(document.body.textContent).toContain('Vendor Co');
    click(document.querySelector('[aria-label="View PUR-1005"]') as HTMLElement);
    await flush();
    expect(api.getPurchaseDetails).toHaveBeenCalledWith(5);
    expect(api.getSaleDetails).not.toHaveBeenCalled();
    click(byText(document.body, 'button', 'Edit'));
    expect(onEdit).toHaveBeenCalledWith(rows[0]);
  });

  it('shows the backend error inside the tab', async () => {
    mount({ getItemSales: vi.fn().mockRejectedValue({ type: 'NotFound', details: { entity: 'Item', id: '1' } }) });
    await flush();
    click(byText(document.body, 'button', 'Sales'));
    await flush();
    expect(document.querySelector('[role="alert"]')).not.toBeNull();
  });
});

describe('Inventory keyboard map (D1)', () => {
  function mount(over: Record<string, unknown> = {}) {
    const api = installApi({
      getItemsCatalog: vi.fn().mockResolvedValue({ items: rows, total: 120, page: 1, limit: 50 }),
      getItemSales: vi.fn().mockResolvedValue({ items: [], total: 0, page: 1, limit: 10 }),
      getItemPurchases: vi.fn().mockResolvedValue({ items: [], total: 0, page: 1, limit: 10 }),
      ...over,
    });
    const r = render(<InventoryPage />);
    return { api, ...r };
  }
  const press = (k: string, target: Element | Window = document.body) => key(target, { key: k });
  const active = (c: HTMLElement) =>
    Array.from(c.querySelectorAll('[data-testid="item-row"]')).findIndex(
      (r) => r.getAttribute('aria-selected') === 'true'
    );

  it('Up/Down/Home/End move the highlighted row; Down from search enters, Up on row 0 returns', async () => {
    const { container } = mount();
    await flush();
    const search = container.querySelector('input[aria-label="Search items"]') as HTMLInputElement;
    search.focus();
    press('ArrowDown', search);
    await flush();
    expect(active(container)).toBe(0);
    expect(document.activeElement).not.toBe(search);
    press('ArrowDown');
    expect(active(container)).toBe(1);
    press('End');
    expect(active(container)).toBe(2);
    press('ArrowDown');
    expect(active(container)).toBe(2);
    press('Home');
    expect(active(container)).toBe(0);
    press('ArrowUp');
    expect(active(container)).toBe(-1);
    expect(document.activeElement).toBe(search);
  });

  it('Enter and a row click open Item details; Right/Left expand and collapse the lots row', async () => {
    const { container } = mount();
    await flush();
    press('ArrowDown');
    press('ArrowRight');
    await flush();
    expect(container.querySelectorAll('[data-testid="lot-expansion"]').length).toBe(1);
    press('ArrowLeft');
    await flush();
    expect(container.querySelectorAll('[data-testid="lot-expansion"]').length).toBe(0);
    press('Enter');
    await flush();
    expect(document.querySelector('[data-testid="item-details"]')?.textContent).toContain('Sale Price');
    click(document.querySelector('[aria-label="Close"]') as HTMLElement);
    await flush();
    expect(document.querySelector('[data-testid="item-details"]')).toBeNull();
    click(container.querySelectorAll('[data-testid="item-row"]')[1]);
    await flush();
    expect(active(container)).toBe(1);
    expect(document.body.textContent).toContain('Gel Pen');
    expect(document.querySelector('[data-testid="item-details"]')).not.toBeNull();
  });

  it('E opens Edit; S opens Set stock for PHYSICAL items only', async () => {
    mount();
    await flush();
    press('ArrowDown');
    press('s');
    await flush();
    expect(document.body.textContent).toContain('Set stock on hand');
    click(byText(document.body, 'button', /Cancel/));
    await flush();
    press('e');
    await flush();
    expect(document.body.textContent).toMatch(/Edit Item|Save Changes|Update Item/i);
  });

  it('S is ignored on a SERVICE row; PageDown/PageUp change page; typing in inputs and buttons is ignored', async () => {
    const { api, container } = mount();
    await flush();
    press('End');
    press('s');
    await flush();
    expect(document.body.textContent).not.toContain('Set stock on hand');
    press('PageDown');
    await flush();
    expect(api.getItemsCatalog).toHaveBeenLastCalledWith(expect.objectContaining({ page: 2 }));
    press('PageUp');
    await flush();
    expect(api.getItemsCatalog).toHaveBeenLastCalledWith(expect.objectContaining({ page: 1 }));
    const before = active(container);
    const search = container.querySelector('input[aria-label="Search items"]') as HTMLInputElement;
    press('End', search);
    press('e', search);
    press('ArrowDown', container.querySelector('button') as HTMLElement);
    expect(active(container)).toBe(before);
  });

  it('keeps the active row on the same item when the list refreshes', async () => {
    const { api, container } = mount();
    await flush();
    press('ArrowDown');
    press('ArrowDown');
    expect(active(container)).toBe(1);
    (api.getItemsCatalog as ReturnType<typeof vi.fn>).mockResolvedValue({
      items: [rows[2], rows[1], rows[0]],
      total: 3,
      page: 1,
      limit: 50,
    });
    click(byText(container, 'button', /Refresh/));
    await flush();
    expect(active(container)).toBe(1);
    expect(container.querySelectorAll('[data-testid="item-row"]')[1].textContent).toContain('Gel Pen');
    (api.getItemsCatalog as ReturnType<typeof vi.fn>).mockResolvedValue({
      items: [rows[2], rows[0]],
      total: 2,
      page: 1,
      limit: 50,
    });
    click(byText(container, 'button', /Refresh/));
    await flush();
    expect(active(container)).toBe(0);
  });
});

describe('Low Stock Level edit availability (FR-50)', () => {
  it('a physical item has an Edit button; a service and a deleted item are read-only', () => {
    installApi({});
    const view = (item: ReturnType<typeof makeItem>) => {
      const r = render(<ItemDetailsDrawer item={item} onClose={() => {}} onEdit={() => {}} />);
      const has = document.querySelector(`button[aria-label="Edit low stock level for ${item.name}"]`) !== null;
      r.unmount();
      return has;
    };
    expect(view(rows[0])).toBe(true);
    expect(view(rows[2])).toBe(false);
    expect(view(makeItem({ itemId: 4, name: 'Old Pen', isActive: false }))).toBe(false);
  });
});
