import React from 'react';
import { describe, it, expect, beforeEach, vi } from 'vitest';
import { render } from '../testUtils';
import { installApi, flush, waitFor, click, setInput, buttonByText, bodyText, fakeBillPreview } from '../pages/helpers';
import { BillDetailDrawer } from '@/components/domain/BillDetailDrawer';
import { useCartStore } from '@/store/useCartStore';
import { saleDetail, saleGuard, search, INK, PEN } from './fixtures';
import type { BillEditGuard } from '@/types';

(globalThis as unknown as { IS_REACT_ACT_ENVIRONMENT: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

vi.mock('@/components/domain/ItemSearchTypeahead', async () => {
  const { INK: ink } = await import('./fixtures');
  return {
    ItemSearchTypeahead: ({ onSelectItem }: { onSelectItem: (i: unknown) => void }) => (
      <button type="button" data-testid="stub-pick" onClick={() => onSelectItem({ ...ink, defaultSalePrice: 11 })}>
        pick
      </button>
    ),
  };
});
vi.mock('@/components/domain/PartySelector', () => ({ PartySelector: () => <div data-testid="stub-party" /> }));

const q = (sel: string) => document.querySelector(sel) as HTMLInputElement;
const settle = async () => {
  for (let i = 0; i < 4; i++) await flush(120);
};

function mount(opts: { guard?: BillEditGuard; preview?: (p: never) => unknown; strict?: boolean; onChanged?: () => void } = {}) {
  const api = installApi({
    getSaleDetails: saleDetail(),
    getBillEditGuard: opts.guard ?? saleGuard(),
    searchItemsFts: search,
    previewSale: opts.preview ?? (async (p: Parameters<typeof fakeBillPreview>[0]) => fakeBillPreview(p)),
    updateSale: { saleId: 5, transactionCode: 'SAL-1005', totalAmount: 0, paidAmount: 0, balanceDue: 0, changeAmount: 0, paymentStatus: 'PAID' },
    getDashboardMetrics: { cashInHand: 0 },
  });
  const ui = <BillDetailDrawer kind="SALE" billId={5} onClose={() => {}} onChanged={opts.onChanged} />;
  render(opts.strict ? <React.StrictMode>{ui}</React.StrictMode> : ui);
  return api;
}

async function openEdit() {
  await waitFor(() => !!document.querySelector('[data-testid="bill-edit"]:not([disabled])'));
  click(q('[data-testid="bill-edit"]'));
  await waitFor(() => document.querySelectorAll('[data-testid="edit-line"]').length > 0);
  await settle();
}

describe('BillDetailDrawer sale edit (C1, DD-57)', () => {
  beforeEach(() => {
    document.body.innerHTML = '';
    useCartStore.getState().clearCart();
  });

  it('edits quantity, rate and unit in place and saves through updateSale', async () => {
    const onChanged = vi.fn();
    const api = mount({ onChanged });
    await openEdit();
    expect(bodyText()).toContain('Editing SAL-1005');
    expect(q('input[aria-label="Quantity Pen"]').value).toBe('2');
    expect(q('input[aria-label="Rate Pen"]').value).toBe('50');
    expect(q('[data-testid="edit-total"]').textContent).toContain('130'); // from the preview
    setInput(q('input[aria-label="Quantity Ink"]'), '3');
    setInput(q('input[aria-label="Rate Ink"]'), '40');
    setInput(document.querySelector('select[aria-label="Unit Pen"]')!, '5');
    await settle();
    // unit change takes the backend-suggested rate of the Box; total is the preview's
    expect(q('input[aria-label="Rate Pen"]').value).toBe(String(PEN.bundles[0].defaultRate));
    click(q('[data-testid="edit-save"]'));
    await flush();
    expect(api.updateSale).toHaveBeenCalledTimes(1);
    const [id, payload] = api.updateSale.mock.calls[0];
    expect(id).toBe(5);
    expect(payload.items).toEqual([
      { itemId: 1, bundleId: 5, bundleQuantity: 2, unitPrice: PEN.bundles[0].defaultRate },
      { itemId: 2, bundleId: null, bundleQuantity: 3, unitPrice: 40 },
    ]);
    expect(payload).toMatchObject({ transactionDate: '2026-11-20', paymentStatus: 'PAID', notes: 'hi' });
    expect(api.previewSale.mock.calls.at(-1)![1]).toBe(5);
    expect(onChanged).toHaveBeenCalled();
    expect(document.querySelector('[data-testid="sale-edit-form"]')).toBeNull();
    expect(bodyText()).toContain('Bill updated');
  });

  it('adds a line at the item suggested rate and removes any line', async () => {
    const api = mount();
    await openEdit();
    click(q('[data-testid="stub-pick"]'));
    await settle();
    expect(document.querySelectorAll('[data-testid="edit-line"]')).toHaveLength(3);
    expect(document.querySelectorAll('input[aria-label="Rate Ink"]')[1]).toHaveProperty('value', '11');
    click(q('button[aria-label="Remove Pen"]'));
    await settle();
    expect(document.querySelectorAll('[data-testid="edit-line"]')).toHaveLength(2);
    click(q('[data-testid="edit-save"]'));
    await flush();
    expect(api.updateSale.mock.calls[0][1].items).toEqual([
      { itemId: 2, bundleId: null, bundleQuantity: 1, unitPrice: 30 },
      { itemId: INK.itemId, bundleId: null, bundleQuantity: 1, unitPrice: 11 },
    ]);
  });

  it('a stock error from the preview shows above Save and disables it', async () => {
    const api = mount({
      preview: async (p: Parameters<typeof fakeBillPreview>[0]) =>
        fakeBillPreview(p, { errors: p.items.some((i) => i.bundleQuantity > 5) ? [{ field: 'quantity', message: 'Only 5 Pen in stock' }] : [] }),
    });
    await openEdit();
    setInput(q('input[aria-label="Quantity Pen"]'), '9');
    await settle();
    expect(q('[data-testid="edit-errors"]').textContent).toContain('Only 5 Pen in stock');
    expect(q('[data-testid="edit-save"]').disabled).toBe(true);
    click(q('[data-testid="edit-save"]'));
    await flush();
    expect(api.updateSale).not.toHaveBeenCalled();
  });

  it('cancel with changes asks first; the sale cart in progress is never touched', async () => {
    useCartStore.getState().addItem({ id: 9, name: 'Other', baseUnit: 'PCS', defaultSalePrice: 5 });
    mount();
    await openEdit();
    setInput(q('input[aria-label="Quantity Pen"]'), '3');
    click(q('[data-testid="edit-cancel"]'));
    expect(bodyText()).toContain('Discard changes?');
    click(buttonByText('Discard changes'));
    expect(document.querySelector('[data-testid="sale-edit-form"]')).toBeNull();
    expect(useCartStore.getState().items.map((i) => i.itemId)).toEqual([9]);
  });

  it('under StrictMode the edit form shows the loaded bill, not an empty one', async () => {
    mount({ strict: true });
    await openEdit();
    expect(document.querySelectorAll('[data-testid="edit-line"]')).toHaveLength(2);
    expect(q('input[aria-label="Quantity Pen"]').value).toBe('2');
  });
});
