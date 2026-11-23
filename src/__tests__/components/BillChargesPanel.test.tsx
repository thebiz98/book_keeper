import { describe, it, expect, beforeEach, vi } from 'vitest';
import { act, createRef } from 'react';
import { render } from '../testUtils';
import { BillChargesPanel, type BillChargesPanelHandle } from '@/components/domain/BillChargesPanel';
import { MockApiClient } from '@/api/mock/mockClient';
import { setApiClient } from '@/api';
import { __resetHotkeys } from '@/hooks/useHotkeys';
import type { ApiClient } from '@/types';
import { waitFor, flush, click, setInput, buttonByText } from '../pages/helpers';

const q = (sel: string) => document.querySelector<HTMLElement>(sel);
const tid = (id: string) => q(`[data-testid="${id}"]`);
const key = (el: Element, k: string) =>
  act(() => {
    el.dispatchEvent(new KeyboardEvent('keydown', { key: k, bubbles: true, cancelable: true }));
  });

function setup(side: 'SALE' | 'PURCHASE' = 'SALE') {
  const mock = new MockApiClient();
  const createSpy = vi.spyOn(mock, 'createChargeType');
  setApiClient(mock as unknown as ApiClient);
  const onAdd = vi.fn(() => true);
  const onDone = vi.fn();
  const ref = createRef<BillChargesPanelHandle>();
  render(
    <BillChargesPanel
      ref={ref}
      side={side}
      lines={[]}
      onAdd={onAdd}
      onUpdateAmount={() => true}
      onRemove={() => {}}
      onDone={onDone}
    />
  );
  return { ref, onAdd, onDone, createSpy };
}

async function typeText(text: string) {
  const input = tid('charge-type-input') as HTMLInputElement;
  setInput(input, text);
  await waitFor(() => !!q('[role="option"]'));
  return input;
}

describe('BillChargesPanel', () => {
  beforeEach(() => {
    document.body.innerHTML = '';
    __resetHotkeys();
  });

  it('open() focuses the charge type input', async () => {
    const { ref } = setup();
    act(() => ref.current!.open());
    await waitFor(() => document.activeElement === tid('charge-type-input'));
  });

  it('filters by side: "tr" on a sale shows Transport; Carrier Fee is purchase-only', async () => {
    const { ref } = setup('SALE');
    act(() => ref.current!.open());
    await waitFor(() => !!tid('charge-type-input'));
    await typeText('tr');
    const opts = Array.from(document.querySelectorAll('[data-testid="charge-type-option"]')).map((o) => o.textContent);
    expect(opts.some((t) => t!.includes('Transport'))).toBe(true);
    expect(opts.some((t) => t!.includes('Carrier'))).toBe(false);
  });

  it('Enter selects, prefills the default amount, and Enter on Amount adds the line', async () => {
    const { ref, onAdd, onDone } = setup();
    act(() => ref.current!.open());
    await waitFor(() => !!tid('charge-type-input'));
    const input = await typeText('tr');
    key(input, 'Enter');
    await waitFor(() => !!tid('charge-amount'));
    const amount = tid('charge-amount') as HTMLInputElement;
    expect(amount.value).toBe('50');
    key(amount, 'Enter');
    expect(onAdd).toHaveBeenCalledWith({ chargeTypeId: 1, name: 'Transport', effect: 'CHARGE', amount: 50 });
    expect(onDone).toHaveBeenCalled();
    expect(tid('charge-entry-row')).toBeNull();
  });

  it('no match offers Create; Enter opens the modal with visible Save and Cancel; Save adds a line', async () => {
    const { ref, onAdd, createSpy } = setup();
    act(() => ref.current!.open());
    await waitFor(() => !!tid('charge-type-input'));
    const input = tid('charge-type-input') as HTMLInputElement;
    setInput(input, 'Packing');
    await waitFor(() => (document.body.textContent ?? '').includes('Create "Packing"'));
    key(input, 'Enter');
    await waitFor(() => !!tid('charge-type-name'));
    expect((tid('charge-type-name') as HTMLInputElement).value).toBe('Packing');
    expect(buttonByText(/^Cancel/)).toBeTruthy();
    click(buttonByText(/^Save/));
    await waitFor(() => !!tid('charge-amount'));
    expect(createSpy).toHaveBeenCalledWith(expect.objectContaining({ name: 'Packing', effect: 'CHARGE', appliesTo: 'BOTH' }));
    setInput(tid('charge-amount')!, '12');
    key(tid('charge-amount')!, 'Enter');
    expect(onAdd).toHaveBeenCalledWith(expect.objectContaining({ name: 'Packing', amount: 12 }));
  });

  it('Cancel closes the modal without creating or adding', async () => {
    const { ref, onAdd, createSpy } = setup();
    act(() => ref.current!.open());
    await waitFor(() => !!tid('charge-type-input'));
    const input = tid('charge-type-input') as HTMLInputElement;
    setInput(input, 'Packing');
    await waitFor(() => (document.body.textContent ?? '').includes('Create "Packing"'));
    key(input, 'Enter');
    await waitFor(() => !!tid('charge-type-name'));
    click(buttonByText(/^Cancel/));
    await flush(20);
    expect(tid('charge-type-name')).toBeNull();
    expect(createSpy).not.toHaveBeenCalled();
    expect(onAdd).not.toHaveBeenCalled();
  });

  it('rejects a zero amount without adding', async () => {
    const { ref, onAdd } = setup();
    act(() => ref.current!.open());
    await waitFor(() => !!tid('charge-type-input'));
    const input = await typeText('cash');
    key(input, 'Enter');
    await waitFor(() => !!tid('charge-amount'));
    setInput(tid('charge-amount')!, '0');
    key(tid('charge-amount')!, 'Enter');
    expect(onAdd).not.toHaveBeenCalled();
    expect(document.body.textContent).toContain('greater than 0');
  });
});
