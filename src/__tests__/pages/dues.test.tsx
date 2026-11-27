import { describe, it, expect, beforeEach } from 'vitest';
import { render } from '../testUtils';
import { CreditDuesPage } from '@/pages/CreditDuesPage';
import type { PartyDueSummary, SaleHistoryRow } from '@/types';
import { installApi, flush, waitFor, click, setInput, buttonByText, bodyText } from './helpers';

const party: PartyDueSummary = {
  partyId: 7,
  partyName: 'Sharma Traders',
  phone: '999',
  partyType: 'CUSTOMER',
  unsettledBillsCount: 2,
  earliestBillDate: '2026-01-01',
  totalInvoiced: 500,
  totalPaid: 200,
  balanceDue: 300,
};
const bill = (id: number, date: string, due: number, status: 'UNPAID' | 'PARTIAL'): SaleHistoryRow => ({
  id,
  transactionCode: `SAL-${id}`,
  transactionDate: date,
  createdAt: `${date}T10:00:00`,
  partyId: 7,
  partyName: 'Sharma Traders',
  lineCount: 1,
  totalAmount: due + 50,
  paidAmount: 50,
  balanceDue: due,
  paymentStatus: status,
  itemsSubtotal: 0,
  chargesTotal: 0,
  discountsTotal: 0,
});

function setup() {
  return installApi({
    getPartyDuesSummary: [party],
    getSalesHistory: async (f: { paymentStatus?: string }) => ({
      items: f.paymentStatus === 'UNPAID' ? [bill(2, '2026-01-02', 200, 'UNPAID')] : [bill(1, '2026-01-01', 100, 'PARTIAL')],
      total: 1,
      page: 1,
      limit: 200,
    }),
    settleBillPayment: { settlementIds: [1], billsTouched: [1] },
    settleBillsBatch: { settlementIds: [1, 2], billsTouched: [1, 2] },
    // DD-56: the split and the over-limit refusal are the backend's; canned here.
    previewSettlementAllocation: async (_p: number, _t: string, amount: number) => {
      if (amount > 300) throw { type: 'ValidationError', details: { field: 'amount_paid', message: 'Settlement amount exceeds total outstanding dues' } };
      return [{ transactionId: 1, amountPaid: 100 }, { transactionId: 2, amountPaid: amount - 100 }];
    },
  });
}

describe('CreditDuesPage', () => {
  beforeEach(() => {
    document.body.innerHTML = '';
  });

  it('dues list fills the page height with a sticky header', async () => {
    setup();
    render(<CreditDuesPage />);
    await waitFor(() => bodyText().includes('Sharma Traders'));
    expect(document.querySelector('[data-table-scroll]')?.getAttribute('data-table-scroll')).toBe('fill');
  });

  it('shows Paid & credited from the backend row (L3: return credits included)', async () => {
    installApi({ getPartyDuesSummary: [{ ...party, totalPaid: 200, paidAndCredited: 260, balanceDue: 240 }] });
    render(<CreditDuesPage />);
    await waitFor(() => bodyText().includes('Sharma Traders'));
    const heads = Array.from(document.querySelectorAll('th')).map((h) => h.textContent ?? '');
    expect(heads).toContain('Paid & credited');
    expect(heads).not.toContain('Total Paid');
    expect(bodyText()).toContain('₹260.00');
  });

  it('settles a single bill with transactionId', async () => {
    const api = setup();
    render(<CreditDuesPage />);
    await waitFor(() => bodyText().includes('Sharma Traders'));
    click(document.querySelector('[aria-label^="Toggle open bills"]')!);
    await waitFor(() => !!document.querySelector('[data-testid="open-bills"]'));
    // oldest first: SAL-1 is the first row
    // nested table in an expanded row: plain (non-sticky) header, must not cover the outer one
    const nestedTh = document.querySelector('[data-testid="open-bills"] th');
    expect(nestedTh?.className).not.toContain('sticky');
    expect(document.querySelector('[data-testid="open-bills"]')?.closest('[data-table-scroll]')?.getAttribute('data-table-scroll')).toBe('none');
    const rows = document.querySelectorAll('[data-testid="open-bills"] tbody tr');
    expect(rows[0].textContent).toContain('SAL-1');
    click(buttonByText('Settle', rows[0] as HTMLElement));
    await flush();
    expect(bodyText()).toContain('SAL-1');
    click(buttonByText('Record Settlement', document.querySelector('[role="dialog"]')!));
    await flush();
    expect(api.settleBillPayment).toHaveBeenCalledTimes(1);
    const payload = api.settleBillPayment.mock.calls[0][0];
    expect(payload).toMatchObject({ partyId: 7, transactionType: 'SALE', transactionId: 1, amountPaid: 100 });
    expect(payload.settlementDate).toMatch(/^\d{4}-\d{2}-\d{2}$/);
  });

  it('shows the backend DATE_ORDER refusal of a payment dated before its bill (DD-94)', async () => {
    const msg = "Payment date can't be before the bill date (01 Jan 2026)";
    const api = setup();
    api.settleBillPayment.mockImplementation(async () => {
      throw { type: 'BillLocked', details: { code: 'DATE_ORDER', message: msg } };
    });
    render(<CreditDuesPage />);
    await waitFor(() => bodyText().includes('Sharma Traders'));
    click(document.querySelector('[aria-label^="Toggle open bills"]')!);
    await waitFor(() => !!document.querySelector('[data-testid="open-bills"]'));
    const rows = document.querySelectorAll('[data-testid="open-bills"] tbody tr');
    click(buttonByText('Settle', rows[0] as HTMLElement));
    await flush();
    click(buttonByText('Record Settlement', document.querySelector('[role="dialog"]')!));
    await waitFor(() => bodyText().includes(msg));
    expect(document.querySelector('[role="dialog"]')).not.toBeNull();
  });

  it('Settle bills sends one batch payload and previews the FIFO fill', async () => {
    const api = setup();
    render(<CreditDuesPage />);
    await waitFor(() => bodyText().includes('Sharma Traders'));
    click(buttonByText('Settle bills'));
    await waitFor(() => !!document.querySelector('[data-testid="batch-rows"] tbody tr'));
    const input = document.querySelector('input[aria-label="Settlement amount"]')!;
    setInput(input, '150.5');
    await waitFor(() => (document.querySelector('input[aria-label="Amount for SAL-2"]') as HTMLInputElement).value === '50.5');
    const rows = document.querySelector('[data-testid="batch-rows"]')!;
    expect((rows.querySelector('input[aria-label="Amount for SAL-1"]') as HTMLInputElement).value).toBe('100');
    expect((rows.querySelector('input[aria-label="Amount for SAL-2"]') as HTMLInputElement).value).toBe('50.5');
    click(buttonByText('Settle bills', document.querySelector('[role="dialog"]')!));
    await flush();
    expect(api.settleBillsBatch).toHaveBeenCalledTimes(1);
    expect(api.settleBillsBatch.mock.calls[0][0]).toMatchObject({
      partyId: 7,
      transactionType: 'SALE',
      allocations: [
        { transactionId: 1, amountPaid: 100 },
        { transactionId: 2, amountPaid: 50.5 },
      ],
    });
    expect(api.settleBillPayment).not.toHaveBeenCalled();
  });

  it('rejects an amount above the outstanding balance', async () => {
    const api = setup();
    render(<CreditDuesPage />);
    await waitFor(() => bodyText().includes('Sharma Traders'));
    click(buttonByText('Settle bills'));
    await waitFor(() => !!document.querySelector('[data-testid="batch-rows"] tbody tr'));
    setInput(document.querySelector('input[aria-label="Settlement amount"]')!, '301');
    await waitFor(() => bodyText().includes('exceeds total outstanding'));
    const save = buttonByText('Settle bills', document.querySelector('[role="dialog"]')!);
    expect(save.disabled).toBe(true);
    click(save);
    await flush();
    expect(api.settleBillsBatch).not.toHaveBeenCalled();
    expect(bodyText()).toContain('exceeds total outstanding');
  });

  it('shows the error instead of "all settled" when loading fails', async () => {
    installApi({
      getPartyDuesSummary: async () => {
        throw { type: 'DatabaseError', details: 'db is locked' };
      },
    });
    render(<CreditDuesPage />);
    await waitFor(() => bodyText().includes('db is locked'));
    expect(bodyText()).not.toContain('No outstanding dues');
    expect(bodyText()).not.toContain('₹0.00');
  });
});

