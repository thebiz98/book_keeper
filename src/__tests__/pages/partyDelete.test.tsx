import { describe, it, expect, beforeEach } from 'vitest';
import { render } from '../testUtils';
import { PartiesPage } from '@/pages/PartiesPage';
import { CreditDuesPage } from '@/pages/CreditDuesPage';
import { __resetHotkeys } from '@/hooks/useHotkeys';
import type { DeletedPartyRow, PartyDeletePreview, PartyDueSummary, PartyListRow } from '@/types';
import { installApi, flush, waitFor, click, setInput, buttonByText, bodyText } from './helpers';

const row: PartyListRow = {
  id: 7, name: 'Ramesh', phone: '98765', partyType: 'CUSTOMER', notes: null, isActive: true, createdAt: '', updatedAt: '',
  outstandingBalance: 0, transactionCount: 2,
};

const preview = (over: Partial<PartyDeletePreview> = {}): PartyDeletePreview => ({
  partyId: 7, name: 'Ramesh', partyType: 'CUSTOMER', mode: 'SOFT', openBillsCount: 0, balanceDueTotal: 0,
  canDelete: true, reason: null, reasonsUsed: ['2 bills'], ...over,
});

const submit = () => document.body.querySelector('[data-testid="delete-confirm-submit"]') as HTMLButtonElement;

async function openDelete(over: Record<string, unknown> = {}) {
  const api = installApi({
    listParties: [row],
    previewPartyDelete: preview(),
    deleteParty: { mode: 'SOFT' },
    ...over,
  });
  render(<PartiesPage />);
  await waitFor(() => bodyText().includes('Ramesh'));
  click(document.querySelector('button[title="Edit"]')!);
  await flush();
  click(document.querySelector('[data-testid="party-delete-button"]')!);
  await flush();
  return api;
}

describe('Delete party (FR-46, DD-80)', () => {
  beforeEach(() => {
    document.body.innerHTML = '';
    __resetHotkeys();
  });

  it('is not offered when adding a party', async () => {
    installApi({ listParties: [row] });
    render(<PartiesPage />);
    await waitFor(() => bodyText().includes('Ramesh'));
    click(buttonByText(/Add Party/));
    await flush();
    expect(document.querySelector('[data-testid="party-delete-button"]')).toBeNull();
  });

  it('SOFT: shows the kept-for-history text, deletes on confirm and notes where to restore', async () => {
    const api = await openDelete();
    expect(api.previewPartyDelete).toHaveBeenCalledWith(7);
    expect(document.body.querySelector('[data-testid="delete-soft-text"]')?.textContent).toBe(
      'Kept for its past bills and payments; hidden from pickers and the Parties list.'
    );
    expect(submit().textContent).toContain('Delete party');
    click(submit());
    await flush();
    expect(api.deleteParty).toHaveBeenCalledWith(7);
    expect(document.body.querySelector('[data-testid="parties-notice"]')?.textContent).toContain(
      "Deleted 'Ramesh'. Restore it from Deleted parties."
    );
    expect(api.listParties.mock.calls.length).toBeGreaterThan(1);
  });

  it('HARD: says it is removed permanently and the button says so', async () => {
    const api = await openDelete({
      previewPartyDelete: preview({ mode: 'HARD', reasonsUsed: [] }),
      deleteParty: { mode: 'HARD' },
    });
    expect(document.body.querySelector('[data-testid="delete-hard-text"]')?.textContent).toBe(
      "'Ramesh' has no bills or payments. It will be removed permanently."
    );
    expect(submit().textContent).toContain('Delete permanently');
    click(submit());
    await flush();
    expect(api.deleteParty).toHaveBeenCalledWith(7);
    expect(document.body.querySelector('[data-testid="parties-notice"]')?.textContent).toContain('permanently');
  });

  it('blocked: shows the backend dues sentence and disables the button', async () => {
    const reason = '₹300 due on 2 bills. Settle them first.';
    const api = await openDelete({
      previewPartyDelete: preview({ canDelete: false, openBillsCount: 2, balanceDueTotal: 300, reason }),
    });
    expect(submit().disabled).toBe(true);
    expect(document.body.querySelector('[data-testid="delete-confirm-blocked"]')?.textContent).toBe(reason);
    click(submit());
    await flush();
    expect(api.deleteParty).not.toHaveBeenCalled();
  });
});

describe('Deleted parties dialog', () => {
  beforeEach(() => {
    document.body.innerHTML = '';
    __resetHotkeys();
  });

  const del = (id: number, name: string, partyType: 'CUSTOMER' | 'VENDOR', phone: string | null): DeletedPartyRow => ({
    partyId: id, name, partyType, phone, deletedAt: '2026-11-25T10:00:00Z',
  });

  it('lists customers and vendors together, searches in the backend and restores a row', async () => {
    const listDeletedParties = async (s: string | null) => ({
      items: s
        ? [del(9, 'Gupta Supplies', 'VENDOR', '91234')]
        : [del(8, 'Sharma Stores', 'CUSTOMER', '98765'), del(9, 'Gupta Supplies', 'VENDOR', '91234')],
      total: s ? 1 : 2,
      page: 1,
      limit: 10,
    });
    const api = installApi({ listParties: [], listDeletedParties, restoreParty: undefined });
    render(<PartiesPage />);
    await flush();
    click(document.querySelector('[data-testid="deleted-parties-button"]')!);
    await flush();
    expect(api.listDeletedParties).toHaveBeenCalledWith('', 1, 10);
    const rows = document.body.querySelectorAll('[data-testid="deleted-row"]');
    expect(rows.length).toBe(2);
    expect(document.body.textContent).toContain('Type');
    expect(rows[0].textContent).toContain('Customer');
    expect(rows[1].textContent).toContain('Vendor');

    setInput(document.body.querySelector('[data-testid="deleted-records-search"]')!, 'gupta');
    await flush(400);
    expect(api.listDeletedParties).toHaveBeenLastCalledWith('gupta', 1, 10);
    expect(document.body.querySelectorAll('[data-testid="deleted-row"]').length).toBe(1);

    const listCalls = api.listParties.mock.calls.length;
    click(document.body.querySelector('[data-testid="deleted-restore-9"]')!);
    await flush();
    expect(api.restoreParty).toHaveBeenCalledWith(9);
    expect(api.listParties.mock.calls.length).toBeGreaterThan(listCalls);
  });
});

describe('Credit & Dues: deleted party badge', () => {
  beforeEach(() => {
    document.body.innerHTML = '';
  });

  it('shows a Deleted badge and no Restore action', async () => {
    const due: PartyDueSummary = {
      partyId: 7, partyName: 'Old Cust', phone: null, partyType: 'CUSTOMER', unsettledBillsCount: 1,
      earliestBillDate: '2026-01-01', totalInvoiced: 100, totalPaid: 0, balanceDue: 100, isActive: false,
    };
    installApi({ getPartyDuesSummary: [due] });
    render(<CreditDuesPage />);
    await waitFor(() => bodyText().includes('Old Cust'));
    expect(document.body.querySelector('[data-testid="party-deleted-badge"]')?.textContent).toBe('Deleted');
    expect(Array.from(document.body.querySelectorAll('button')).some((b) => /Restore/.test(b.textContent ?? ''))).toBe(false);
    expect(bodyText()).toContain('Settle bills');
  });
});
