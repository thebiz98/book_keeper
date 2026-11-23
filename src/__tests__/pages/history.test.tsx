import { describe, it, expect, beforeEach } from 'vitest';
import { render } from '../testUtils';
import { SalesHistoryPage } from '@/pages/SalesHistoryPage';
import { BillDetailDrawer } from '@/components/domain/BillDetailDrawer';
import type { SaleDetailView } from '@/types';
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
  itemsSubtotal: 100,
  chargesTotal: 0,
  discountsTotal: 0,
});

describe('Sales history', () => {
  beforeEach(() => {
    document.body.innerHTML = '';
  });

  it('uses a full-height scroll container with a sticky header', async () => {
    installApi({ getSalesHistory: { items: [row(1, 30)], total: 1, page: 1, limit: 25 } });
    render(<SalesHistoryPage />);
    await waitFor(() => bodyText().includes('SAL-1'));
    expect(document.querySelector('[data-table-scroll]')?.getAttribute('data-table-scroll')).toBe('fill');
    expect(document.querySelector('th')?.className).toContain('sticky');
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

describe('Bill charges in history (FR-19)', () => {
  beforeEach(() => {
    document.body.innerHTML = '';
  });

  it('shows the Chg / Disc column', async () => {
    installApi({
      getSalesHistory: { items: [{ ...row(1, 0), chargesTotal: 50, discountsTotal: 20, itemsSubtotal: 200, totalAmount: 230 }, row(2, 0)], total: 2, page: 1, limit: 25 },
    });
    render(<SalesHistoryPage />);
    await waitFor(() => !!document.querySelector('[data-testid="adj-cell"]'));
    expect(bodyText()).toContain('Chg / Disc');
    const cells = Array.from(document.querySelectorAll('[data-testid="adj-cell"]')).map((c) => c.textContent);
    expect(cells).toEqual(['+₹50.00 / −₹20.00']);
  });

  it('the drawer lists the charge lines and the subtotal breakdown', async () => {
    const detail = {
      sale: { id: 1, transactionCode: 'SAL-1', transactionDate: '2026-02-01', partyName: null, paymentStatus: 'PAID', totalAmount: 230, paidAmount: 230, balanceDue: 0, itemsSubtotal: 200, chargesTotal: 50, discountsTotal: 20 },
      party: null,
      settlements: [],
      items: [],
      charges: [
        { id: 1, transactionType: 'SALE', transactionId: 1, chargeTypeId: 1, chargeName: 'Transport', effect: 'CHARGE', amount: 50 },
        { id: 2, transactionType: 'SALE', transactionId: 1, chargeTypeId: 3, chargeName: 'Cash Discount', effect: 'DISCOUNT', amount: 20 },
      ],
    } as unknown as SaleDetailView;
    installApi({ getSaleDetails: detail });
    render(<BillDetailDrawer kind="SALE" billId={1} onClose={() => {}} />);
    await waitFor(() => !!document.querySelector('[data-testid="bill-charges"]'));
    const text = document.querySelector('[data-testid="bill-charges"]')!.textContent ?? '';
    expect(text).toContain('Transport');
    expect(text).toContain('Cash Discount');
    expect(document.querySelector('[data-testid="bill-items-subtotal"]')!.textContent).toContain('200.00');
    expect(document.querySelector('[data-testid="bill-charges-total"]')!.textContent).toContain('50.00');
    expect(document.querySelector('[data-testid="bill-discounts-total"]')!.textContent).toContain('20.00');
  });
});