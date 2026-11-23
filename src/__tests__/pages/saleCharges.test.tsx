import { describe, it, expect, beforeEach } from 'vitest';
import { act } from 'react';
import { render } from '../testUtils';
import { NewSalePage } from '@/pages/NewSalePage';
import { useCartStore } from '@/store/useCartStore';
import { __resetHotkeys } from '@/hooks/useHotkeys';
import type { ChargeType } from '@/types';
import { installApi, waitFor, click, setInput, bodyText } from './helpers';

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
    getDashboardMetrics: { liveCashInDrawer: 0 },
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
      { id: 1, name: 'Pencil', baseUnit: 'PCS', defaultSalePrice: 100, mrp: null, currentStock: 50 },
      2,
      null,
      100
    );
  });

  it('Alt+X opens the charge row and the footer shows the grand total', async () => {
    makeApi();
    render(<NewSalePage />);
    await addCharge('tr');
    await waitFor(() => !!tid('cart-grand-total'));
    expect(tid('cart-subtotal')!.textContent).toContain('200.00');
    expect(tid('cart-charges-total')!.textContent).toContain('50.00');
    expect(tid('cart-grand-total')!.textContent).toContain('250.00');
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
    expect(tid('grand-total')!.textContent).toContain('230.00');
    expect((tid('cash-received-input') as HTMLInputElement).value).toBe('230');
    click(tid('confirm-btn')!);
    await waitFor(() => api.createSale.mock.calls.length === 1);
    const payload = api.createSale.mock.calls[0][0] as { charges: unknown[]; paidAmount: number };
    expect(payload.charges).toEqual([
      { chargeTypeId: 1, amount: 50 },
      { chargeTypeId: 3, amount: 20 },
    ]);
    expect(payload.paidAmount).toBe(230);
    expect(bodyText()).not.toContain('Discounts exceed');
  });

  it('blocks the payment dialog when discounts exceed the bill', async () => {
    makeApi();
    render(<NewSalePage />);
    await addCharge('cash', '250');
    click(tid('complete-sale-btn')!);
    await waitFor(() => !!tid('page-error'));
    expect(tid('page-error')!.textContent).toContain('Discounts exceed');
    expect(tid('payment-breakdown')).toBeNull();
  });
});
