import { useMemo } from 'react';
import { usePreview } from '@/hooks/usePreview';
import type { BillPreview, PaymentStatus, PreviewError } from '@/types';

type WithPayment<P> = P & { paymentStatus?: PaymentStatus | null; paidAmount?: number | null };

export interface UseBillEditPreviewOptions<P> {
  /** The bill without a payment, or null while the form is incomplete (nothing to preview). */
  bill: P | null;
  paymentStatus: PaymentStatus;
  /** The typed paid amount; only used for PARTIAL. */
  paidInput: number;
  /** `previewPurchase` / `previewSale` bound to the bill being edited. */
  fetcher: (payload: WithPayment<P>) => Promise<BillPreview>;
  /** Bumped to re-run both previews for the same draft (after a Restore, DD-90). */
  refreshKey?: number;
}

/**
 * DD-56 for an edit form: two backend previews, never a figure computed here.
 * `totals` previews the bill without a payment (line totals, charges, total, open); `pay` adds the
 * chosen payment (balance, errors). PAID pays what the backend says is open.
 */
export function useBillEditPreview<P extends object>({
  bill,
  paymentStatus,
  paidInput,
  fetcher,
  refreshKey = 0,
}: UseBillEditPreviewOptions<P>) {
  const totalsPreview = usePreview<WithPayment<P>, BillPreview>({ input: bill, fetcher, refreshKey });
  const totals = totalsPreview.data;
  const paidToSend = paymentStatus === 'PAID' ? totals?.openAmount : paymentStatus === 'UNPAID' ? 0 : paidInput;
  const payInput = useMemo<WithPayment<P> | null>(
    () =>
      bill && paidToSend != null && !totalsPreview.isLoading ? { ...bill, paymentStatus, paidAmount: paidToSend } : null,
    [bill, paymentStatus, paidToSend, totalsPreview.isLoading]
  );
  const payPreview = usePreview<WithPayment<P>, BillPreview>({ input: payInput, fetcher, refreshKey });

  const pending = bill !== null && (totalsPreview.isLoading || payPreview.isLoading || payPreview.data === null);
  const errors = useMemo(() => {
    const seen = new Set<string>();
    const out: PreviewError[] = [];
    for (const e of [...totalsPreview.errors, ...payPreview.errors]) {
      const k = `${e.field}|${e.message}`;
      if (!seen.has(k)) {
        seen.add(k);
        out.push(e);
      }
    }
    return out;
  }, [totalsPreview.errors, payPreview.errors]);

  return {
    totals,
    pay: payPreview.data?.payment ?? null,
    pending,
    errors,
    /** What to send as `paidAmount` (undefined until the backend has said what is open). */
    paidToSend,
    paidShown: paymentStatus === 'PARTIAL' ? paidInput : (payPreview.data?.payment?.paidAmount ?? paidToSend ?? 0),
    balanceDue: payPreview.data?.payment?.balanceDue ?? totals?.openAmount ?? 0,
  };
}
