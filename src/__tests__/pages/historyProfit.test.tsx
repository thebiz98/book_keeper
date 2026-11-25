import { describe, it, expect, beforeEach, vi } from 'vitest';
import { render } from '../testUtils';
import { SalesHistoryPage } from '@/pages/SalesHistoryPage';
import { PurchaseHistoryPage } from '@/pages/PurchaseHistoryPage';
import { BillDetailDrawer } from '@/components/domain/BillDetailDrawer';
import { useAppStore } from '@/store/useAppStore';
import { installApi, flush, waitFor, click, bodyText, fakeBillPreview } from './helpers';
import { saleDetail, saleGuard, search } from '../billEdit/fixtures';
import type { SaleHistoryRow, SaleDetailView } from '@/types';

(globalThis as unknown as { IS_REACT_ACT_ENVIRONMENT: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

vi.mock('@/components/domain/ItemSearchTypeahead', () => ({ ItemSearchTypeahead: () => <div /> }));
vi.mock('@/components/domain/PartySelector', () => ({ PartySelector: () => <div data-testid="stub-party" /> }));

const row = (id: number, over: Partial<SaleHistoryRow> = {}): SaleHistoryRow => ({
  id,
  transactionCode: `SAL-${id}`,
  transactionDate: '2026-02-01',
  createdAt: '2026-02-01T10:30:00',
  partyId: 3,
  partyName: 'Ram',
  lineCount: 2,
  totalAmount: 100,
  paidAmount: 70,
  balanceDue: 30,
  paymentStatus: 'PARTIAL',
  itemsSubtotal: 100,
  chargesTotal: 0,
  discountsTotal: 0,
  ...over,
});
const page = (items: SaleHistoryRow[]) => ({ items, total: items.length, page: 1, limit: 25 });
const q = (sel: string) => document.querySelector(sel) as HTMLElement;
const label = (l: string) => document.querySelector(`[aria-label="${l}"]`) as HTMLElement | null;

const profitDetail = (): SaleDetailView => {
  const d = saleDetail();
  d.items[0] = { ...d.items[0], costAmount: 60, profit: 40 };
  d.items[1] = { ...d.items[1], costAmount: 10, profit: 20 };
  d.profitSummary = { linesProfit: 60, charges: 5, returnAdjustment: -12, total: 53 };
  return d;
};

beforeEach(() => {
  document.body.innerHTML = '';
  useAppStore.setState({ showProfit: false });
});

describe('History icon actions (FR-34)', () => {
  it('actions are icon buttons with accessible names; Settle only with a balance and a party', async () => {
    installApi({ getSalesHistory: page([row(1), row(2, { balanceDue: 0, paidAmount: 100, paymentStatus: 'PAID' })]) });
    render(<SalesHistoryPage />);
    await waitFor(() => bodyText().includes('SAL-1'));
    for (const a of ['View', 'Edit', 'Settle', 'Return']) {
      const b = label(`${a} SAL-1`) as HTMLButtonElement;
      expect(b, a).not.toBeNull();
      expect(b.getAttribute('title')).toBe(`${a} SAL-1`);
    }
    expect(label('Settle SAL-2')).toBeNull();
    expect(label('View SAL-2')).not.toBeNull();
    const rows = document.querySelectorAll('tbody tr');
    expect(rows[0].textContent).not.toContain('Settle'); // icon only, no text
  });

  it('Edit opens the drawer in edit mode', async () => {
    installApi({
      getSalesHistory: page([row(5, { transactionCode: 'SAL-1005' })]),
      getSaleDetails: saleDetail(),
      getBillEditGuard: saleGuard(),
      searchItemsFts: search,
      previewSale: async (p: Parameters<typeof fakeBillPreview>[0]) => fakeBillPreview(p),
      getDashboardMetrics: { cashInHand: 0 },
    });
    render(<SalesHistoryPage />);
    await waitFor(() => bodyText().includes('SAL-1005'));
    click(label('Edit SAL-1005')!);
    await waitFor(() => !!q('[data-testid="sale-edit-form"]'));
  });

  it('Edit stays in view mode with the reasons when the guard blocks editing', async () => {
    installApi({
      getSalesHistory: page([row(5, { transactionCode: 'SAL-1005' })]),
      getSaleDetails: saleDetail(),
      getBillEditGuard: saleGuard({ canEdit: false, reasons: [{ code: 'HAS_RETURNS', message: 'Bill has returns', blocksEdit: true, blocksDelete: true }] }),
    });
    render(<SalesHistoryPage />);
    await waitFor(() => bodyText().includes('SAL-1005'));
    click(label('Edit SAL-1005')!);
    await waitFor(() => !!q('[data-testid="guard-reasons"]'));
    expect(q('[data-testid="guard-reasons"]').textContent).toContain('Bill has returns');
    expect(q('[data-testid="sale-edit-form"]')).toBeNull();
  });

  it('View opens the drawer in view mode', async () => {
    installApi({ getSalesHistory: page([row(5)]), getSaleDetails: saleDetail(), getBillEditGuard: saleGuard() });
    render(<SalesHistoryPage />);
    await waitFor(() => bodyText().includes('SAL-5'));
    click(label('View SAL-5')!);
    await waitFor(() => !!q('[data-testid="bill-actions"]'));
    expect(q('[data-testid="sale-edit-form"]')).toBeNull();
  });
});

describe('Chg / Disc cell (FR-34)', () => {
  it('renders the charge and the discount on two lines, only the non-zero ones', async () => {
    installApi({
      getSalesHistory: page([row(1, { chargesTotal: 50, discountsTotal: 20 }), row(2, { chargesTotal: 0, discountsTotal: 7 })]),
    });
    render(<SalesHistoryPage />);
    await waitFor(() => bodyText().includes('SAL-1'));
    const cells = Array.from(document.querySelectorAll('[data-testid="adj-cell"]'));
    const lines = cells.map((c) => Array.from(c.children).map((s) => s.textContent));
    expect(lines).toEqual([['+₹50.00', '−₹20.00'], ['−₹7.00']]);
    expect(cells[0].querySelectorAll('.whitespace-nowrap')).toHaveLength(2);
  });
});

describe('Profit column (FR-34)', () => {
  it('is hidden by default and the request carries no includeProfit', async () => {
    const api = installApi({ getSalesHistory: page([row(1, { profit: 12 })]) });
    render(<SalesHistoryPage />);
    await waitFor(() => bodyText().includes('SAL-1'));
    expect(Array.from(document.querySelectorAll('th')).map((t) => t.textContent)).not.toContain('Profit');
    expect(api.getSalesHistory.mock.calls[0][0]).not.toHaveProperty('includeProfit');
  });

  it('the Columns menu toggle shows the column and sends includeProfit', async () => {
    const api = installApi({
      getSalesHistory: async (f: { includeProfit?: boolean }) => page([row(1, { profit: f.includeProfit ? 12.5 : null })]),
    });
    render(<SalesHistoryPage />);
    await waitFor(() => bodyText().includes('SAL-1'));
    click(label('Columns')!);
    click(label('Show profit')!);
    await flush(50);
    await waitFor(() => Array.from(document.querySelectorAll('th')).some((t) => t.textContent === 'Profit'));
    expect(api.getSalesHistory.mock.calls.at(-1)![0]).toMatchObject({ includeProfit: true });
    expect(bodyText()).toContain('12.50');
    expect(useAppStore.getState().showProfit).toBe(true);
    click(label('Show profit')!);
    await flush(50);
    expect(api.getSalesHistory.mock.calls.at(-1)![0]).not.toHaveProperty('includeProfit');
    expect(Array.from(document.querySelectorAll('th')).map((t) => t.textContent)).not.toContain('Profit');
  });

  it('purchase history has no column menu and never asks for profit', async () => {
    const api = installApi({
      getPurchasesHistory: page([{ ...row(1), transactionCode: 'PUR-1', supplierInvoiceRef: null } as never]),
    });
    useAppStore.setState({ showProfit: true });
    render(<PurchaseHistoryPage />);
    await waitFor(() => bodyText().includes('PUR-1'));
    expect(label('Columns')).toBeNull();
    expect(api.getPurchasesHistory.mock.calls[0][0]).not.toHaveProperty('includeProfit');
    expect(Array.from(document.querySelectorAll('th')).map((t) => t.textContent)).not.toContain('Profit');
  });
});

describe('Bill drawer profit (FR-34)', () => {
  const mount = (showProfit: boolean) => {
    installApi({ getSaleDetails: profitDetail(), getBillEditGuard: saleGuard() });
    render(<BillDetailDrawer kind="SALE" billId={5} showProfit={showProfit} onClose={() => {}} />);
  };

  it('shows Cost / Profit per line and the reconciliation footer only when on', async () => {
    mount(true);
    await waitFor(() => !!q('[data-testid="profit-footer"]'));
    expect(Array.from(document.querySelectorAll('th')).map((t) => t.textContent)).toEqual(expect.arrayContaining(['Cost', 'Profit']));
    expect(Array.from(document.querySelectorAll('[data-testid="line-profit"]')).map((c) => c.textContent)).toEqual(['₹40.00', '₹20.00']);
    const f = q('[data-testid="profit-footer"]').textContent ?? '';
    expect(f).toContain('Lines profit');
    expect(f).toContain('60.00');
    expect(f).toContain('Charges');
    expect(f).toContain('12.00');
    expect(q('[data-testid="profit-total"]').textContent).toContain('53.00');
  });

  it('hides the profit columns and footer when off', async () => {
    mount(false);
    await waitFor(() => !!q('[data-testid="bill-actions"]'));
    expect(q('[data-testid="profit-footer"]')).toBeNull();
    expect(q('[data-testid="line-profit"]')).toBeNull();
    expect(Array.from(document.querySelectorAll('th')).map((t) => t.textContent)).not.toContain('Cost');
  });
});
