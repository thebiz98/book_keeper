import React, { act, useState } from 'react';
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render } from '../testUtils';
import {
  useLiveSearch,
  type UseLiveSearchOptions,
  type UseLiveSearchResult,
  SPINNER_DELAY_MS,
} from '@/hooks/useLiveSearch';

(globalThis as unknown as { IS_REACT_ACT_ENVIRONMENT: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

type R = UseLiveSearchResult<string[]>;

/** Mounts the hook and exposes its latest result through `out.current`. */
function mount(
  make: (page: number) => Partial<UseLiveSearchOptions<string[]>> & { fetcher: (q: string) => Promise<string[]> },
  extra?: { onPage?: (setPage: (n: number) => void) => void }
) {
  const out: { current: R; setPage: (n: number) => void } = {
    current: undefined as unknown as R,
    setPage: () => {},
  };
  const Probe: React.FC = () => {
    const [page, setPage] = useState(1);
    out.setPage = setPage;
    extra?.onPage?.(setPage);
    const o = make(page);
    out.current = useLiveSearch<string[]>({ initial: [], ...o });
    return null;
  };
  const r = render(<Probe />);
  return { out, ...r };
}

const ms = (n: number) => act(async () => { await vi.advanceTimersByTimeAsync(n); });

beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());

describe('useLiveSearch', () => {
  it('does not call before debounceMs and calls once after; keystrokes coalesce', async () => {
    const fetcher = vi.fn(async (q: string) => [q]);
    const { out } = mount(() => ({ fetcher, debounceMs: 150 }));
    act(() => out.current.setQuery('p'));
    await ms(100);
    act(() => out.current.setQuery('pe'));
    await ms(100);
    act(() => out.current.setQuery('pen'));
    await ms(149);
    expect(fetcher).not.toHaveBeenCalled();
    await ms(2);
    expect(fetcher).toHaveBeenCalledTimes(1);
    expect(fetcher).toHaveBeenCalledWith('pen');
    expect(out.current.results).toEqual(['pen']);
    expect(out.current.resultsFor).toBe('pen');
  });

  it('minChars with "clear" resets results to initial without a call', async () => {
    const fetcher = vi.fn(async (q: string) => [q]);
    const { out } = mount(() => ({ fetcher, debounceMs: 10, minChars: 2 }));
    act(() => out.current.setQuery('ab'));
    await ms(20);
    expect(out.current.results).toEqual(['ab']);
    act(() => out.current.setQuery('a'));
    await ms(20);
    expect(out.current.results).toEqual([]);
    expect(out.current.resultsFor).toBeNull();
    expect(fetcher).toHaveBeenCalledTimes(1);
  });

  it('emptyBehavior "fetch" calls the fetcher with an empty query on mount and when cleared', async () => {
    const fetcher = vi.fn(async (q: string) => [`all:${q}`]);
    const { out } = mount(() => ({ fetcher, debounceMs: 10, emptyBehavior: 'fetch' }));
    await ms(0);
    expect(fetcher).toHaveBeenCalledWith('');
    expect(out.current.results).toEqual(['all:']);
    act(() => out.current.setQuery('x'));
    await ms(20);
    expect(out.current.results).toEqual(['all:x']);
    act(() => out.current.setQuery(''));
    await ms(20);
    expect(out.current.results).toEqual(['all:']);
  });

  it('only the latest request applies when an older one resolves later', async () => {
    const resolvers: Record<string, (v: string[]) => void> = {};
    const fetcher = vi.fn((q: string) => new Promise<string[]>((res) => { resolvers[q] = res; }));
    const { out } = mount(() => ({ fetcher, debounceMs: 10 }));
    act(() => out.current.setQuery('a'));
    await ms(20);
    act(() => out.current.setQuery('ab'));
    await ms(20);
    await act(async () => { resolvers['ab'](['B']); });
    await act(async () => { resolvers['a'](['A']); });
    expect(out.current.results).toEqual(['B']);
    expect(out.current.resultsFor).toBe('ab');
  });

  it('a deps change refetches immediately with the committed query', async () => {
    const fetcher = vi.fn(async (q: string) => [q]);
    const { out } = mount((page) => ({ fetcher, debounceMs: 10, deps: [page] }));
    act(() => out.current.setQuery('pen'));
    await ms(20);
    expect(fetcher).toHaveBeenCalledTimes(1);
    act(() => out.setPage(2));
    await ms(0);
    expect(fetcher).toHaveBeenCalledTimes(2);
    expect(fetcher).toHaveBeenLastCalledWith('pen');
  });

  it('onQueryCommit + setPage in the same tick gives a single fetch', async () => {
    const fetcher = vi.fn(async (q: string) => [q]);
    const ref: { setPage: (n: number) => void } = { setPage: () => {} };
    const { out } = mount(
      (page) => ({ fetcher, debounceMs: 10, deps: [page], onQueryCommit: () => ref.setPage(1) }),
      { onPage: (sp) => { ref.setPage = sp; } }
    );
    act(() => out.setPage(3));
    await ms(0);
    fetcher.mockClear();
    act(() => out.current.setQuery('pen'));
    await ms(20);
    expect(fetcher).toHaveBeenCalledTimes(1);
    expect(fetcher).toHaveBeenCalledWith('pen');
  });

  it('flush cancels the timer, dedupes the in-flight query and resolves with the results', async () => {
    let resolve: (v: string[]) => void = () => {};
    const fetcher = vi.fn(() => new Promise<string[]>((res) => { resolve = res; }));
    const { out } = mount(() => ({ fetcher, debounceMs: 150 }));
    act(() => out.current.setQuery('pen'));
    let p1!: Promise<string[]>;
    let p2!: Promise<string[]>;
    act(() => { p1 = out.current.flush(); });
    await ms(0);
    act(() => { p2 = out.current.flush(); });
    await ms(300);
    expect(fetcher).toHaveBeenCalledTimes(1);
    await act(async () => { resolve(['done']); });
    expect(await p1).toEqual(['done']);
    expect(await p2).toEqual(['done']);
  });

  it('exposes an error and clears it on the next success', async () => {
    let fail = true;
    const fetcher = vi.fn(async (q: string) => {
      if (fail) throw new Error('boom');
      return [q];
    });
    const { out } = mount(() => ({ fetcher, debounceMs: 10 }));
    act(() => out.current.setQuery('a'));
    await ms(20);
    expect((out.current.error as Error).message).toBe('boom');
    fail = false;
    act(() => out.current.setQuery('ab'));
    await ms(20);
    expect(out.current.error).toBeNull();
    expect(out.current.results).toEqual(['ab']);
  });

  it('applies no state update after unmount', async () => {
    let resolve: (v: string[]) => void = () => {};
    const fetcher = vi.fn(() => new Promise<string[]>((res) => { resolve = res; }));
    const errors = vi.spyOn(console, 'error').mockImplementation(() => {});
    const { out, unmount } = mount(() => ({ fetcher, debounceMs: 10 }));
    act(() => out.current.setQuery('a'));
    await ms(20);
    unmount();
    errors.mockClear();
    resolve(['late']);
    await vi.advanceTimersByTimeAsync(10);
    expect(errors).not.toHaveBeenCalled();
    errors.mockRestore();
  });

  it('showSpinner turns on only after SPINNER_DELAY_MS', async () => {
    let resolve: (v: string[]) => void = () => {};
    const fetcher = vi.fn(() => new Promise<string[]>((res) => { resolve = res; }));
    const { out } = mount(() => ({ fetcher, debounceMs: 10 }));
    act(() => out.current.setQuery('a'));
    await ms(20);
    expect(out.current.isLoading).toBe(true);
    expect(out.current.showSpinner).toBe(false);
    await ms(SPINNER_DELAY_MS + 5);
    expect(out.current.showSpinner).toBe(true);
    await act(async () => { resolve(['x']); });
    expect(out.current.showSpinner).toBe(false);
  });

  it('reset clears query and results and invalidates in-flight requests', async () => {
    let resolve: (v: string[]) => void = () => {};
    const fetcher = vi.fn(() => new Promise<string[]>((res) => { resolve = res; }));
    const { out } = mount(() => ({ fetcher, debounceMs: 10 }));
    act(() => out.current.setQuery('a'));
    await ms(20);
    act(() => out.current.reset());
    await act(async () => { resolve(['stale']); });
    expect(out.current.query).toBe('');
    expect(out.current.results).toEqual([]);
  });
});
