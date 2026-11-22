import { act } from 'react';
import { vi } from 'vitest';
import { setApiClient } from '@/api';
import type { ApiClient } from '@/types/api';
import type { ItemSearchResult, PartyListRow, SaleSummaryResponse } from '@/types';

export function makeItem(over: Partial<ItemSearchResult> = {}): ItemSearchResult {
  return {
    itemId: 1,
    name: 'Apsara Pencil',
    category: 'Stationery',
    barcode: '111',
    baseUnit: 'PCS',
    mrp: 12,
    defaultSalePrice: 10,
    currentStock: 100,
    isLowStock: false,
    bundles: [],
    matchedBundleId: null,
    ...over,
  };
}

export function makeParty(over: Partial<PartyListRow> = {}): PartyListRow {
  return {
    id: 7,
    name: 'Ramesh Kumar',
    phone: '9999900000',
    partyType: 'CUSTOMER',
    notes: null,
    isActive: true,
    createdAt: '',
    updatedAt: '',
    outstandingBalance: 0,
    transactionCount: 0,
    ...over,
  };
}

export function saleResponse(over: Partial<SaleSummaryResponse> = {}): SaleSummaryResponse {
  return {
    saleId: 1,
    transactionCode: 'SAL-0001',
    totalAmount: 100,
    paidAmount: 100,
    balanceDue: 0,
    changeAmount: 0,
    paymentStatus: 'PAID',
    ...over,
  };
}

/** Installs a fake ApiClient (the global setup mocks Tauri, so the default client is not usable). */
export function installApi(over: Partial<Record<keyof ApiClient, unknown>> = {}) {
  const fake = {
    getNextSaleCode: vi.fn(async () => 'SAL-0002'),
    createSale: vi.fn(async () => saleResponse()),
    searchItemsFts: vi.fn(async () => [] as ItemSearchResult[]),
    listParties: vi.fn(async () => [] as PartyListRow[]),
    createParty: vi.fn(async () => 99),
    getDashboardMetrics: vi.fn(async () => ({ liveCashInDrawer: 123 })),
    getShopSettings: vi.fn(async () => ({
      id: 1,
      shopName: 'Test Shop',
      phone: null,
      address: null,
      taxId: null,
    })),
    getBundleSuggestions: vi.fn(async () => []),
    createBundle: vi.fn(),
    ...over,
  };
  setApiClient(fake as unknown as ApiClient);
  return fake as typeof fake & Record<string, ReturnType<typeof vi.fn>>;
}

/** Type into a React-controlled input (uses the native setter so React's value tracker fires onChange). */
export function typeInto(el: Element, value: string): void {
  const input = el as HTMLInputElement;
  const setter = Object.getOwnPropertyDescriptor(Object.getPrototypeOf(input), 'value')?.set;
  act(() => {
    setter?.call(input, value);
    input.dispatchEvent(new Event('input', { bubbles: true }));
  });
}

export function press(el: Element | Document | Window, key: string, init: KeyboardEventInit = {}): KeyboardEvent {
  const ev = new KeyboardEvent('keydown', { key, bubbles: true, cancelable: true, ...init });
  act(() => {
    el.dispatchEvent(ev);
  });
  return ev;
}

/** Advance fake timers and flush promises inside act. */
export async function flush(ms = 0): Promise<void> {
  await act(async () => {
    await vi.advanceTimersByTimeAsync(ms);
  });
}

export const byTestId = (id: string): HTMLElement | null =>
  document.querySelector<HTMLElement>(`[data-testid="${id}"]`);
