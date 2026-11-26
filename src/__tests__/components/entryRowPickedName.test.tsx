import { describe, it, expect, vi, beforeEach } from 'vitest';
import { act } from 'react';
import { render } from '../testUtils';
import { click, flush, installApi, makeItem, typeInto } from '../inventory/testHelpers';
import { RapidBillingRow } from '@/components/domain/RapidBillingRow';
import { NewPurchasePage } from '@/pages/NewPurchasePage';
import { __resetHotkeys } from '@/hooks/useHotkeys';

const pencil = makeItem({ itemId: 1, name: 'Pencil', currentStock: 50, defaultSalePrice: 5 });
const eraser = makeItem({ itemId: 2, name: 'Eraser', currentStock: 30, defaultSalePrice: 3 });

const keyOn = (el: Element, k: string) =>
  act(() => {
    el.dispatchEvent(new KeyboardEvent('keydown', { key: k, bubbles: true, cancelable: true }));
  });
const search = () => document.querySelector('[data-testid="item-search-input"]') as HTMLInputElement;
const listOpen = () => document.querySelector('[role="listbox"]') !== null;

function setup() {
  return installApi({
    searchItemsFts: vi.fn().mockImplementation(async (q: string) =>
      [pencil, eraser].filter((i) => i.name.toLowerCase().startsWith(q.toLowerCase()))
    ),
  });
}

async function pick(text: string) {
  typeInto(search(), text);
  await flush(300);
  keyOn(search(), 'Enter');
  await flush(50);
}

describe('New Sale entry row keeps the picked name (FR-48)', () => {
  beforeEach(() => {
    document.body.innerHTML = '';
    __resetHotkeys();
  });

  it('shows the name in the box with the list closed, and Enter on it moves to Qty', async () => {
    const api = setup();
    render(<RapidBillingRow />);
    await pick('pen');
    expect(search().value).toBe('Pencil');
    expect(listOpen()).toBe(false);
    const qty = document.querySelector('[data-testid="quantity-input"]') as HTMLInputElement;
    expect(qty.disabled).toBe(false);

    const calls = api.searchItemsFts.mock.calls.length;
    search().focus();
    keyOn(search(), 'Enter');
    await flush(300);
    expect(document.activeElement).toBe(qty);
    expect(api.searchItemsFts.mock.calls.length).toBe(calls);
  });

  it('editing the text un-picks the item; Enter picks the new match', async () => {
    render(<RapidBillingRow />);
    await pick('pen');
    typeInto(search(), 'era');
    await flush(10);
    expect(search().value).toBe('era');
    expect((document.querySelector('[data-testid="quantity-input"]') as HTMLInputElement).disabled).toBe(true);
    await flush(300);
    keyOn(search(), 'Enter');
    await flush(50);
    expect(search().value).toBe('Eraser');
  });

  it('clears the box after Add', async () => {
    const onAddLine = vi.fn(() => true);
    render(<RapidBillingRow onAddLine={onAddLine} />);
    await pick('pen');
    click(document.querySelector('[data-testid="add-item-btn"]') as HTMLElement);
    await flush(10);
    expect(onAddLine).toHaveBeenCalledWith(expect.objectContaining({ item: pencil }));
    expect(search().value).toBe('');
  });
});

describe('New Purchase entry row keeps the picked name (FR-48)', () => {
  beforeEach(() => {
    document.body.innerHTML = '';
    __resetHotkeys();
  });

  it('shows the name in the box; editing it un-picks', async () => {
    setup();
    render(<NewPurchasePage />);
    await flush();
    await pick('pen');
    expect(search().value).toBe('Pencil');
    expect(document.querySelector('[data-testid="selected-item"]')).not.toBeNull();
    typeInto(search(), 'x');
    await flush(10);
    expect(search().value).toBe('x');
    expect(document.querySelector('[data-testid="selected-item"]')).toBeNull();
  });
});
