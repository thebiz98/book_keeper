import { describe, it, expect, vi, beforeEach } from 'vitest';
import { act } from 'react';
import { render } from '../testUtils';
import { PartiesPage } from '@/pages/PartiesPage';
import { __resetHotkeys } from '@/hooks/useHotkeys';
import { installApi, typeInto, press } from '../components/saleFixtures';

const settle = () => act(async () => { await Promise.resolve(); await Promise.resolve(); });
const click = (el: Element) =>
  act(() => {
    el.dispatchEvent(new MouseEvent('click', { bubbles: true, cancelable: true }));
  });
const dialog = () => document.querySelector('[role="dialog"]');
const dlgButton = (re: RegExp) =>
  Array.from(document.querySelectorAll<HTMLButtonElement>('[role="dialog"] button')).find((b) =>
    re.test(b.textContent ?? ''),
  )!;
const nameInput = () => document.querySelector<HTMLInputElement>('[role="dialog"] input')!;

async function openAddDialog() {
  const add = Array.from(document.querySelectorAll('button')).find((b) => /Add Party/.test(b.textContent ?? ''))!;
  click(add);
  await settle();
  typeInto(nameInput(), 'Ramesh');
}

describe('PartiesPage add/edit dialog', () => {
  beforeEach(() => __resetHotkeys());

  function setup(createParty = vi.fn(async () => 5)) {
    const api = installApi({ listParties: vi.fn(async () => []), createParty });
    render(<PartiesPage />);
    return api;
  }

  it('Enter in the Name input saves', async () => {
    const api = setup();
    await openAddDialog();
    press(nameInput(), 'Enter');
    await settle();
    expect(api.createParty).toHaveBeenCalledTimes(1);
  });

  it('clicking Save saves', async () => {
    const api = setup();
    await openAddDialog();
    click(dlgButton(/^Save/));
    await settle();
    expect(api.createParty).toHaveBeenCalledTimes(1);
  });

  it('clicking Cancel closes the dialog', async () => {
    setup();
    await openAddDialog();
    expect(dialog()).not.toBeNull();
    click(dlgButton(/^Cancel/));
    await settle();
    expect(dialog()).toBeNull();
  });

  it('shows a backend error inside the dialog', async () => {
    setup(
      vi.fn(async () => {
        throw { type: 'ValidationError', details: { field: 'name', message: 'Name already exists' } };
      }),
    );
    await openAddDialog();
    press(nameInput(), 'Enter');
    await settle();
    await settle();
    expect(document.querySelector('[role="dialog"] [role="alert"]')?.textContent).toContain('Name already exists');
  });
});
