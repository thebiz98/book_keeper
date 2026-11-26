import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render } from '../testUtils';
import { byText, click, flush, installApi, makeItem, typeInto, inputByLabel } from './testHelpers';
import { __resetHotkeys } from '@/hooks/useHotkeys';
import { InventoryPage } from '@/pages/InventoryPage';
import { ItemDetailsDrawer } from '@/components/domain/ItemDetailsDrawer';
import type { ItemDeletePreview } from '@/types';

const pen = makeItem({ itemId: 1, name: 'Gel Pen', currentStock: 12 });

const soft: ItemDeletePreview = {
  itemId: 1,
  name: 'Gel Pen',
  itemType: 'PHYSICAL',
  stock: 12,
  baseUnit: 'PCS',
  stockValue: 840,
  blockingServices: [],
  canDelete: true,
  reason: null,
  mode: 'SOFT',
  reasonsUsed: ['sold in 2 bill(s)'],
};

function mount(over: Record<string, unknown> = {}) {
  const api = installApi({
    getItemsCatalog: vi.fn().mockResolvedValue({ items: [pen], total: 1, page: 1, limit: 50 }),
    previewItemDelete: vi.fn().mockResolvedValue(soft),
    deleteItem: vi.fn().mockResolvedValue({ mode: 'SOFT' }),
    restoreItem: vi.fn().mockResolvedValue(undefined),
    listDeletedItems: vi.fn().mockResolvedValue({ items: [], total: 0, page: 1, limit: 10 }),
    ...over,
  });
  const r = render(<InventoryPage />);
  return { api, ...r };
}

async function openEdit(container: HTMLElement) {
  await flush();
  click(container.querySelector('button[aria-label="Edit Gel Pen"]') as HTMLElement);
  await flush();
}

beforeEach(() => {
  document.body.innerHTML = '';
  __resetHotkeys();
});

describe('Delete item (FR-45)', () => {
  it('shows the backend preview, deletes on confirm, closes the drawer and shows the notice', async () => {
    const { api, container } = mount();
    await openEdit(container);
    click(container.querySelector('[data-testid="item-delete-button"]') as HTMLElement);
    await flush();
    expect(api.previewItemDelete).toHaveBeenCalledWith(1);
    const text = container.ownerDocument.body.querySelector('[data-testid="delete-soft-text"]')?.textContent ?? '';
    expect(text).toContain('12 PCS');
    expect(text).toContain('₹840.00');
    expect(document.body.textContent).toContain('moved to Deleted items');
    // No stock to write off (writeOff null): no choice is offered.
    expect(document.body.querySelector('[data-testid="delete-writeoff-choice"]')).toBeNull();
    click(document.body.querySelector('[data-testid="delete-confirm-submit"]') as HTMLElement);
    await flush();
    expect(api.deleteItem).toHaveBeenCalledWith(1, false);
    expect(document.body.querySelector('[data-testid="inventory-notice"]')?.textContent).toContain(
      "Deleted 'Gel Pen'. Restore it from Deleted items."
    );
  });

  it('a blocked delete disables the button and shows the backend reason', async () => {
    const reason = "'Gel Pen' is a component of: Print. Remove it from those services first.";
    const { api, container } = mount({
      previewItemDelete: vi.fn().mockResolvedValue({ ...soft, canDelete: false, blockingServices: ['Print'], reason }),
    });
    await openEdit(container);
    click(container.querySelector('[data-testid="item-delete-button"]') as HTMLElement);
    await flush();
    const submit = document.body.querySelector('[data-testid="delete-confirm-submit"]') as HTMLButtonElement;
    expect(submit.disabled).toBe(true);
    expect(document.body.querySelector('[data-testid="delete-confirm-blocked"]')?.textContent).toBe(reason);
    click(submit);
    await flush();
    expect(api.deleteItem).not.toHaveBeenCalled();
  });

  it('a never-used item is described as permanent and the notice says so', async () => {
    const { api, container } = mount({
      previewItemDelete: vi.fn().mockResolvedValue({ ...soft, mode: 'HARD', reasonsUsed: [] }),
      deleteItem: vi.fn().mockResolvedValue({ mode: 'HARD' }),
    });
    await openEdit(container);
    click(container.querySelector('[data-testid="item-delete-button"]') as HTMLElement);
    await flush();
    expect(document.body.querySelector('[data-testid="delete-hard-text"]')?.textContent).toContain(
      "'Gel Pen' was never used. It will be removed permanently (12 PCS opening stock, ₹840.00)."
    );
    const submit = document.body.querySelector('[data-testid="delete-confirm-submit"]') as HTMLElement;
    expect(submit.textContent).toContain('Delete permanently');
    click(submit);
    await flush();
    expect(api.deleteItem).toHaveBeenCalledWith(1, false);
    expect(document.body.querySelector('[data-testid="inventory-notice"]')?.textContent).toContain('permanently');
  });
});

describe('Delete item with stock: Just delete or Book as loss (FR-58)', () => {
  const withStock: ItemDeletePreview = { ...soft, writeOff: { quantity: 12, baseUnit: 'PCS', loss: 790 } };

  async function openDelete() {
    const r = mount({ previewItemDelete: vi.fn().mockResolvedValue(withStock) });
    await openEdit(r.container);
    click(r.container.querySelector('[data-testid="item-delete-button"]') as HTMLElement);
    await flush();
    return r;
  }

  it('pre-selects Just delete, explains it and sends writeOff=false', async () => {
    const { api } = await openDelete();
    const just = document.body.querySelector('[data-testid="delete-just-delete"]') as HTMLInputElement;
    const loss = document.body.querySelector('[data-testid="delete-book-loss"]') as HTMLInputElement;
    expect(just.checked).toBe(true);
    expect(loss.checked).toBe(false);
    const choice = document.body.querySelector('[data-testid="delete-writeoff-choice"]')?.textContent ?? '';
    expect(choice).toContain('the item was a mistake and you will re-add it');
    // The loss figure is the backend's dry-run value, not stock x price.
    expect(choice).toContain('Book as loss (₹790.00)');
    click(document.body.querySelector('[data-testid="delete-confirm-submit"]') as HTMLElement);
    await flush();
    expect(api.deleteItem).toHaveBeenCalledWith(1, false);
  });

  it('Book as loss sends writeOff=true', async () => {
    const { api } = await openDelete();
    click(document.body.querySelector('[data-testid="delete-book-loss"]') as HTMLElement);
    await flush();
    expect((document.body.querySelector('[data-testid="delete-book-loss"]') as HTMLInputElement).checked).toBe(true);
    click(document.body.querySelector('[data-testid="delete-confirm-submit"]') as HTMLElement);
    await flush();
    expect(api.deleteItem).toHaveBeenCalledWith(1, true);
  });
});

describe('Deleted items dialog', () => {
  const row = (id: number, name: string, canPurge = false) => ({
    itemId: id,
    name,
    itemType: 'PHYSICAL' as const,
    category: null,
    baseUnit: 'PCS',
    currentStock: 4,
    stockValue: 10,
    deletedAt: '2026-11-25T10:00:00Z',
    canPurge,
  });

  it('lists with backend paging, searches by name and restores a row', async () => {
    const listDeletedItems = vi
      .fn()
      .mockImplementation(async (s: string | null) => ({
        items: s ? [row(7, 'Old pen')] : [row(7, 'Old pen'), row(8, 'Old ink', true)],
        total: s ? 1 : 2,
        page: 1,
        limit: 10,
      }));
    const { api, container } = mount({ listDeletedItems });
    await flush();
    click(container.querySelector('[data-testid="deleted-items-button"]') as HTMLElement);
    await flush();
    expect(listDeletedItems).toHaveBeenCalledWith('', 1, 10);
    expect(document.body.querySelectorAll('[data-testid="deleted-row"]').length).toBe(2);
    expect(document.body.textContent).toContain('Deleted on');
    // "Delete permanently" is offered only where the backend says the item was never used.
    expect(document.body.querySelector('[data-testid="deleted-extra-7"]')).toBeNull();
    expect(document.body.querySelector('[data-testid="deleted-extra-8"]')).not.toBeNull();

    typeInto(document.body.querySelector('[data-testid="deleted-records-search"]') as HTMLInputElement, 'pen');
    await flush(400);
    expect(listDeletedItems).toHaveBeenLastCalledWith('pen', 1, 10);
    expect(document.body.querySelectorAll('[data-testid="deleted-row"]').length).toBe(1);

    click(document.body.querySelector('[data-testid="deleted-restore-7"]') as HTMLElement);
    await flush();
    expect(api.restoreItem).toHaveBeenCalledWith(7);
    expect(api.getItemsCatalog.mock.calls.length).toBeGreaterThan(1);
  });
});

describe('Create form, deleted name', () => {
  it('offers Restore on ITEM_NAME_DELETED, restores and opens the item in edit', async () => {
    const clash = {
      type: 'DeletedNameClash',
      details: { field: 'name', code: 'ITEM_NAME_DELETED', existing_id: 9, message: "'Gel Pen' was deleted. Restore it from Deleted items instead of creating it again." },
    };
    const restored = makeItem({ itemId: 9, name: 'Gel Pen' });
    const { api, container } = mount({
      createItem: vi.fn().mockRejectedValue(clash),
      getItem: vi.fn().mockResolvedValue(restored),
    });
    await flush();
    click(byText(container, 'button', /Add Item/));
    await flush();
    typeInto(inputByLabel(container, /Item Name/), 'Gel Pen');
    typeInto(inputByLabel(container, /Sale Price/), '10');
    click(byText(container, 'button', /Save Item/));
    await flush();
    expect(container.textContent).toContain('was deleted');
    click(container.querySelector('[data-testid="item-restore-clash"]') as HTMLElement);
    await flush();
    expect(api.restoreItem).toHaveBeenCalledWith(9);
    expect(api.getItem).toHaveBeenCalledWith(9);
    // The drawer is now the edit drawer of the restored item.
    expect(container.textContent).toContain('Edit Item: Gel Pen');
  });
});

describe('Details drawer of a deleted item', () => {
  it('is read-only: Deleted badge, no Edit and no Restore (Restore lives in Deleted items)', async () => {
    installApi({});
    render(<ItemDetailsDrawer item={makeItem({ itemId: 5, name: 'Old pen', isActive: false })} onClose={vi.fn()} onEdit={vi.fn()} />);
    await flush();
    expect(document.body.querySelector('[data-testid="item-deleted-badge"]')?.textContent).toBe('Deleted');
    expect(Array.from(document.body.querySelectorAll('button')).some((b) => /^Edit$/.test((b.textContent ?? '').trim()))).toBe(false);
    expect(Array.from(document.body.querySelectorAll('button')).some((b) => /Restore/.test(b.textContent ?? ''))).toBe(false);
  });
});
