import { describe, it, expect, vi, beforeEach } from 'vitest';
import { act } from 'react';
import { render } from '../testUtils';
import { click, flush, installApi, makeItem, typeInto } from '../inventory/testHelpers';
import { RapidBillingRow } from '@/components/domain/RapidBillingRow';
import { NewPurchasePage } from '@/pages/NewPurchasePage';
import { __resetHotkeys } from '@/hooks/useHotkeys';

const doc = () => document.body;
const byTid = (id: string) => doc().querySelector<HTMLInputElement>(`[data-testid="${id}"]`);
const modalText = () => doc().querySelector('[role="dialog"]')?.textContent ?? '';
const saveBtn = () =>
  Array.from(doc().querySelectorAll<HTMLButtonElement>('[role="dialog"] button')).find((b) =>
    (b.textContent ?? '').trim().startsWith('Save')
  ) as HTMLButtonElement;
const keyOn = (el: Element | Window, k: string, init: KeyboardEventInit = {}) =>
  act(() => {
    el.dispatchEvent(new KeyboardEvent('keydown', { key: k, bubbles: true, cancelable: true, ...init }));
  });

const created = makeItem({ itemId: 9, name: 'Zed Marker', category: null, barcode: null, currentStock: 12, defaultSalePrice: 20 });

function setup() {
  let made = false;
  return installApi({
    searchItemsFts: vi.fn().mockImplementation(async (q: string) => (made && q === 'Zed Marker' ? [created] : [])),
    createItem: vi.fn().mockImplementation(async () => {
      made = true;
      return 9;
    }),
  });
}

async function typeSearch(container: HTMLElement, text: string) {
  const input = container.querySelector('[data-testid="item-search-input"]') as HTMLInputElement;
  typeInto(input, text);
  await flush(300);
  return input;
}

async function fillSale(price: string, stock?: string, cost?: string) {
  typeInto(byTid('quick-item-price') as HTMLInputElement, price);
  if (stock !== undefined) typeInto(byTid('quick-item-stock') as HTMLInputElement, stock);
  if (cost !== undefined) typeInto(byTid('quick-item-cost') as HTMLInputElement, cost);
}

describe('ItemQuickCreateModal on the sale screen (FR-24)', () => {
  beforeEach(() => {
    document.body.innerHTML = '';
    __resetHotkeys();
  });

  it('no match shows Create; Enter opens the modal pre-filled; sale mode requires stock and price', async () => {
    const api = setup();
    const { container } = render(<RapidBillingRow />);
    await flush();
    const input = await typeSearch(container, 'Zed Marker');
    expect(container.querySelector('[data-create-option]')?.textContent).toContain('Create "Zed Marker"');

    keyOn(input, 'Enter');
    await flush();
    expect((byTid('quick-item-name') as HTMLInputElement).value).toBe('Zed Marker');
    expect(byTid('quick-item-stock')).not.toBeNull();
    expect(byTid('quick-item-cost')).not.toBeNull();

    // price only -> stock and purchase price are required
    typeInto(byTid('quick-item-price') as HTMLInputElement, '20');
    click(saveBtn());
    await flush();
    expect(api.createItem).not.toHaveBeenCalled();
    expect(modalText()).toContain('Stock on hand must be greater than 0');
    expect(modalText()).toContain('Purchase price must be greater than 0');

    await fillSale('20', '12', '8.5');
    click(saveBtn());
    await flush(20);
    expect(api.createItem).toHaveBeenCalledWith(
      expect.objectContaining({
        name: 'Zed Marker',
        category: null,
        barcode: null,
        defaultSalePrice: 20,
        initialStock: 12,
        initialCost: 8.5,
        lowStockThreshold: 5,
        bundles: null,
      })
    );
    // line is filled and Qty is focused
    expect((container.querySelector('[data-testid="item-search-input"]') as HTMLInputElement).value).toBe('Zed Marker');
    expect(container.querySelector('[data-testid="stock-indicator"]')).not.toBeNull();
    await flush(10);
    expect(document.activeElement).toBe(container.querySelector('[data-testid="quantity-input"]'));
    expect(doc().querySelector('[role="dialog"]')).toBeNull();
  });

  it('Alt+N opens the modal; Esc closes it without creating or completing the bill', async () => {
    const api = setup();
    const onComplete = vi.fn();
    const { container } = render(<RapidBillingRow onCompleteBill={onComplete} />);
    await flush();
    keyOn(window, 'n', { altKey: true, code: 'KeyN' });
    await flush();
    expect(byTid('quick-item-name')).not.toBeNull();

    // Enter inside the modal submits the modal only (validation fails here), never the bill
    keyOn(byTid('quick-item-name') as Element, 'Enter');
    await flush();
    expect(onComplete).not.toHaveBeenCalled();
    expect(api.createItem).not.toHaveBeenCalled();

    keyOn(window, 'Escape');
    await flush(10);
    expect(doc().querySelector('[role="dialog"]')).toBeNull();
    expect(api.createItem).not.toHaveBeenCalled();
    expect(container.querySelector('[data-testid="stock-indicator"]')).toBeNull();
  });
});

describe('ItemQuickCreateModal on the purchase screen', () => {
  beforeEach(() => {
    document.body.innerHTML = '';
    __resetHotkeys();
  });

  it('hides the stock fields, sends no opening stock and selects the item', async () => {
    const api = setup();
    const { container } = render(<NewPurchasePage />);
    await flush();
    const input = container.querySelector('[data-testid="purchase-item-search"] input') as HTMLInputElement;
    typeInto(input, 'Zed Marker');
    await flush(300);
    click(container.querySelector('[data-create-option]') as HTMLElement);
    await flush();

    expect((byTid('quick-item-name') as HTMLInputElement).value).toBe('Zed Marker');
    expect(byTid('quick-item-stock')).toBeNull();
    expect(byTid('quick-item-cost')).toBeNull();

    typeInto(byTid('quick-item-price') as HTMLInputElement, '20');
    click(saveBtn());
    await flush(20);
    expect(api.createItem).toHaveBeenCalledWith(
      expect.objectContaining({ name: 'Zed Marker', initialStock: null, initialCost: null, category: null, barcode: null })
    );
    expect((container.querySelector('[data-testid="item-search-input"]') as HTMLInputElement).value).toBe('Zed Marker');
  });

  it('Alt+N opens the modal', async () => {
    setup();
    render(<NewPurchasePage />);
    await flush();
    keyOn(window, 'n', { altKey: true, code: 'KeyN' });
    await flush();
    expect(byTid('quick-item-name')).not.toBeNull();
  });
});
