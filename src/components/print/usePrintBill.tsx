import { useMemo } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { flushSync } from 'react-dom';
import { api } from '@/api';
import { useAppStore } from '@/store/useAppStore';
import type { PurchaseDetailView, SaleDetailView, TransactionType } from '@/types';
import { BillPrintDocument, type PrintShop, type PrintSize } from './BillPrintDocument';

const PRINT_ROOT_ID = 'print-root';
/** If the webview never fires `afterprint`, clean up anyway. */
export const PRINT_CLEANUP_TIMEOUT_MS = 60_000;

/** Natural bill height (mm) that still fits the top half of an A4 (min-height of a half bill). */
export const HALF_SHEET_MAX_MM = 136;
/** Natural bill height (mm) that still fits one A4 page (297mm - 2 x 8mm margin, less a hair). */
export const FULL_SHEET_MAX_MM = 280;
const MM_PER_PX = 25.4 / 96;

/** Half an A4 for a short bill, a full A4 when it is longer, several pages beyond that. */
export function pickPrintSize(heightMm: number): PrintSize {
  if (heightMm <= HALF_SHEET_MAX_MM) return 'half';
  if (heightMm <= FULL_SHEET_MAX_MM) return 'full';
  return 'multi';
}

const nextFrame = () => new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));

let active: { root: Root; host: HTMLElement; cleanup: () => void } | null = null;

/** Tears down any print view that is still mounted. */
export function cleanupPrintRoot(): void {
  active?.cleanup();
}

/**
 * Renders the bill into a `#print-root` host on `body`, measured off-screen at the print width
 * (`.print-measure`) to pick its size (half / full A4 / multi-page), re-renders it at that size,
 * waits two frames so layout settles, calls `window.print()` and removes the host on `afterprint`
 * (timeout as a fallback).
 */
export async function printBillDocument(
  kind: TransactionType,
  shop: PrintShop,
  detail: SaleDetailView | PurchaseDetailView,
): Promise<void> {
  cleanupPrintRoot();
  const host = document.createElement('div');
  host.id = PRINT_ROOT_ID;
  document.body.appendChild(host);
  const root: Root = createRoot(host);
  let timer: ReturnType<typeof setTimeout> | undefined;
  let done = false;
  const cleanup = () => {
    if (done) return;
    done = true;
    window.removeEventListener('afterprint', cleanup);
    if (timer) clearTimeout(timer);
    root.unmount();
    host.remove();
    if (active?.host === host) active = null;
  };
  active = { root, host, cleanup };
  host.classList.add('print-measure');
  flushSync(() => root.render(<BillPrintDocument kind={kind} shop={shop} detail={detail} />));
  await nextFrame();
  if (done) return;
  const doc = host.querySelector('.print-doc');
  const size = pickPrintSize((doc?.getBoundingClientRect().height ?? 0) * MM_PER_PX);
  host.classList.remove('print-measure');
  flushSync(() => root.render(<BillPrintDocument kind={kind} shop={shop} detail={detail} size={size} />));
  await nextFrame();
  await nextFrame();
  if (done) return;
  window.addEventListener('afterprint', cleanup);
  timer = setTimeout(cleanup, PRINT_CLEANUP_TIMEOUT_MS);
  window.print();
}

/** Prints an already loaded bill with the Shop Profile from the store (loaded on demand). */
export async function printLoadedBill(
  kind: TransactionType,
  detail: SaleDetailView | PurchaseDetailView,
): Promise<void> {
  const store = useAppStore.getState();
  if (!store.shopLoaded) await store.loadShopSettings();
  await printBillDocument(kind, useAppStore.getState().shopProfile, detail);
}

export interface PrintBillApi {
  /** Prints a bill that is already loaded. */
  printDetail: typeof printLoadedBill;
  /** Loads the bill by id, then prints it. Rejects on load errors. */
  printById: (kind: TransactionType, billId: number) => Promise<void>;
}

export function usePrintBill(): PrintBillApi {
  return useMemo<PrintBillApi>(
    () => ({
      printDetail: printLoadedBill,
      printById: async (kind, billId) => {
        const detail = kind === 'SALE' ? await api.getSaleDetails(billId) : await api.getPurchaseDetails(billId);
        await printLoadedBill(kind, detail);
      },
    }),
    [],
  );
}
