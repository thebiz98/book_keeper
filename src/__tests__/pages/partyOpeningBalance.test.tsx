import { describe, it, expect, beforeEach } from 'vitest';
import { render } from '../testUtils';
import { PartiesPage } from '@/pages/PartiesPage';
import { BillDetailDrawer } from '@/components/domain/BillDetailDrawer';
import { __resetHotkeys } from '@/hooks/useHotkeys';
import { todayLocal } from '@/lib/dates';
import type { BillEditGuard, PartyListRow, SaleDetailView } from '@/types';
import { installApi, flush, waitFor, click, setInput, bodyText, buttonByText } from './helpers';

const dlg = () => document.querySelector('[role="dialog"]')!;
const dlgInput = (type: string) => dlg().querySelector<HTMLInputElement>(`input[type="${type}"]`)!;
const nameInput = () => dlg().querySelector<HTMLInputElement>('input')!;

const row = (over: Partial<PartyListRow> = {}): PartyListRow => ({
  id: 7, name: 'Ramesh', phone: null, partyType: 'CUSTOMER', notes: null, isActive: true, createdAt: '', updatedAt: '',
  outstandingBalance: 0, transactionCount: 0, ...over,
});

async function openAdd() {
  click(buttonByText(/Add Party/));
  await flush();
  setInput(nameInput(), 'Ramesh');
}

describe('Party dialog: opening balance (FR-35)', () => {
  beforeEach(() => {
    document.body.innerHTML = '';
    __resetHotkeys();
  });

  it('sends openingBalance with the amount and the As of date', async () => {
    const api = installApi({ listParties: [], createParty: 5 });
    render(<PartiesPage />);
    await openAdd();
    expect(dlgInput('date').value).toBe(todayLocal());
    expect(dlgInput('date').max).toBe(todayLocal());
    setInput(dlgInput('number'), '500');
    setInput(dlgInput('date'), '2026-04-01');
    click(buttonByText(/^Save/, dlg()));
    await flush();
    expect(api.createParty).toHaveBeenCalledTimes(1);
    expect(api.createParty.mock.calls[0][0]).toMatchObject({
      name: 'Ramesh',
      partyType: 'CUSTOMER',
      openingBalance: { amount: 500, asOfDate: '2026-04-01' },
    });
  });

  it('omits openingBalance when the amount is empty', async () => {
    const api = installApi({ listParties: [], createParty: 5 });
    render(<PartiesPage />);
    await openAdd();
    click(buttonByText(/^Save/, dlg()));
    await flush();
    expect(api.createParty.mock.calls[0][0].openingBalance).toBeUndefined();
  });

  it('the amount label follows the party type', async () => {
    installApi({ listParties: [] });
    render(<PartiesPage />);
    await openAdd();
    expect(dlg().textContent).toContain('Customer owes you (₹)');
    setInput(dlg().querySelector('select')!, 'VENDOR');
    expect(dlg().textContent).toContain('You owe this vendor (₹)');
    expect(dlg().textContent).not.toContain('Customer owes you (₹)');
  });

  it('edit prefills the opening bill and shows what is already settled', async () => {
    const p = row({
      outstandingBalance: 300,
      openingBalance: { billId: 3, transactionCode: 'OB-0001', amount: 500, asOfDate: '2026-04-01', settledAmount: 200 },
    });
    const api = installApi({ listParties: [p], updateParty: undefined });
    render(<PartiesPage />);
    await waitFor(() => bodyText().includes('Ramesh'));
    click(document.querySelector('button[title="Edit"]')!);
    await flush();
    expect(dlgInput('number').value).toBe('500');
    expect(dlgInput('date').value).toBe('2026-04-01');
    expect(dlg().querySelector('[data-testid="opening-settled"]')!.textContent).toContain('₹200 already settled');
    expect(dlg().querySelector('select')!.disabled).toBe(true);
    // Emptying the field asks the backend to remove the bill.
    setInput(dlgInput('number'), '');
    click(buttonByText(/^Save/, dlg()));
    await flush();
    expect(api.updateParty.mock.calls[0][1].openingBalance).toMatchObject({ amount: 0 });
  });

  it('shows a backend opening-balance error inside the dialog', async () => {
    installApi({
      listParties: [],
      createParty: async () => {
        throw { type: 'ValidationError', details: { field: 'opening_balance.amount', message: '₹200 is already settled against the opening balance' } };
      },
    });
    render(<PartiesPage />);
    await openAdd();
    setInput(dlgInput('number'), '10');
    click(buttonByText(/^Save/, dlg()));
    await flush();
    expect(dlg().textContent).toContain('already settled against the opening balance');
  });
});

const openingSale = (): SaleDetailView =>
  ({
    sale: {
      id: 9, transactionCode: 'OB-0001', transactionDate: '2026-04-01', partyId: 7, partyName: 'Ramesh', paymentMode: null,
      paymentStatus: 'UNPAID', totalAmount: 500, paidAmount: 0, balanceDue: 500, notes: null, createdAt: '', itemsSubtotal: 500,
      chargesTotal: 0, discountsTotal: 0, updatedAt: null, isOpeningBalance: true,
    },
    party: null,
    settlements: [],
    charges: [],
    items: [],
    profitSummary: { linesProfit: 0, charges: 0, returnAdjustment: 0, total: 0 },
  }) as unknown as SaleDetailView;

const lockedGuard = (): BillEditGuard => ({
  target: 'SALE', id: 9, code: 'OB-0001', canEdit: false, canDelete: false, canChangeParty: false,
  reasons: [{ code: 'OPENING_BALANCE', message: 'OB-0001 is the party opening balance.', blocksEdit: true, blocksDelete: true }],
  upfrontAmount: 0, appliedAmount: 0, removedCashAmount: 0, removedCreditAmount: 0, lotLimits: [], settlementsToMove: { count: 0, amount: 0 },
});

describe('Opening-balance bill drawer (FR-35)', () => {
  beforeEach(() => {
    document.body.innerHTML = '';
  });

  it('is read-only (no Edit / Delete / Print) and keeps Settle', async () => {
    installApi({ getSaleDetails: openingSale(), getBillEditGuard: lockedGuard() });
    render(<BillDetailDrawer kind="SALE" billId={9} onClose={() => {}} />);
    await waitFor(() => bodyText().includes('Opening balance OB-0001'));
    await waitFor(() => !!document.querySelector('[data-testid="bill-actions"]'));
    expect(document.querySelector('[data-testid="bill-edit"]')).toBeNull();
    expect(document.querySelector('[data-testid="bill-delete"]')).toBeNull();
    expect(document.querySelector('[data-testid="bill-print"]')).toBeNull();
    expect(document.querySelector('[data-testid="bill-settle"]')).not.toBeNull();
    expect(bodyText()).toContain('As of:');
  });
});
