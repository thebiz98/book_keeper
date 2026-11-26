import { describe, it, expect, vi, beforeEach } from 'vitest';
import { act } from 'react';
import { render } from '../testUtils';
import { RapidBillingRow } from '@/components/domain/RapidBillingRow';
import { __resetHotkeys } from '@/hooks/useHotkeys';
import type { PreviewSaleLinePayload } from '@/types';
import { installApi, waitFor, click, setInput } from './helpers';
import { makeItem } from '../inventory/testHelpers';

// FR-47 / DD-82: the entry row's Total box sets the line's rate. Every figure is the backend's
// (DD-56); this double stands in for `preview_sale_line`.
const pencil = makeItem({ itemId: 1, name: 'Pencil', currentStock: 50, defaultSalePrice: 102.3, bundles: [] });

const previewSaleLine = async (p: PreviewSaleLinePayload) => {
  const unitPrice = p.enteredTotal != null && p.bundleQuantity > 0 ? p.enteredTotal / p.bundleQuantity : p.unitPrice;
  return { lineTotal: Math.round(p.bundleQuantity * unitPrice * 100) / 100, unitPrice };
};
const noLots = { lots: [], drawCost: 0, available: 50, shortfall: 0 };

const tid = <T extends HTMLElement = HTMLElement>(id: string) => document.querySelector<T>(`[data-testid="${id}"]`);
const key = (el: Element, k: string) =>
  act(() => {
    el.dispatchEvent(new KeyboardEvent('keydown', { key: k, bubbles: true, cancelable: true }));
  });

async function pickPencil() {
  setInput(tid('item-search-input')!, 'pen');
  await waitFor(() => !!document.querySelector('[role="option"]'));
  key(tid('item-search-input')!, 'Enter');
  await waitFor(() => tid<HTMLInputElement>('item-search-input')!.value === 'Pencil');
}

describe('RapidBillingRow Total box (FR-47, DD-82)', () => {
  beforeEach(() => {
    document.body.innerHTML = '';
    __resetHotkeys();
  });

  it('is prefilled from the backend, follows qty until typed in, and Enter in Rate moves to it', async () => {
    const api = installApi({ searchItemsFts: [pencil], previewLotDraw: noLots, previewSaleLine });
    render(<RapidBillingRow />);
    await pickPencil();
    await waitFor(() => tid<HTMLInputElement>('total-input')!.value === '102.30');
    setInput(tid('quantity-input')!, '10');
    await waitFor(() => tid<HTMLInputElement>('total-input')!.value === '1023.00');
    expect(api.previewSaleLine).toHaveBeenCalledWith(expect.objectContaining({ bundleQuantity: 10, unitPrice: 102.3 }));
    expect(tid('line-new-rate')).toBeNull();

    key(tid('rate-input')!, 'Enter');
    expect(document.activeElement).toBe(tid('total-input'));
  });

  it('a typed total shows the new rate and adds the line at that rate', async () => {
    installApi({ searchItemsFts: [pencil], previewLotDraw: noLots, previewSaleLine });
    const onAddLine = vi.fn((_e: { quantity: number; rate: number }) => true);
    render(<RapidBillingRow onAddLine={onAddLine} />);
    await pickPencil();
    setInput(tid('quantity-input')!, '10');
    await waitFor(() => tid<HTMLInputElement>('total-input')!.value === '1023.00');
    setInput(tid('total-input')!, '1000');
    await waitFor(() => !!tid('line-new-rate'));
    expect(tid('line-new-rate')!.textContent).toContain('Rate becomes');
    expect(tid('line-new-rate')!.textContent).toContain('100.00');

    key(tid('total-input')!, 'Enter');
    await waitFor(() => onAddLine.mock.calls.length === 1);
    expect(onAddLine).toHaveBeenCalledWith(expect.objectContaining({ quantity: 10, rate: 100 }));
    await waitFor(() => tid<HTMLInputElement>('item-search-input')!.value === '');
  });

  it('uses the rate of exactly the typed total even before the live preview catches up', async () => {
    installApi({ searchItemsFts: [pencil], previewLotDraw: noLots, previewSaleLine });
    const onAddLine = vi.fn((_e: { quantity: number; rate: number }) => true);
    render(<RapidBillingRow onAddLine={onAddLine} />);
    await pickPencil();
    setInput(tid('quantity-input')!, '3');
    setInput(tid('total-input')!, '1000');
    key(tid('total-input')!, 'Enter');
    await waitFor(() => onAddLine.mock.calls.length === 1);
    expect(onAddLine.mock.calls[0][0]).toMatchObject({ quantity: 3 });
    expect(onAddLine.mock.calls[0][0].rate).toBeCloseTo(1000 / 3, 10);
  });

  it('editing the rate after typing a total resets the total and keeps the typed rate', async () => {
    installApi({ searchItemsFts: [pencil], previewLotDraw: noLots, previewSaleLine });
    const onAddLine = vi.fn((_e: { quantity: number; rate: number }) => true);
    render(<RapidBillingRow onAddLine={onAddLine} />);
    await pickPencil();
    setInput(tid('total-input')!, '110');
    await waitFor(() => !!tid('line-new-rate'));

    setInput(tid('rate-input')!, '100');
    await waitFor(() => tid<HTMLInputElement>('total-input')!.value === '100.00');
    expect(tid('line-new-rate')).toBeNull();
    click(tid('add-item-btn')!);
    await waitFor(() => onAddLine.mock.calls.length === 1);
    expect(onAddLine).toHaveBeenCalledWith(expect.objectContaining({ rate: 100 }));
  });
});
