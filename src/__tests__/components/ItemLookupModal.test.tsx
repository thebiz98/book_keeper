import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render } from '../testUtils';
import { ItemLookupModal } from '@/App';
import { __resetHotkeys } from '@/hooks/useHotkeys';
import { installApi, makeItem, typeInto, press, flush } from './saleFixtures';

const input = () => document.querySelector<HTMLInputElement>('[data-testid="item-search-input"]')!;

describe('ItemLookupModal (FR-40)', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    __resetHotkeys();
  });
  afterEach(() => vi.useRealTimers());

  it('keeps the search field outside the scrolling results list and does not scroll the body', async () => {
    installApi({ searchItemsFts: vi.fn(async () => [makeItem({ itemId: 1, name: 'Pencil' }), makeItem({ itemId: 2, name: 'Pen' })]) });
    render(<ItemLookupModal isOpen onClose={vi.fn()} />);
    typeInto(input(), 'pen');
    await flush(60);
    const list = document.querySelector('[role="listbox"]')!;
    expect(list.getAttribute('data-results-mode')).toBe('inline');
    expect(list.className).toContain('overflow-y-auto');
    expect(list.contains(input())).toBe(false);
    expect(document.querySelector('[data-modal-body]')!.className).toContain('overflow-hidden');
    expect(document.querySelector('[role="dialog"]')!.className).toContain('max-w-lg');
  });

  it('selecting an item shows its card; typing again removes it', async () => {
    installApi({ searchItemsFts: vi.fn(async () => [makeItem({ itemId: 1, name: 'Pencil' })]) });
    render(<ItemLookupModal isOpen onClose={vi.fn()} />);
    typeInto(input(), 'pen');
    await flush(60);
    press(input(), 'Enter');
    await flush(0);
    expect(document.querySelector('[data-testid="lookup-result"]')!.textContent).toContain('Pencil');
    typeInto(input(), 'penc');
    await flush(60);
    expect(document.querySelector('[data-testid="lookup-result"]')).toBeNull();
  });

  it('a service result card shows Can make N, not Out of Stock', async () => {
    installApi({
      searchItemsFts: vi.fn(async () => [
        makeItem({ itemId: 7, name: 'Printing', itemType: 'SERVICE', currentStock: 0, canMake: 9 }),
      ]),
    });
    render(<ItemLookupModal isOpen onClose={vi.fn()} />);
    typeInto(input(), 'pri');
    await flush(60);
    press(input(), 'Enter');
    await flush(0);
    const card = document.querySelector('[data-testid="lookup-result"]')!;
    expect(card.textContent).toContain('Can make 9');
    expect(card.textContent).not.toContain('Out of Stock');
  });
});
