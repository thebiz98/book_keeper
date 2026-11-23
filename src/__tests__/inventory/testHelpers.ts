(globalThis as unknown as { IS_REACT_ACT_ENVIRONMENT: boolean }).IS_REACT_ACT_ENVIRONMENT = true;
import { act } from 'react';
import { vi } from 'vitest';
import { setApiClient } from '@/api';
import type { ApiClient, ItemSearchResult } from '@/types';

/** Set an input/select/textarea value so React's onChange fires (bypasses the value tracker). */
export function typeInto(el: Element, value: string): void {
  const proto =
    el instanceof HTMLSelectElement
      ? HTMLSelectElement.prototype
      : el instanceof HTMLTextAreaElement
        ? HTMLTextAreaElement.prototype
        : HTMLInputElement.prototype;
  const setter = Object.getOwnPropertyDescriptor(proto, 'value')?.set;
  act(() => {
    setter?.call(el, value);
    el.dispatchEvent(new Event(el instanceof HTMLSelectElement ? 'change' : 'input', { bubbles: true }));
  });
}

export function click(el: Element): void {
  act(() => {
    el.dispatchEvent(new MouseEvent('click', { bubbles: true, cancelable: true }));
  });
}

export function key(el: Element | Window, init: KeyboardEventInit): void {
  act(() => {
    el.dispatchEvent(new KeyboardEvent('keydown', { bubbles: true, cancelable: true, ...init }));
  });
}

/** Let pending promises and short timers settle inside act. */
export async function flush(ms = 0): Promise<void> {
  await act(async () => {
    await new Promise((r) => setTimeout(r, ms));
  });
}

export function byText(root: ParentNode, selector: string, text: string | RegExp): HTMLElement {
  const found = Array.from(root.querySelectorAll<HTMLElement>(selector)).find((e) =>
    typeof text === 'string' ? (e.textContent ?? '').trim() === text : text.test(e.textContent ?? '')
  );
  if (!found) throw new Error(`No <${selector}> with text ${text}`);
  return found;
}

export function inputByLabel(root: ParentNode, label: string | RegExp): HTMLInputElement {
  const l = byText(root, 'label', label);
  const id = l.getAttribute('for');
  const el = id ? (root.querySelector(`#${CSS.escape(id)}`) as HTMLInputElement | null) : null;
  if (!el) throw new Error(`No control for label ${label}`);
  return el;
}

export function makeItem(over: Partial<ItemSearchResult> = {}): ItemSearchResult {
  return {
    itemId: 1,
    name: 'Classmate Notebook',
    category: 'Notebooks',
    barcode: '8901',
    baseUnit: 'PCS', itemType: 'PHYSICAL',
    mrp: 50,
    defaultSalePrice: 45,
    currentStock: 20,
    isLowStock: false,
    bundles: [],
    matchedBundleId: null,
    ...over,
  };
}

/** A fully stubbed ApiClient: every method is a vi.fn() returning undefined unless overridden. */
export function installApi(over: Partial<Record<keyof ApiClient, unknown>> = {}) {
  const base: Record<string, unknown> = {
    getNextPurchaseCode: vi.fn().mockResolvedValue('PUR-0001'),
    getItemsCatalog: vi.fn().mockResolvedValue({ items: [], total: 0, page: 1, limit: 50 }),
    listCategories: vi.fn().mockResolvedValue([]),
    getBundleSuggestions: vi.fn().mockResolvedValue([]),
    getItemLots: vi.fn().mockResolvedValue([]),
    listParties: vi.fn().mockResolvedValue([]),
    getPartyDuesSummary: vi.fn().mockResolvedValue([]),
    searchItemsFts: vi.fn().mockResolvedValue([]),
  };
  const api = { ...base, ...over } as Record<string, ReturnType<typeof vi.fn>>;
  setApiClient(api as unknown as ApiClient);
  return api;
}
