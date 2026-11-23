/**
 * Hand-off for bill edit mode (FR-20). The router (App.tsx ROUTES) renders pages without props, so
 * the bill drawer queues the bill here and then navigates:
 *
 *   import { queueBillEdit } from '@/pages/billEditHandoff';
 *   queueBillEdit({ kind: 'SALE', id });
 *   setActiveRoute('sale');
 *
 * NewSalePage / NewPurchasePage consume the queued value on mount (same pattern as
 * `purchasePrefill.ts`).
 */
export interface BillEditRequest {
  kind: 'SALE' | 'PURCHASE';
  id: number;
}

let pending: BillEditRequest | null = null;

export function queueBillEdit(r: BillEditRequest): void {
  pending = r;
}

/** Returns and clears the queued request. */
export function takeBillEdit(): BillEditRequest | null {
  const r = pending;
  pending = null;
  return r;
}

/** Reads without clearing. */
export function peekBillEdit(): BillEditRequest | null {
  return pending;
}
