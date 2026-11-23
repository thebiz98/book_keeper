import { useCallback } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { flushSync } from 'react-dom';
import { api } from '@/api';
import { useAppStore } from '@/store/useAppStore';
import type { PurchaseDetailView, SaleDetailView, TransactionType } from '@/types';
import { BillPrintDocument, type PrintShop } from './BillPrintDocument';

const PRINT_ROOT_ID = 'print-root';
/** If the webview never fires `afterprint`, clean up anyway. */
export const PRINT_CLEANUP_TIMEOUT_MS = 60_000;

const nextFrame = () => new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));

let active: { root: Root; host: HTMLElement; cleanup: () => void } | null = null;

/** Tears down any print view that is still mounted. */
export function cleanupPrintRoot(): void {
  active?.cleanup();
}

/**
 * Renders the bill into a `#print-root` host on `body`, waits two frames so layout settles, calls
 * `window.print()` and removes the host on `afterprint` (timeout as a fallback).
 */
export async function printBillDocument(
  kind: TransactionType,
  shop: PrintShop,
  detail: SaleDetailView | PurchaseDetailView
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
  flushSync(() => root.render(<BillPrintDocument kind={kind} shop={shop} detail={detail} />));
  await nextFrame();
  await nextFrame();
  if (done) return;
  window.addEventListener('afterprint', cleanup);
  timer = setTimeout(cleanup, PRINT_CLEANUP_TIMEOUT_MS);
  window.print();
}

/** Returns `printBill(kind, id)`: loads the bill and the Shop Profile, then prints. Rejects on load errors. */
export function usePrintBill(): (kind: TransactionType, billId: number) => Promise<void> {
  return useCallback(async (kind: TransactionType, billId: number) => {
    const store = useAppStore.getState();
    if (!store.shopLoaded) await store.loadShopSettings();
    const detail = kind === 'SALE' ? await api.getSaleDetails(billId) : await api.getPurchaseDetails(billId);
    await printBillDocument(kind, useAppStore.getState().shopProfile, detail);
  }, []);
}
