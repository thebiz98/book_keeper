import { describe, it, expect, beforeEach } from 'vitest';
import { render } from '../testUtils';
import { CreditDuesPage } from '@/pages/CreditDuesPage';
import { BillDetailDrawer } from '@/components/domain/BillDetailDrawer';
import { PartyLedgerDrawer } from '@/components/domain/PartyLedgerDrawer';
import { amountFromRows, rowsFromAllocations } from '@/components/domain/BatchSettlementForm';
import type { OpenBillRow, PartyDueSummary, SaleDetailView } from '@/types';
import { installApi, flush, waitFor, click, setInput, buttonByText, bodyText } from './helpers';

const party: PartyDueSummary = {
  partyId: 7, partyName: 'Sharma Traders', phone: '999', partyType: 'CUSTOMER', unsettledBillsCount: 3,
  earliestBillDate: '2026-01-01', totalInvoiced: 900, totalPaid: 0, balanceDue: 600,
};
const bill = (id: number, date: string, due: number): OpenBillRow => ({
  id, transactionCode: `SAL-${id}`, transactionDate: date, totalAmount: due, paidAmount: 0, balanceDue: due,
  paymentStatus: 'UNPAID', isOpeningBalance: false,
});
const BILLS = [bill(1, '2026-01-01', 100), bill(2, '2026-01-02', 200), bill(3, '2026-01-03', 300)];

function setupDues() {
  return installApi({
    getPartyDuesSummary: [party],
    getPartyOpenBills: BILLS,
    settleBillsBatch: { settlementIds: [1], billsTouched: [1] },
    // DD-56: the oldest-first split is the backend's; canned for the 250 case.
    previewSettlementAllocation: async () => [
      { transactionId: 1, amountPaid: 100 },
      { transactionId: 2, amountPaid: 150 },
    ],
  });
}

const amountInput = () => document.querySelector('input[aria-label="Settlement amount"]') as HTMLInputElement;
const rowInput = (code: string) => document.querySelector(`input[aria-label="Amount for ${code}"]`) as HTMLInputElement;
const tick = (code: string) => document.querySelector(`input[aria-label="Settle ${code}"]`) as HTMLInputElement;

async function openBatch() {
  render(<CreditDuesPage />);
  await waitFor(() => bodyText().includes('Sharma Traders'));
  click(buttonByText('Settle bills'));
  await waitFor(() => !!rowInput('SAL-3'));
}

describe('rowsFromAllocations / amountFromRows', () => {
  it('maps the backend split to rows and sums back', () => {
    const rows = rowsFromAllocations(BILLS, [
      { transactionId: 1, amountPaid: 100 },
      { transactionId: 2, amountPaid: 150 },
    ]);
    expect(rows[1]).toEqual({ checked: true, amount: 100 });
    expect(rows[2]).toEqual({ checked: true, amount: 150 });
    expect(rows[3]).toEqual({ checked: false, amount: 0 });
    expect(amountFromRows(rows)).toBe(250);
  });
});

describe('Settle bills form (D9)', () => {
  beforeEach(() => {
    document.body.innerHTML = '';
  });

  it('starts with every bill fully ticked and the full outstanding as the amount', async () => {
    setupDues();
    await openBatch();
    expect(amountInput().value).toBe('600');
    expect(tick('SAL-1').checked && tick('SAL-2').checked && tick('SAL-3').checked).toBe(true);
  });

  it('typing the amount shows the backend split of that amount', async () => {
    const api = setupDues();
    await openBatch();
    setInput(amountInput(), '250');
    await waitFor(() => rowInput('SAL-2').value === '150');
    expect(api.previewSettlementAllocation).toHaveBeenLastCalledWith(7, 'SALE', 250);
    expect(rowInput('SAL-1').value).toBe('100');
    expect(rowInput('SAL-2').value).toBe('150');
    expect(tick('SAL-3').checked).toBe(false);
    expect(rowInput('SAL-3').value).toBe('0');
  });

  it('editing a row and unticking update the amount (the sum of the entered rows)', async () => {
    setupDues();
    await openBatch();
    click(tick('SAL-3'));
    await flush();
    expect(amountInput().value).toBe('300');
    setInput(rowInput('SAL-1'), '40');
    await flush();
    expect(amountInput().value).toBe('240');
  });

  it('sends only the ticked rows and disables Save at zero', async () => {
    const api = setupDues();
    await openBatch();
    click(tick('SAL-1'));
    click(tick('SAL-2'));
    await flush();
    expect(amountInput().value).toBe('300');
    click(buttonByText('Settle bills', document.querySelector('[role="dialog"]')!));
    await flush();
    expect(api.settleBillsBatch.mock.calls[0][0].allocations).toEqual([{ transactionId: 3, amountPaid: 300 }]);
  });

  it('Save is disabled when nothing is ticked', async () => {
    const api = setupDues();
    await openBatch();
    click(tick('SAL-1'));
    click(tick('SAL-2'));
    click(tick('SAL-3'));
    await flush();
    const save = buttonByText('Settle bills', document.querySelector('[role="dialog"]')!);
    expect(save.disabled).toBe(true);
    click(save);
    await flush();
    expect(api.settleBillsBatch).not.toHaveBeenCalled();
  });
});

const detail = (balanceDue: number, partyId: number | null): SaleDetailView =>
  ({
    sale: {
      id: 5, transactionCode: 'SAL-1005', transactionDate: '2026-11-20', partyId, partyName: partyId ? 'Sharma' : null,
      paymentMode: 'CASH', paymentStatus: balanceDue > 0 ? 'PARTIAL' : 'PAID', totalAmount: 500, paidAmount: 500 - balanceDue,
      balanceDue, notes: null, createdAt: '', itemsSubtotal: 500, chargesTotal: 0, discountsTotal: 0, updatedAt: null,
    },
    party: null,
    settlements: [
      { id: 11, settlementDate: '2026-11-20', partyId, kind: 'UPFRONT', transactionType: 'SALE', transactionId: 5, returnId: null, amountPaid: 100, paymentMode: 'CASH', notes: null, createdAt: '' },
      { id: 12, settlementDate: '2026-11-21', partyId, kind: 'SETTLEMENT', transactionType: 'SALE', transactionId: 5, returnId: null, amountPaid: 150, paymentMode: 'CASH', notes: null, createdAt: '' },
    ],
    charges: [],
    items: [],
  }) as unknown as SaleDetailView;

const guard = {
  target: 'SALE', id: 5, code: 'SAL-1005', canEdit: true, canDelete: true, canChangeParty: true, reasons: [],
  upfrontAmount: 100, appliedAmount: 150, removedCashAmount: 100, removedCreditAmount: 0, lotLimits: [], settlementsToMove: { count: 0, amount: 0 },
};

describe('BillDetailDrawer settlement actions and Settle (D8, D11)', () => {
  beforeEach(() => {
    document.body.innerHTML = '';
  });

  it('shows Settle only with a balance and a party', async () => {
    installApi({ getSaleDetails: detail(250, 7), getBillEditGuard: guard });
    render(<BillDetailDrawer kind="SALE" billId={5} onClose={() => {}} />);
    await waitFor(() => bodyText().includes('SAL-1005'));
    expect(document.querySelector('[data-testid="bill-settle"]')).not.toBeNull();
    document.body.innerHTML = '';
    installApi({ getSaleDetails: detail(0, 7), getBillEditGuard: guard });
    render(<BillDetailDrawer kind="SALE" billId={5} onClose={() => {}} />);
    await waitFor(() => bodyText().includes('SAL-1005'));
    expect(document.querySelector('[data-testid="bill-settle"]')).toBeNull();
    document.body.innerHTML = '';
    installApi({ getSaleDetails: detail(250, null), getBillEditGuard: guard });
    render(<BillDetailDrawer kind="SALE" billId={5} onClose={() => {}} />);
    await waitFor(() => bodyText().includes('SAL-1005'));
    expect(document.querySelector('[data-testid="bill-settle"]')).toBeNull();
  });

  it('edit and delete icons exist on SETTLEMENT rows only', async () => {
    installApi({ getSaleDetails: detail(250, 7), getBillEditGuard: guard });
    render(<BillDetailDrawer kind="SALE" billId={5} onClose={() => {}} />);
    await waitFor(() => bodyText().includes('SAL-1005'));
    expect(document.querySelector('[data-testid="settlement-edit-12"]')).not.toBeNull();
    expect(document.querySelector('[data-testid="settlement-delete-12"]')).not.toBeNull();
    expect(document.querySelector('[data-testid="settlement-edit-11"]')).toBeNull();
    expect(document.querySelector('[data-testid="settlement-delete-11"]')).toBeNull();
  });

  it('edit saves through updateSettlement with the cap and reloads', async () => {
    const api = installApi({
      getSaleDetails: detail(250, 7),
      getBillEditGuard: guard,
      updateSettlement: { id: 12 },
    });
    render(<BillDetailDrawer kind="SALE" billId={5} onClose={() => {}} />);
    await waitFor(() => bodyText().includes('SAL-1005'));
    click(document.querySelector('[data-testid="settlement-edit-12"]')!);
    await waitFor(() => !!document.querySelector('[data-testid="settlement-edit"]'));
    const box = document.querySelector('[data-testid="settlement-edit"]')!;
    // The cap against the bill is the backend's: no client-side message while typing.
    setInput(box.querySelector('input[aria-label="Settlement amount"]')!, '401');
    await flush();
    expect(bodyText()).not.toContain('cannot exceed');
    setInput(box.querySelector('input[aria-label="Settlement amount"]')!, '400');
    await flush();
    click(buttonByText('Save', document.querySelector('[data-testid="settlement-edit"]')!));
    await flush();
    expect(api.updateSettlement).toHaveBeenCalledTimes(1);
    expect(api.updateSettlement.mock.calls[0][0]).toBe(12);
    expect(api.updateSettlement.mock.calls[0][1]).toMatchObject({ amountPaid: 400, settlementDate: '2026-11-21' });
    expect(api.getSaleDetails.mock.calls.length).toBeGreaterThanOrEqual(2);
  });

  it('delete is one danger confirm with no typed text', async () => {
    const api = installApi({
      getSaleDetails: detail(250, 7),
      getBillEditGuard: guard,
      deleteSettlement: { removedAmount: 150, transactionId: 5 },
    });
    render(<BillDetailDrawer kind="SALE" billId={5} onClose={() => {}} />);
    await waitFor(() => bodyText().includes('SAL-1005'));
    click(document.querySelector('[data-testid="settlement-delete-12"]')!);
    await flush();
    expect(bodyText()).toContain('cash records drop by ₹150.00');
    expect(document.querySelector('input[type="text"][placeholder*="Type"]')).toBeNull();
    click(buttonByText('Delete payment'));
    await flush();
    expect(api.deleteSettlement).toHaveBeenCalledWith(12);
  });

  it('Settle opens the per-bill modal and records through settleBillPayment', async () => {
    const api = installApi({
      getSaleDetails: detail(250, 7),
      getBillEditGuard: guard,
      settleBillPayment: { settlementIds: [1], billsTouched: [5] },
    });
    render(<BillDetailDrawer kind="SALE" billId={5} onClose={() => {}} />);
    await waitFor(() => bodyText().includes('SAL-1005'));
    click(document.querySelector('[data-testid="bill-settle"]')!);
    await flush();
    click(buttonByText('Record Settlement', document.querySelector('[role="dialog"]')!));
    await flush();
    expect(api.settleBillPayment.mock.calls[0][0]).toMatchObject({ partyId: 7, transactionId: 5, amountPaid: 250 });
  });
});

describe('PartyLedgerDrawer settlement actions', () => {
  beforeEach(() => {
    document.body.innerHTML = '';
  });

  it('lists SETTLEMENT payments with edit and delete', async () => {
    installApi({
      getPartyLedger: {
        party: { id: 7, name: 'Sharma', partyType: 'CUSTOMER' },
        currentBalance: 250,
        bills: [{ id: 5, transactionType: 'SALE', transactionCode: 'SAL-1005', transactionDate: '2026-11-20', totalAmount: 500, paidAmount: 250, balanceDue: 250, paymentStatus: 'PARTIAL' }],
        payments: detail(250, 7).settlements,
        returns: [],
        entries: [],
      },
    });
    render(<PartyLedgerDrawer partyId={7} onClose={() => {}} />);
    await waitFor(() => !!document.querySelector('[data-testid="party-settlements"]'));
    expect(document.querySelector('[data-testid="settlement-edit-12"]')).not.toBeNull();
    expect(document.querySelector('[data-testid="settlement-edit-11"]')).toBeNull();
    expect(document.querySelector('[data-testid="party-settlements"]')!.textContent).toContain('SAL-1005');
  });
});
