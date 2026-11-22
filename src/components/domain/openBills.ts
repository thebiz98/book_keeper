import { api } from '@/api';
import { round2 } from '@/lib/money';
import type { PartyType, PurchaseHistoryRow, SaleHistoryRow, TransactionType } from '@/types';

export type OpenBill = SaleHistoryRow | PurchaseHistoryRow;

export function transactionTypeFor(partyType: PartyType): TransactionType {
  return partyType === 'CUSTOMER' ? 'SALE' : 'PURCHASE';
}

/** All open (UNPAID + PARTIAL) bills of a party, oldest first (the FIFO settlement order). */
export async function fetchOpenBills(type: TransactionType, partyId: number): Promise<OpenBill[]> {
  const fetchAll = async (status: 'UNPAID' | 'PARTIAL'): Promise<OpenBill[]> => {
    const out: OpenBill[] = [];
    for (let page = 1; page <= 50; page++) {
      const filter = { partyId, paymentStatus: status, page, limit: 200 };
      const res = type === 'SALE' ? await api.getSalesHistory(filter) : await api.getPurchasesHistory(filter);
      out.push(...res.items);
      if (out.length >= res.total || res.items.length === 0) break;
    }
    return out;
  };
  const [unpaid, partial] = await Promise.all([fetchAll('UNPAID'), fetchAll('PARTIAL')]);
  return [...unpaid, ...partial]
    .filter((b) => b.balanceDue > 0)
    .sort((a, b) =>
      a.transactionDate === b.transactionDate ? a.id - b.id : a.transactionDate < b.transactionDate ? -1 : 1
    );
}

export interface FifoSplitRow {
  bill: OpenBill;
  applied: number;
  remainingAfter: number;
}

/** Preview of how a lump-sum payment is applied across bills, oldest first. */
export function allocateFifo(bills: OpenBill[], amount: number): FifoSplitRow[] {
  let left = round2(Math.max(0, amount));
  const out: FifoSplitRow[] = [];
  for (const bill of bills) {
    if (left <= 0) break;
    const applied = round2(Math.min(left, bill.balanceDue));
    if (applied <= 0) continue;
    out.push({ bill, applied, remainingAfter: round2(bill.balanceDue - applied) });
    left = round2(left - applied);
  }
  return out;
}
