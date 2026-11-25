import { describe, it, expect, beforeEach } from 'vitest';
import { render } from '../testUtils';
import { CreditDuesPage } from '@/pages/CreditDuesPage';
import { SettlementHistoryView } from '@/components/domain/SettlementHistoryView';
import type { SettlementHistoryPage, SettlementHistoryRow } from '@/types';
import { installApi, flush, waitFor, click, setInput, buttonByText, bodyText } from './helpers';

const row = (id: number, over: Partial<SettlementHistoryRow> = {}): SettlementHistoryRow => ({
  settlementId: id, settlementDate: '2026-11-10', partyId: 1, partyName: 'Ramesh Kumar', transactionType: 'SALE',
  transactionId: id, transactionCode: `SAL-100${id}`, isOpeningBalance: false, amount: 200, paymentMode: 'CASH',
  notes: null, ...over,
});
const PAGE: SettlementHistoryPage = {
  rows: [row(1), row(2, { transactionType: 'PURCHASE', transactionCode: 'PUR-1002', partyName: 'Suresh', amount: 300 })],
  totalReceived: 200, totalPaid: 300, totalCount: 2,
};

const btn = (label: string) => document.querySelector(`button[aria-label="${label}"]`) as HTMLButtonElement;

describe('Settlement History tab (FR-42)', () => {
  beforeEach(() => {
    document.body.innerHTML = '';
  });

  it('is a third tab on Credit & Dues and loads the month by default', async () => {
    const api = installApi({ getPartyDuesSummary: [], getSettlementHistory: PAGE });
    render(<CreditDuesPage />);
    await flush();
    click(buttonByText('Settlement History'));
    await waitFor(() => bodyText().includes('Ramesh Kumar'));
    const f = api.getSettlementHistory.mock.calls[0][0];
    expect(f.fromDate.endsWith('-01')).toBe(true);
    expect(f).toMatchObject({ side: null, partySearch: null, page: 1, pageSize: 25 });
    expect(document.querySelector('[data-testid="settlement-summary"]')?.textContent).toMatch(/2 settlements/);
  });

  it('side select and party search call the API with the right args', async () => {
    const api = installApi({ getSettlementHistory: PAGE });
    render(<SettlementHistoryView />);
    await waitFor(() => bodyText().includes('Suresh'));
    setInput(document.querySelector('select')!, 'PURCHASE');
    await waitFor(() => api.getSettlementHistory.mock.calls.some((c) => c[0].side === 'PURCHASE'));
    setInput(document.querySelector('input[aria-label="Search party"]')!, 'sur');
    await waitFor(() => api.getSettlementHistory.mock.calls.some((c) => c[0].partySearch === 'sur' && c[0].side === 'PURCHASE'));
  });

  it('shows side badges and the Opening balance label', async () => {
    installApi({
      getSettlementHistory: {
        ...PAGE,
        rows: [row(3, { isOpeningBalance: true, transactionCode: 'OB-0001' })],
        totalCount: 1,
      },
    });
    render(<SettlementHistoryView />);
    await waitFor(() => bodyText().includes('Opening balance'));
    expect(document.querySelector('[data-testid="side-badge"]')?.textContent).toBe('Received');
  });

  it('Edit opens the settlement form for that row', async () => {
    installApi({ getSettlementHistory: PAGE });
    render(<SettlementHistoryView />);
    await waitFor(() => !!btn('Edit settlement PUR-1002'));
    click(btn('Edit settlement PUR-1002'));
    await waitFor(() => !!document.querySelector('input[value="300"]'));
    expect(document.querySelector('input[value="2026-11-10"]')).toBeTruthy();
  });

  it('Delete confirms, calls deleteSettlement, then reloads and notifies', async () => {
    let calls = 0;
    const api = installApi({
      getSettlementHistory: async () => (++calls === 1 ? PAGE : { ...PAGE, rows: [PAGE.rows[1]], totalCount: 1 }),
      deleteSettlement: { removedAmount: 200, transactionId: 1 },
    });
    let changed = 0;
    render(<SettlementHistoryView onChanged={() => changed++} />);
    await waitFor(() => !!btn('Delete settlement SAL-1001'));
    click(btn('Delete settlement SAL-1001'));
    await waitFor(() => bodyText().includes('Delete the settlement of ₹200.00 on SAL-1001'));
    click(buttonByText('Delete'));
    await waitFor(() => api.deleteSettlement.mock.calls.length === 1);
    expect(api.deleteSettlement.mock.calls[0][0]).toBe(1);
    await waitFor(() => !bodyText().includes('Ramesh Kumar'));
    expect(changed).toBe(1);
  });

  it('shows the empty state', async () => {
    installApi({ getSettlementHistory: { rows: [], totalReceived: 0, totalPaid: 0, totalCount: 0 } });
    render(<SettlementHistoryView />);
    await waitFor(() => bodyText().includes('No settlements match the filters.'));
  });

  it('keyboard: Down then Delete opens the confirm', async () => {
    installApi({ getSettlementHistory: PAGE });
    render(<SettlementHistoryView />);
    await waitFor(() => bodyText().includes('Ramesh Kumar'));
    const key = (k: string) => {
      window.dispatchEvent(new KeyboardEvent('keydown', { key: k, bubbles: true, cancelable: true }));
      return flush();
    };
    await key('ArrowDown');
    await key('Delete');
    await waitFor(() => bodyText().includes('Delete the settlement of'));
  });
});
