import { act } from 'react';
import { vi } from 'vitest';
import { setApiClient } from '@/api';
import type { ApiClient } from '@/types';

/** Installs a partial fake client; unspecified methods reject loudly. Returns the vi.fn per method. */
export function installApi(overrides: Partial<Record<keyof ApiClient, unknown>>): Record<string, ReturnType<typeof vi.fn>> {
  const fns: Record<string, ReturnType<typeof vi.fn>> = {};
  for (const [name, v] of Object.entries(overrides)) {
    fns[name] = typeof v === 'function' ? vi.fn(v as (...a: unknown[]) => unknown) : vi.fn(async () => v);
  }
  const client = new Proxy(fns as Record<string, unknown>, {
    get(t, prop: string) {
      if (prop in t) return t[prop];
      return async () => {
        throw new Error(`unexpected api call: ${prop}`);
      };
    },
  });
  setApiClient(client as unknown as ApiClient);
  return fns;
}

export async function flush(ms = 0): Promise<void> {
  await act(async () => {
    await new Promise((r) => setTimeout(r, ms));
  });
}

export async function waitFor(check: () => boolean, timeout = 1500): Promise<void> {
  const start = Date.now();
  while (!check()) {
    if (Date.now() - start > timeout) throw new Error('waitFor timed out');
    await flush(10);
  }
}

export function setInput(el: Element, value: string | number): void {
  const proto = el instanceof HTMLSelectElement ? HTMLSelectElement.prototype : HTMLInputElement.prototype;
  const setter = Object.getOwnPropertyDescriptor(proto, 'value')?.set;
  act(() => {
    setter?.call(el, String(value));
    el.dispatchEvent(new Event(el instanceof HTMLSelectElement ? 'change' : 'input', { bubbles: true }));
  });
}

export function click(el: Element): void {
  act(() => {
    el.dispatchEvent(new MouseEvent('click', { bubbles: true, cancelable: true }));
  });
}

export function buttonByText(text: string | RegExp, root: ParentNode = document.body): HTMLButtonElement {
  const btns = Array.from(root.querySelectorAll('button'));
  const b = btns.find((x) => (typeof text === 'string' ? (x.textContent ?? '').trim().includes(text) : text.test(x.textContent ?? '')));
  if (!b) throw new Error(`button not found: ${text}`);
  return b;
}

export const bodyText = () => document.body.textContent ?? '';
