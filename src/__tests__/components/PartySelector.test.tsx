import { describe, it, expect, vi, beforeEach } from 'vitest';
import { act } from 'react';
import { render } from '../testUtils';
import { PartySelector } from '@/components/domain/PartySelector';
import { __resetHotkeys } from '@/hooks/useHotkeys';
import { installApi, makeParty, typeInto, press } from './saleFixtures';

const trigger = () => document.querySelector<HTMLButtonElement>('button[role="combobox"]')!;
const search = () => document.querySelector<HTMLInputElement>('input[role="combobox"]')!;
const optionTexts = () => Array.from(document.querySelectorAll('[role="option"]')).map((o) => o.textContent ?? '');
const settle = () => act(async () => { await Promise.resolve(); await Promise.resolve(); });

describe('PartySelector', () => {
  beforeEach(() => __resetHotkeys());

  const parties = [
    makeParty({ id: 1, name: 'Anil Traders', phone: '111', outstandingBalance: 250 }),
    makeParty({ id: 2, name: 'Bharat Stores', phone: '222' }),
  ];

  it('is keyboard operable: open with ArrowDown, filter, arrow + Enter selects', async () => {
    installApi({ listParties: vi.fn(async () => parties) });
    const onSelect = vi.fn();
    render(<PartySelector partyType="CUSTOMER" onSelectParty={onSelect} />);
    await settle();

    trigger().focus();
    press(trigger(), 'ArrowDown');
    await act(async () => { await new Promise((r) => setTimeout(r, 5)); });
    expect(trigger().getAttribute('aria-expanded')).toBe('true');
    expect(document.activeElement).toBe(search());
    expect(document.querySelector('[role="listbox"]')).not.toBeNull();
    // walk-in, 2 parties, create row
    expect(optionTexts()).toHaveLength(4);

    press(search(), 'ArrowDown'); // Anil
    press(search(), 'ArrowDown'); // Bharat
    press(search(), 'Enter');
    expect(onSelect).toHaveBeenCalledTimes(1);
    expect(onSelect.mock.calls[0][0].id).toBe(2);
    expect(document.querySelector('[role="listbox"]')).toBeNull();
    expect(document.activeElement).toBe(trigger());
  });

  it('filters by name or phone', async () => {
    const searchParties = vi.fn(async (q: string) =>
      parties.filter((p) => p.name.toLowerCase().includes(q.toLowerCase()) || (p.phone ?? '').includes(q))
    );
    installApi({ listParties: vi.fn(async () => parties), searchParties });
    render(<PartySelector partyType="CUSTOMER" onSelectParty={vi.fn()} />);
    await settle();
    trigger().click();
    await settle();
    typeInto(search(), '222');
    await act(async () => { await new Promise((r) => setTimeout(r, 80)); });
    expect(searchParties).toHaveBeenCalledWith('222', 'CUSTOMER');
    expect(optionTexts().some((t) => t.includes('Bharat'))).toBe(true);
    expect(optionTexts().some((t) => t.includes('Anil'))).toBe(false);
  });

  it('shows the outstanding balance pill for the selected party', async () => {
    installApi({ listParties: vi.fn(async () => parties) });
    render(<PartySelector partyType="CUSTOMER" selectedPartyId={1} onSelectParty={vi.fn()} />);
    await settle();
    const pill = document.querySelector('[data-testid="outstanding-pill"]');
    expect(pill?.textContent).toContain('250');
  });

  it('Esc closes the list and Walk-in selects null', async () => {
    installApi({ listParties: vi.fn(async () => parties) });
    const onSelect = vi.fn();
    render(<PartySelector partyType="CUSTOMER" selectedPartyId={1} onSelectParty={onSelect} />);
    await settle();
    trigger().click();
    await settle();
    press(window, 'Escape');
    expect(document.querySelector('[role="listbox"]')).toBeNull();

    trigger().click();
    await settle();
    press(search(), 'Enter'); // first row is walk-in
    expect(onSelect).toHaveBeenCalledWith(null);
  });

  it('Alt+C opens inline create; Enter calls createParty and selects the new party', async () => {
    const created = makeParty({ id: 99, name: 'New Guy' });
    const listParties = vi.fn().mockResolvedValueOnce(parties).mockResolvedValue([...parties, created]);
    const api = installApi({ listParties, createParty: vi.fn(async () => 99) });
    const onSelect = vi.fn();
    const onCreated = vi.fn();
    render(<PartySelector partyType="CUSTOMER" onSelectParty={onSelect} onCreated={onCreated} />);
    await settle();

    press(window, 'c', { altKey: true, code: 'KeyC' });
    const name = document.querySelector<HTMLInputElement>('input[aria-label="New customer name"]')!;
    expect(name).not.toBeNull();
    typeInto(name, 'New Guy');
    press(name, 'Enter');
    await settle();
    await settle();
    expect(api.createParty).toHaveBeenCalledWith({ name: 'New Guy', phone: null, partyType: 'CUSTOMER', notes: null });
    expect(onSelect).toHaveBeenCalledTimes(1);
    expect(onSelect.mock.calls[0][0].id).toBe(99);
    expect(onCreated).toHaveBeenCalled();
  });

  it('uses Alt+V for vendors and shows a backend error inline', async () => {
    const api = installApi({
      listParties: vi.fn(async () => []),
      createParty: vi.fn(async () => {
        throw { type: 'ValidationError', details: { field: 'name', message: 'Name already exists' } };
      }),
    });
    render(<PartySelector partyType="VENDOR" onSelectParty={vi.fn()} />);
    await settle();
    press(window, 'c', { altKey: true, code: 'KeyC' });
    expect(document.querySelector('input[aria-label="New vendor name"]')).toBeNull();
    press(window, 'v', { altKey: true, code: 'KeyV' });
    const name = document.querySelector<HTMLInputElement>('input[aria-label="New vendor name"]')!;
    typeInto(name, 'Dup');
    press(name, 'Enter');
    await settle();
    expect(api.createParty).toHaveBeenCalledTimes(1);
    expect(document.querySelector('[role="alert"]')?.textContent).toContain('Name already exists');
  });

  describe('inline create buttons (FR-22)', () => {
    const groupBtn = (re: RegExp) =>
      Array.from(document.querySelectorAll<HTMLButtonElement>('[role="group"] button')).find((b) =>
        re.test(b.textContent ?? ''),
      )!;
    const nameInput = () =>
      document.querySelector<HTMLInputElement>('input[aria-label="New customer name"]');

    it('clicking Save creates the party and selects it', async () => {
      const created = makeParty({ id: 99, name: 'New Guy' });
      const listParties = vi.fn().mockResolvedValueOnce(parties).mockResolvedValue([...parties, created]);
      const api = installApi({ listParties, createParty: vi.fn(async () => 99) });
      const onSelect = vi.fn();
      render(<PartySelector partyType="CUSTOMER" onSelectParty={onSelect} />);
      await settle();
      press(window, 'c', { altKey: true, code: 'KeyC' });
      typeInto(nameInput()!, 'New Guy');
      act(() => { groupBtn(/^Save/).click(); });
      await settle();
      await settle();
      expect(api.createParty).toHaveBeenCalledTimes(1);
      expect(onSelect.mock.calls[0][0].id).toBe(99);
    });

    it('Save is disabled while the name is empty', async () => {
      installApi({ listParties: vi.fn(async () => parties) });
      render(<PartySelector partyType="CUSTOMER" onSelectParty={vi.fn()} />);
      await settle();
      press(window, 'c', { altKey: true, code: 'KeyC' });
      expect(groupBtn(/^Save/).disabled).toBe(true);
      typeInto(nameInput()!, 'X');
      expect(groupBtn(/^Save/).disabled).toBe(false);
    });

    it('Cancel closes the form and refocuses the trigger', async () => {
      installApi({ listParties: vi.fn(async () => parties) });
      render(<PartySelector partyType="CUSTOMER" onSelectParty={vi.fn()} />);
      await settle();
      press(window, 'c', { altKey: true, code: 'KeyC' });
      expect(nameInput()).not.toBeNull();
      act(() => { groupBtn(/^Cancel/).click(); });
      expect(nameInput()).toBeNull();
      expect(document.activeElement).toBe(trigger());
    });

    it('Esc closes the create form', async () => {
      installApi({ listParties: vi.fn(async () => parties) });
      render(<PartySelector partyType="CUSTOMER" onSelectParty={vi.fn()} />);
      await settle();
      press(window, 'c', { altKey: true, code: 'KeyC' });
      expect(nameInput()).not.toBeNull();
      press(window, 'Escape');
      expect(nameInput()).toBeNull();
    });
  });
});
