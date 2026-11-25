import { api } from '@/api';
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
      const filter = { partyId, paymentStatus: status, page, limit: 200, includeOpeningBalance: true };
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
