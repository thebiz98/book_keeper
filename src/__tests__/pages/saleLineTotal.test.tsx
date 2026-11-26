import { describe, it, expect, vi, beforeEach } from 'vitest';
import { act } from 'react';
import { render } from '../testUtils';
import { NewSalePage } from '@/pages/NewSalePage';
import { RapidBillingRow } from '@/components/domain/RapidBillingRow';
import { useCartStore } from '@/store/useCartStore';
import { __resetHotkeys } from '@/hooks/useHotkeys';
import type { BillPreview, PreviewSaleLinePayload } from '@/types';
import { installApi, waitFor, click, setInput, fakeBillPreview } from './helpers';
import { makeItem } from '../inventory/testHelpers';

// FR-47: the entry row's Total box. Every figure is the backend's (DD-56); these doubles stand in for
// `preview_sale_line` and `preview_sale`.
const pencil = makeItem({ itemId: 1, name: 'Pencil', currentStock: 50, defaultSalePrice: 102.3, bundles: [] });

const r2 = (n: number) => Math.round(n * 100) / 100;
const previewSaleLine = async (p: PreviewSaleLinePayload) => {
  const lineTotal = r2(p.bundleQuantity * p.unitPrice);
  return { lineTotal, adjustment: p.enteredTotal == null ? 0 : r2(p.enteredTotal - lineTotal) };
};

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

describe('RapidBillingRow Total box (FR-47)', () => {
  beforeEach(() => {
    document.body.innerHTML = '';
    __resetHotkeys();
  });

  it('is prefilled from the backend, follows qty until typed in, and Enter in Rate moves to it', async () => {
    const api = installApi({ searchItemsFts: [pencil], previewLotDraw: { lots: [], drawCost: 0, available: 50, shortfall: 0 }, previewSaleLine });
    render(<RapidBillingRow />);
    await pickPencil();
    await waitFor(() => tid<HTMLInputElement>('total-input')!.value === '102.30');
    setInput(tid('quantity-input')!, '10');
    await waitFor(() => tid<HTMLInputElement>('total-input')!.value === '1023.00');
    expect(api.previewSaleLine).toHaveBeenCalledWith(expect.objectContaining({ bundleQuantity: 10, unitPrice: 102.3 }));

    key(tid('rate-input')!, 'Enter');
    expect(document.activeElement).toBe(tid('total-input'));
  });

  it('shows the discount of a lower total and adds the line with the entered total', async () => {
    installApi({ searchItemsFts: [pencil], previewLotDraw: { lots: [], drawCost: 0, available: 50, shortfall: 0 }, previewSaleLine });
    const onAddLine = vi.fn(() => true);
    render(<RapidBillingRow onAddLine={onAddLine} />);
    await pickPencil();
    setInput(tid('quantity-input')!, '10');
    await waitFor(() => tid<HTMLInputElement>('total-input')!.value === '1023.00');
    setInput(tid('total-input')!, '1000');
    await waitFor(() => !!tid('line-adjustment'));
    expect(tid('line-adjustment')!.textContent).toContain('Discount');
    expect(tid('line-adjustment')!.textContent).toContain('23.00');
    expect(tid('line-adjustment')!.className).toContain('rose');

    key(tid('total-input')!, 'Enter');
    expect(onAddLine).toHaveBeenCalledWith(expect.objectContaining({ quantity: 10, rate: 102.3, enteredTotal: 1000 }));
  });

  it('shows an extra charge for a higher total; editing the rate afterwards resets the total', async () => {
    installApi({ searchItemsFts: [pencil], previewLotDraw: { lots: [], drawCost: 0, available: 50, shortfall: 0 }, previewSaleLine });
    const onAddLine = vi.fn(() => true);
    render(<RapidBillingRow onAddLine={onAddLine} />);
    await pickPencil();
    setInput(tid('total-input')!, '110');
    await waitFor(() => tid('line-adjustment')?.textContent?.includes('Extra charge') ?? false);
    expect(tid('line-adjustment')!.textContent).toContain('7.70');

    setInput(tid('rate-input')!, '100');
    await waitFor(() => tid<HTMLInputElement>('total-input')!.value === '100.00');
    expect(tid('line-adjustment')).toBeNull();
    click(tid('add-item-btn')!);
    expect(onAddLine).toHaveBeenCalledWith(expect.objectContaining({ rate: 100, enteredTotal: null }));
  });
});

describe('Cart store keeps the entered total (FR-47)', () => {
  const s = () => useCartStore.getState();
  const item = { id: 1, name: 'Pencil', baseUnit: 'PCS', defaultSalePrice: 10, mrp: null };
  beforeEach(() => s().clearCart());

  it('never merges a line with an entered total, and drops it when qty or rate change', () => {
    s().addItem(item, 2, null, 10, 18);
    s().addItem(item, 2, null, 10, null);
    s().addItem(item, 1, null, 10, null);
    expect(s().items.map((l) => [l.quantity, l.enteredTotal ?? null])).toEqual([
      [2, 18],
      [3, null],
    ]);
    const id = s().items[0].lineId;
    s().updateQuantity(id, 2);
    expect(s().items[0].enteredTotal).toBe(18);
    s().updateQuantity(id, 3);
    expect(s().items[0].enteredTotal).toBeNull();

    s().addItem(item, 1, null, 12, 15);
    const id2 = s().items[2].lineId;
    s().updateRate(id2, 13);
    expect(s().items[2].enteredTotal).toBeNull();
  });

  it('rejects a negative entered total', () => {
    expect(s().addItem(item, 1, null, 10, -1)).toBe(false);
    expect(s().items).toHaveLength(0);
  });
});

describe('NewSalePage shows item adjustments at the bottom (FR-47)', () => {
  beforeEach(() => {
    document.body.innerHTML = '';
    __resetHotkeys();
    useCartStore.getState().clearCart();
    useCartStore.getState().addItem({ id: 1, name: 'Pencil', baseUnit: 'PCS', defaultSalePrice: 102.3, mrp: null }, 10, null, 102.3, 1000);
  });

  it('sends the entered total and shows the tag, the discount row and the payment breakdown', async () => {
    const api = installApi({
      getNextSaleCode: 'SAL-0002',
      searchItemsFts: [],
      listParties: [],
      searchParties: [],
      getDashboardMetrics: { cashInHand: 0 },
      previewSale: async (p: Parameters<typeof fakeBillPreview>[0]): Promise<BillPreview> => {
        const base = fakeBillPreview(p);
        return {
          ...base,
          discountsTotal: 23,
          totalAmount: 1000,
          lineAdjustments: [-23],
          adjustmentLines: [{ name: 'Item discount', effect: 'DISCOUNT', amount: 23 }],
        };
      },
    });
    render(<NewSalePage />);
    await waitFor(() => !!tid('line-adjustment-tag'));
    expect(tid('line-adjustment-tag')!.textContent).toContain('23.00');
    expect(api.previewSale.mock.calls[0][0].items[0]).toMatchObject({ bundleQuantity: 10, unitPrice: 102.3, enteredTotal: 1000 });
    expect(tid('cart-discounts-total')!.textContent).toContain('23.00');
    expect(tid('cart-grand-total')!.textContent).toContain('1,000.00');

    click(tid('complete-sale-btn')!);
    await waitFor(() => !!tid('payment-breakdown'));
    expect(tid('payment-breakdown')!.textContent).toContain('Item discount');
  });
});
