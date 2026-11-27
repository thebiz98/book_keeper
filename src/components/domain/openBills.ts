import { api } from '@/api';
import type { OpenBillRow, PartyType, TransactionType } from '@/types';

export type OpenBill = OpenBillRow;

export function transactionTypeFor(partyType: PartyType): TransactionType {
  return partyType === 'CUSTOMER' ? 'SALE' : 'PURCHASE';
}

/** Open bills of a party in the backend's order (oldest first, the FIFO settlement order; DD-97). */
export function fetchOpenBills(type: TransactionType, partyId: number): Promise<OpenBill[]> {
  return api.getPartyOpenBills(partyId, type);
}
