import { describe, it, expect, beforeEach } from 'vitest';
import { render } from '../testUtils';
import { PartiesPage } from '@/pages/PartiesPage';
import { __resetHotkeys } from '@/hooks/useHotkeys';
import type { PartyListRow } from '@/types';
import { installApi, flush, click, buttonByText } from './helpers';

const row = (over: Partial<PartyListRow> = {}): PartyListRow => ({
  id: 7, name: 'Ramesh', phone: null, partyType: 'CUSTOMER', notes: null, isActive: true, createdAt: '', updatedAt: '',
  outstandingBalance: 300, transactionCount: 1, ...over,
});

const balanceCell = () =>
  Array.from(document.querySelectorAll('td span.font-mono')).find((s) => /300\.00/.test(s.textContent ?? ''))!;

describe('Parties page: dues colour (FR-49)', () => {
  beforeEach(() => {
    document.body.innerHTML = '';
    __resetHotkeys();
  });

  it('shows customer dues in green', async () => {
    installApi({ listParties: [row()] });
    render(<PartiesPage />);
    await flush();
    expect(balanceCell().className).toContain('text-emerald-600');
  });

  it('shows vendor dues in red', async () => {
    installApi({ listParties: [row({ partyType: 'VENDOR', name: 'Supplier' })] });
    render(<PartiesPage />);
    await flush();
    click(buttonByText(/Vendors/));
    await flush();
    expect(balanceCell().className).toContain('text-rose-600');
  });
});
