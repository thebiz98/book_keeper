import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { act } from 'react';
import { render } from '../testUtils';
import { ItemSearchTypeahead } from '@/components/domain/ItemSearchTypeahead';
import { __resetHotkeys } from '@/hooks/useHotkeys';
import { installApi, makeItem, typeInto, press, flush } from './saleFixtures';
import type { ItemSearchResult } from '@/types';

function deferred<T>() {
  let resolve!: (v: T) => void;
  const promise = new Promise<T>((r) => (resolve = r));
  return { promise, resolve };
}

const input = () => document.querySelector<HTMLInputElement>('input')!;
const options = () => Array.from(document.querySelectorAll('[role="option"]')).map((o) => o.textContent ?? '');

describe('ItemSearchTypeahead', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    __resetHotkeys();
  });
  afterEach(() => vi.useRealTimers());

  it('debounces 50 ms, searches from 1 character with limit 8', async () => {
    const api = installApi({ searchItemsFts: vi.fn(async () => [makeItem()]) });
    render(<ItemSearchTypeahead onSelectItem={vi.fn()} />);
    typeInto(input(), 'p');
    await flush(49);
    expect(api.searchItemsFts).not.toHaveBeenCalled();
    await flush(1);
    expect(api.searchItemsFts).toHaveBeenCalledWith('p', 8);
    expect(options()[0]).toContain('Apsara Pencil');
  });

  it('ignores a stale response that resolves after a newer query', async () => {
    const slow = deferred<ItemSearchResult[]>();
    const search = vi.fn((q: string) =>
      q === 'pen' ? slow.promise : Promise.resolve([makeItem({ itemId: 2, name: 'Pencil Box' })])
    );
    installApi({ searchItemsFts: search });
    render(<ItemSearchTypeahead onSelectItem={vi.fn()} />);

    typeInto(input(), 'pen');
    await flush(50);
    typeInto(input(), 'pencil');
    await flush(50);
    expect(options()).toHaveLength(1);
    expect(options()[0]).toContain('Pencil Box');

    await act(async () => {
      slow.resolve([makeItem({ itemId: 1, name: 'Pen Old' })]);
    });
    expect(options()).toHaveLength(1);
    expect(options()[0]).toContain('Pencil Box');
  });

  it('Enter while the debounce is still pending runs the search and selects the top match', async () => {
    const onSelect = vi.fn();
    const onSelectItem = vi.fn();
    const search = vi.fn(async () => [makeItem({ itemId: 9, name: 'Scanned' }), makeItem({ itemId: 10, name: 'Other' })]);
    installApi({ searchItemsFts: search });
    render(<ItemSearchTypeahead onSelectItem={onSelectItem} onSelect={onSelect} />);

    typeInto(input(), '8901030381023');
    press(input(), 'Enter'); // scanner: digits then Enter within a few ms
    await flush(0);
    expect(search).toHaveBeenCalledTimes(1);
    expect(onSelectItem).toHaveBeenCalledTimes(1);
    expect(onSelectItem.mock.calls[0][0].itemId).toBe(9);
    expect(input().value).toBe(''); // cleared for the next scan
    await flush(100);
    expect(search).toHaveBeenCalledTimes(1); // debounce did not fire a second search
  });

  it('Enter while the search is in flight awaits it (no early no-op, no duplicate request)', async () => {
    const d = deferred<ItemSearchResult[]>();
    const search = vi.fn(() => d.promise);
    installApi({ searchItemsFts: search });
    const onSelectItem = vi.fn();
    render(<ItemSearchTypeahead onSelectItem={onSelectItem} />);

    typeInto(input(), 'pencil');
    await flush(50);
    expect(search).toHaveBeenCalledTimes(1);
    press(input(), 'Enter');
    expect(onSelectItem).not.toHaveBeenCalled();
    await act(async () => {
      d.resolve([makeItem({ itemId: 3, name: 'Late' })]);
    });
    expect(search).toHaveBeenCalledTimes(1);
    expect(onSelectItem.mock.calls[0][0].itemId).toBe(3);
  });

  it('Enter picks the bundle-barcode hit and exposes matchedBundleId', async () => {
    const onSelect = vi.fn();
    installApi({
      searchItemsFts: vi.fn(async () => [
        makeItem({ itemId: 1, name: 'Name match' }),
        makeItem({ itemId: 2, name: 'Carton', matchedBundleId: 44 }),
      ]),
    });
    render(<ItemSearchTypeahead onSelectItem={vi.fn()} onSelect={onSelect} />);
    typeInto(input(), '555');
    press(input(), 'Enter');
    await flush(0);
    expect(onSelect).toHaveBeenCalledTimes(1);
    expect(onSelect.mock.calls[0][0].itemId).toBe(2);
    expect(onSelect.mock.calls[0][1]).toEqual({ matchedBundleId: 44 });
  });

  it('arrow keys move the highlight, Enter selects it, Esc closes the list', async () => {
    const onSelectItem = vi.fn();
    installApi({
      searchItemsFts: vi.fn(async () => [makeItem({ itemId: 1, name: 'A' }), makeItem({ itemId: 2, name: 'B' })]),
    });
    render(<ItemSearchTypeahead onSelectItem={onSelectItem} />);
    typeInto(input(), 'x');
    await flush(50);
    press(input(), 'ArrowDown');
    expect(document.querySelectorAll('[role="option"]')[1].getAttribute('aria-selected')).toBe('true');
    press(input(), 'ArrowUp');
    expect(document.querySelectorAll('[role="option"]')[0].getAttribute('aria-selected')).toBe('true');
    press(input(), 'ArrowDown');
    press(input(), 'Enter');
    await flush(0);
    expect(onSelectItem.mock.calls[0][0].itemId).toBe(2);

    typeInto(input(), 'y');
    await flush(50);
    expect(options().length).toBeGreaterThan(0);
    press(window, 'Escape');
    expect(options()).toHaveLength(0);
  });

  it('does not flash the empty state while loading, shows it only after an empty result', async () => {
    const d = deferred<ItemSearchResult[]>();
    installApi({ searchItemsFts: vi.fn(() => d.promise) });
    render(<ItemSearchTypeahead onSelectItem={vi.fn()} />);
    typeInto(input(), 'zzz');
    await flush(50);
    expect(document.body.textContent).not.toContain('No matching items');
    expect(document.querySelector('[aria-label="Searching"]')).toBeNull(); // spinner is delayed
    await flush(150);
    expect(document.querySelector('[aria-label="Searching"]')).not.toBeNull();
    await act(async () => d.resolve([]));
    expect(document.body.textContent).toContain('No matching items');
    expect(document.querySelector('[aria-label="Searching"]')).toBeNull();
  });

  it('Enter on an empty query calls onEmptyEnter', () => {
    installApi();
    const onEmptyEnter = vi.fn();
    render(<ItemSearchTypeahead onSelectItem={vi.fn()} onEmptyEnter={onEmptyEnter} />);
    press(input(), 'Enter');
    expect(onEmptyEnter).toHaveBeenCalledTimes(1);
  });
});
