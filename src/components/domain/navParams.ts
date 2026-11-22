import { useAppStore } from '@/store/useAppStore';

/**
 * Cross-page navigation parameters (F6-c).
 *
 * The app has no router, so a page hand-off is: write a query param with
 * `navigateWithParam`, switch route, and the target page calls `consumeNavParam`
 * once on mount (which removes it again).
 *
 * Known params:
 *  - `returnBill` = "SALE:<saleId>" | "PURCHASE:<purchaseId>"  -> Returns & Adjustments page
 *  - `inwardItem` = "<itemId>"                                  -> New Purchase page (TODO(F6-integrate): (b) must read it)
 */
export function navigateWithParam(route: string, key: string, value: string): void {
  try {
    const url = new URL(window.location.href);
    url.searchParams.set(key, value);
    window.history.replaceState(null, '', url.pathname + url.search + url.hash);
  } catch {
    /* history unavailable: navigate without the hint */
  }
  useAppStore.getState().setActiveRoute(route);
}

/** Reads a query param without removing it (safe inside render / state initializers). */
export function peekNavParam(key: string): string | null {
  try {
    return new URL(window.location.href).searchParams.get(key);
  } catch {
    return null;
  }
}

/** Reads and removes a query param. Returns null when absent. */
export function consumeNavParam(key: string): string | null {
  try {
    const url = new URL(window.location.href);
    const v = url.searchParams.get(key);
    if (v === null) return null;
    url.searchParams.delete(key);
    window.history.replaceState(null, '', url.pathname + url.search + url.hash);
    return v;
  } catch {
    return null;
  }
}

export function parseReturnBill(v: string | null): { kind: 'SALE' | 'PURCHASE'; id: number } | null {
  if (!v) return null;
  const [kind, idStr] = v.split(':');
  const id = Number(idStr);
  if ((kind !== 'SALE' && kind !== 'PURCHASE') || !Number.isInteger(id) || id <= 0) return null;
  return { kind, id };
}

/**
 * Asks the shell to re-read the header cash pill / metrics after money moved.
 * TODO(F6-integrate): `refreshMetrics` is being added to the app store by the shell work; until
 * it exists this is a no-op (typed loosely so it compiles either way).
 */
export function refreshAppMetrics(): void {
  const s = useAppStore.getState() as unknown as { refreshMetrics?: () => void | Promise<void> };
  void s.refreshMetrics?.();
}
