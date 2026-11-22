import { describe, it, expect, beforeEach } from 'vitest';
import { render } from '../testUtils';
import { SalesHistoryPage } from '@/pages/SalesHistoryPage';
import { PurchaseHistoryPage } from '@/pages/PurchaseHistoryPage';
import { installApi, flush, waitFor, click, setInput, buttonByText, bodyText } from './helpers';
import type { SaleHistoryRow } from '@/types';

const row = (id: number, due: number): SaleHistoryRow => ({
  id,
  transactionCode: `SAL-${id}`,
  transactionDate: '2026-02-01',
  createdAt: '2026-02-01T10:30:00',
  partyId: 3,
  partyName: 'Ram Snapshot',
  lineCount: 4,
  totalAmount: 100,
  paidAmount: 100 - due,
  balanceDue: due,
  paymentStatus: due > 0 ? 'PARTIAL' : 'PAID',
});

describe('Sales history', () => {
  beforeEach(() => {
    document.body.innerHTML = '';
  });

  it('loads server-side with default filters, shows partyName/lineCount, and settle only when due', async () => {
    const api = installApi({
      getSalesHistory: { items: [row(1, 30), row(2, 0)], total: 60, page: 1, limit: 25 },
    });
    render(<SalesHistoryPage />);
    await waitFor(() => bodyText().includes('SAL-1'));
    expect(api.getSalesHistory.mock.calls[0][0]).toEqual({
      startDate: null,
      endDate: null,
      paymentStatus: 'ALL',
      searchQuery: null,
      page: 1,
      limit: 25,
    });
    expect(bodyText()).toContain('Ram Snapshot');
    const rows = document.querySelectorAll('tbody tr');
    expect(rows[0].textContent).toContain('Settle');
    expect(rows[1].textContent).not.toContain('Settle');
  });

  it('status filter, pager and search hit the server', async () => {
    const api = installApi({
      getSalesHistory: { items: [row(1, 30)], total: 60, page: 1, limit: 25 },
    });
    render(<SalesHistoryPage />);
    await waitFor(() => bodyText().includes('SAL-1'));
    click(document.querySelector('[data-status="PAID"]')!);
    await flush();
    expect(api.getSalesHistory.mock.calls.at(-1)![0]).toMatchObject({ paymentStatus: 'PAID', page: 1 });
    click(document.querySelector('button[aria-label="Next page"]')!);
    await flush();
    expect(api.getSalesHistory.mock.calls.at(-1)![0]).toMatchObject({ paymentStatus: 'PAID', page: 2 });
    setInput(document.querySelector('input[aria-label="Search"]')!, 'ram');
    await flush(400);
    expect(api.getSalesHistory.mock.calls.at(-1)![0]).toMatchObject({ searchQuery: 'ram', page: 1 });
    click(buttonByText('Today'));
    await flush();
    const last = api.getSalesHistory.mock.calls.at(-1)![0];
    expect(last.startDate).toMatch(/^\d{4}-\d{2}-\d{2}$/);
    expect(last.startDate).toBe(last.endDate);
  });

  it('shows a load error instead of an empty state', async () => {
    installApi({
      getSalesHistory: async () => {
        throw { type: 'ValidationError', details: { field: 'page', message: 'page must be >= 1' } };
      },
    });
    render(<SalesHistoryPage />);
    await waitFor(() => bodyText().includes('page must be >= 1'));
    expect(bodyText()).not.toContain('No sales invoices match');
  });
});

describe('Purchase history', () => {
  beforeEach(() => {
    document.body.innerHTML = '';
  });

  it('calls the purchases endpoint and shows the supplier bill ref', async () => {
    const api = installApi({
      getPurchasesHistory: {
        items: [{ ...row(1, 0), transactionCode: 'PUR-1', supplierInvoiceRef: 'INV-77' }],
        total: 1,
        page: 1,
        limit: 25,
      },
    });
    render(<PurchaseHistoryPage />);
    await waitFor(() => bodyText().includes('PUR-1'));
    expect(api.getPurchasesHistory).toBeDefined();
    expect(bodyText()).toContain('INV-77');
  });
});
