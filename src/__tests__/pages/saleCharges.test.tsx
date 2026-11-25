import { describe, it, expect, beforeEach } from 'vitest';
import { act } from 'react';
import { render } from '../testUtils';
import { NewSalePage } from '@/pages/NewSalePage';
import { useCartStore } from '@/store/useCartStore';
import { __resetHotkeys } from '@/hooks/useHotkeys';
import type { ChargeType } from '@/types';
import { installApi, waitFor, click, setInput, bodyText, fakeBillPreview } from './helpers';

const TRANSPORT: ChargeType = {
  id: 1, name: 'Transport', effect: 'CHARGE', appliesTo: 'BOTH', defaultAmount: 50, isActive: true, createdAt: '',
};
const DISCOUNT: ChargeType = {
  id: 3, name: 'Cash Discount', effect: 'DISCOUNT', appliesTo: 'BOTH', defaultAmount: null, isActive: true, createdAt: '',
};

const q = (sel: string) => document.querySelector<HTMLElement>(sel);
const tid = (id: string) => q(`[data-testid="${id}"]`);
const key = (el: Element | Window, k: string, init: KeyboardEventInit = {}) =>
  act(() => {
    el.dispatchEvent(new KeyboardEvent('keydown', { key: k, bubbles: true, cancelable: true, ...init }));
  });

function makeApi() {
  return installApi({
    getNextSaleCode: 'SAL-0002',
    searchChargeTypes: async (query: string) =>
      [TRANSPORT, DISCOUNT].filter((t) => t.name.toLowerCase().startsWith(query.toLowerCase())),
    searchItemsFts: [],
    listParties: [],
    searchParties: [],
    getDashboardMetrics: { cashInHand: 0 },
    // DD-56: the screen shows what the backend preview returns (a test double stands in for it).
    previewSale: async (p: Parameters<typeof fakeBillPreview>[0] & { charges: { amount: number }[] }) =>
      p.charges.some((c) => c.amount === 250)
        ? fakeBillPreview(p, { errors: [{ field: 'charges', message: 'Discounts (₹250.00) exceed the bill amount (₹200.00)' }] })
        : fakeBillPreview(p),
    createSale: {
      saleId: 1, transactionCode: 'SAL-0002', totalAmount: 230, paidAmount: 230, balanceDue: 0, changeAmount: 0, paymentStatus: 'PAID',
    },
  });
}

async function addCharge(prefix: string, amount?: string) {
  key(window, 'x', { altKey: true });
  await waitFor(() => document.activeElement === tid('charge-type-input'));
  setInput(tid('charge-type-input')!, prefix);
  await waitFor(() => !!q('[role="option"]'));
  key(tid('charge-type-input')!, 'Enter');
  await waitFor(() => !!tid('charge-amount'));
  if (amount) setInput(tid('charge-amount')!, amount);
  key(tid('charge-amount')!, 'Enter');
}

describe('NewSalePage charges (FR-19)', () => {
  beforeEach(() => {
    document.body.innerHTML = '';
    __resetHotkeys();
    useCartStore.getState().clearCart();
    useCartStore.getState().addItem(
      { id: 1, name: 'Pencil', baseUnit: 'PCS', defaultSalePrice: 100, mrp: null },
      2,
      null,
      100
    );
  });

  it('Alt+X opens the charge row and the footer shows the grand total', async () => {
    makeApi();
    render(<NewSalePage />);
    await addCharge('tr');
    await waitFor(() => !!tid('cart-grand-total') && tid('cart-grand-total')!.textContent!.includes('250.00'));
    expect(tid('cart-subtotal')!.textContent).toContain('200.00');
    expect(tid('cart-charges-total')!.textContent).toContain('50.00');
    await addCharge('cash', '20');
    await waitFor(() => tid('cart-grand-total')!.textContent!.includes('230.00'));
    expect(tid('cart-discounts-total')!.textContent).toContain('20.00');
  });

  it('payment dialog shows the breakdown and the payload carries the charges', async () => {
    const api = makeApi();
    render(<NewSalePage />);
    await addCharge('tr');
    await addCharge('cash', '20');
    click(tid('complete-sale-btn')!);
    await waitFor(() => !!tid('payment-breakdown'));
    expect(tid('payment-breakdown')!.textContent).toContain('Transport');
    await waitFor(() => tid('grand-total')!.textContent!.includes('230.00'));
    await waitFor(() => (tid('cash-received-input') as HTMLInputElement).value === '230');
    // The confirm button waits for the backend preview of exactly what is on screen.
    await waitFor(() => !(tid('confirm-btn') as HTMLButtonElement).disabled);
    expect(api.previewSale.mock.calls.at(-1)![0]).toMatchObject({ paymentStatus: 'PAID', paidAmount: 230 });
    click(tid('confirm-btn')!);
    await waitFor(() => api.createSale.mock.calls.length === 1);
    const payload = api.createSale.mock.calls[0][0] as { charges: unknown[]; paidAmount: number };
    expect(payload.charges).toEqual([
      { chargeTypeId: 1, amount: 50 },
      { chargeTypeId: 3, amount: 20 },
    ]);
    expect(payload.paidAmount).toBe(230);
    expect(bodyText()).not.toContain('exceed the bill amount');
  });

  it('blocks the payment dialog when discounts exceed the bill', async () => {
    makeApi();
    render(<NewSalePage />);
    await addCharge('cash', '250');
    await waitFor(() => !!tid('preview-errors'));
    click(tid('complete-sale-btn')!);
    await waitFor(() => !!tid('page-error'));
    expect(tid('page-error')!.textContent).toContain('exceed the bill amount');
    expect(tid('payment-breakdown')).toBeNull();
  });
});
