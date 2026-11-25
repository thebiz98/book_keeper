import React, { act } from 'react';
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render } from '../testUtils';
import { usePreview, type UsePreviewOptions, type UsePreviewResult } from '@/hooks/usePreview';

(globalThis as unknown as { IS_REACT_ACT_ENVIRONMENT: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

interface Out {
  data: { n: number; errors?: { field: string; message: string }[] };
}
type R = UsePreviewResult<Out['data']>;

function mount(make: () => UsePreviewOptions<number, Out['data']>, strict = false) {
  const out: { current: R } = { current: undefined as unknown as R };
  const Probe: React.FC = () => {
    out.current = usePreview<number, Out['data']>(make());
    return null;
  };
  const ui = strict ? (
    <React.StrictMode>
      <Probe />
    </React.StrictMode>
  ) : (
    <Probe />
  );
  return { out, ...render(ui) };
}

const ms = (n: number) => act(async () => { await vi.advanceTimersByTimeAsync(n); });

beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());

describe('usePreview', () => {
  it('is loading from the first render until the debounced result applies', async () => {
    const fetcher = vi.fn(async (n: number) => ({ n }));
    const { out } = mount(() => ({ input: 4, fetcher, debounceMs: 80 }));
    expect(out.current.isLoading).toBe(true);
    expect(out.current.data).toBeNull();
    await ms(79);
    expect(fetcher).not.toHaveBeenCalled();
    await ms(2);
    expect(fetcher).toHaveBeenCalledWith(4);
    expect(out.current).toEqual({ data: { n: 4 }, errors: [], isLoading: false });
  });

  it('keystrokes coalesce: only the last input is fetched', async () => {
    const fetcher = vi.fn(async (n: number) => ({ n }));
    const Probe: React.FC<{ v: number }> = ({ v }) => {
      out.current = usePreview<number, Out['data']>({ input: v, fetcher, debounceMs: 80 });
      return null;
    };
    const out: { current: R } = { current: undefined as unknown as R };
    const { rerender } = render(<Probe v={1} />);
    await ms(40);
    rerender(<Probe v={2} />);
    await ms(40);
    rerender(<Probe v={3} />);
    await ms(79);
    expect(fetcher).not.toHaveBeenCalled();
    await ms(2);
    expect(fetcher).toHaveBeenCalledTimes(1);
    expect(fetcher).toHaveBeenCalledWith(3);
    expect(out.current.data).toEqual({ n: 3 });
    expect(out.current.isLoading).toBe(false);
  });

  it('only the latest request applies (a slow earlier response is dropped)', async () => {
    const resolvers: Array<(v: Out['data']) => void> = [];
    const fetcher = vi.fn((_n: number) => new Promise<Out['data']>((r) => resolvers.push(r)));
    const out: { current: R } = { current: undefined as unknown as R };
    const Probe: React.FC<{ v: number }> = ({ v }) => {
      out.current = usePreview<number, Out['data']>({ input: v, fetcher, debounceMs: 10 });
      return null;
    };
    const { rerender } = render(<Probe v={1} />);
    await ms(20);
    rerender(<Probe v={2} />);
    await ms(20);
    expect(fetcher).toHaveBeenCalledTimes(2);
    await act(async () => { resolvers[1]({ n: 2 }); });
    expect(out.current.data).toEqual({ n: 2 });
    await act(async () => { resolvers[0]({ n: 1 }); });
    expect(out.current.data).toEqual({ n: 2 });
    expect(out.current.isLoading).toBe(false);
  });

  it('exposes data.errors and turns a rejection into one error', async () => {
    const fetcher = vi
      .fn<(n: number) => Promise<Out['data']>>()
      .mockResolvedValueOnce({ n: 1, errors: [{ field: 'paid_amount', message: 'PAID needs paid_amount >= total' }] })
      .mockRejectedValueOnce({ type: 'ValidationError', details: { field: 'amount_paid', message: 'too much' } });
    const out: { current: R } = { current: undefined as unknown as R };
    const Probe: React.FC<{ v: number }> = ({ v }) => {
      out.current = usePreview<number, Out['data']>({ input: v, fetcher, debounceMs: 10 });
      return null;
    };
    const { rerender } = render(<Probe v={1} />);
    await ms(20);
    expect(out.current.errors).toEqual([{ field: 'paid_amount', message: 'PAID needs paid_amount >= total' }]);
    rerender(<Probe v={2} />);
    await ms(20);
    expect(out.current.data).toBeNull();
    expect(out.current.errors).toEqual([{ field: 'amount_paid', message: 'too much' }]);
  });

  it('null input or enabled=false: no fetch, no data, not loading', async () => {
    const fetcher = vi.fn(async (n: number) => ({ n }));
    const a = mount(() => ({ input: null, fetcher }));
    const b = mount(() => ({ input: 5, fetcher, enabled: false }));
    await ms(200);
    expect(fetcher).not.toHaveBeenCalled();
    for (const o of [a.out, b.out]) {
      expect(o.current).toEqual({ data: null, errors: [], isLoading: false });
    }
  });

  it('an equal input (new object, same value) does not refetch', async () => {
    const fetcher = vi.fn(async (n: { a: number }) => ({ n: n.a }));
    const out: { current: UsePreviewResult<{ n: number }> } = { current: undefined as never };
    const Probe: React.FC = () => {
      out.current = usePreview<{ a: number }, { n: number }>({ input: { a: 1 }, fetcher, debounceMs: 10 });
      return null;
    };
    const { rerender } = render(<Probe />);
    await ms(30);
    rerender(<Probe />);
    await ms(30);
    expect(fetcher).toHaveBeenCalledTimes(1);
  });

  it('StrictMode remount: the preview still settles (stuck-on-loading regression)', async () => {
    // StrictMode mounts, unmounts and re-mounts every component in dev. The unmount invalidates the
    // first request; the remount must start a fresh one, not wait on the stale one.
    const fetcher = vi.fn(
      (n: number) => new Promise<Out['data']>((resolve) => setTimeout(() => resolve({ n }), 20))
    );
    const { out } = mount(() => ({ input: 7, fetcher, debounceMs: 80 }), true);
    await ms(150);
    expect(out.current.isLoading).toBe(false);
    expect(out.current.data).toEqual({ n: 7 });
  });
});
