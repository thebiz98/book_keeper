import React from 'react';
import { describe, it, expect, beforeEach, vi } from 'vitest';
import { render } from '../testUtils';
import { installApi, flush, waitFor, click, setInput, buttonByText, bodyText, fakeBillPreview } from '../pages/helpers';
import { BillDetailDrawer } from '@/components/domain/BillDetailDrawer';
import { purchaseDetail, purchaseGuard, search, INK, getItem } from './fixtures';
import type { BillEditGuard } from '@/types';

(globalThis as unknown as { IS_REACT_ACT_ENVIRONMENT: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

vi.mock('@/components/domain/ItemSearchTypeahead', async () => {
  const { INK: ink } = await import('./fixtures');
  return {
    ItemSearchTypeahead: ({ onSelectItem }: { onSelectItem: (i: unknown) => void }) => (
      <button type="button" data-testid="stub-pick" onClick={() => onSelectItem(ink)}>
        pick
      </button>
    ),
  };
});
vi.mock('@/components/domain/PartySelector', () => ({ PartySelector: () => <div data-testid="stub-party" /> }));

const q = (sel: string) => document.querySelector(sel) as HTMLInputElement;
const settle = async () => {
  for (let i = 0; i < 4; i++) await flush(120); // the totals and payment previews come back
};

function mount(opts: { guard?: BillEditGuard; preview?: (p: never) => unknown; strict?: boolean; onChanged?: () => void } = {}) {
  const api = installApi({
    getPurchaseDetails: purchaseDetail(),
    getBillEditGuard: opts.guard ?? purchaseGuard(),
    searchItemsFts: search, getItem,
    previewPurchase: opts.preview ?? (async (p: Parameters<typeof fakeBillPreview>[0]) => fakeBillPreview(p)),
    updatePurchase: { purchaseId: 8, transactionCode: 'PUR-1008' },
    getDashboardMetrics: { cashInHand: 0 },
  });
  const ui = <BillDetailDrawer kind="PURCHASE" billId={8} onClose={() => {}} onChanged={opts.onChanged} />;
  render(opts.strict ? <React.StrictMode>{ui}</React.StrictMode> : ui);
  return api;
}

async function openEdit() {
  await waitFor(() => !!document.querySelector('[data-testid="bill-edit"]:not([disabled])'));
  click(q('[data-testid="bill-edit"]'));
  await waitFor(() => document.querySelectorAll('[data-testid="edit-line"]').length > 0);
  await settle();
}

describe('BillDetailDrawer purchase edit (C1, DD-57)', () => {
  beforeEach(() => {
    document.body.innerHTML = '';
  });

  it('edits quantity, price and unit in place and saves through updatePurchase', async () => {
    const onChanged = vi.fn();
    const api = mount({ onChanged });
    await openEdit();
    expect(bodyText()).toContain('Editing PUR-1008');
    expect(q('input[aria-label="Quantity Pen"]').value).toBe('10');
    expect(q('[data-testid="edit-total"]').textContent).toContain('200'); // from the preview (fake: qty x cost)
    setInput(q('input[aria-label="Quantity Ink"]'), '8');
    setInput(q('input[aria-label="Purchase Price Ink"]'), '25');
    setInput(document.querySelector('select[aria-label="Unit Pen"]')!, '5');
    await settle();
    expect(q('[data-testid="edit-total"]').textContent).toContain('300'); // 10x10 + 8x25
    click(q('[data-testid="edit-save"]'));
    await flush();
    expect(api.updatePurchase).toHaveBeenCalledTimes(1);
    const [id, payload] = api.updatePurchase.mock.calls[0];
    expect(id).toBe(8);
    expect(payload.items).toEqual([
      { itemId: 1, bundleId: 5, bundleQuantity: 10, unitCost: 10, purchaseItemId: 21 },
      { itemId: 2, bundleId: null, bundleQuantity: 8, unitCost: 25, purchaseItemId: 22 },
    ]);
    expect(payload).toMatchObject({ transactionDate: '2026-11-20', paymentStatus: 'PAID', paidAmount: 300 });
    expect(api.previewPurchase.mock.calls.at(-1)![1]).toBe(8); // the previews carry the purchase id
    expect(onChanged).toHaveBeenCalled();
    expect(document.querySelector('[data-testid="purchase-edit-form"]')).toBeNull(); // back to view mode
    expect(bodyText()).toContain('Bill updated');
  });

  it('adds a line that needs a price, and sends it without a lot id', async () => {
    const api = mount();
    await openEdit();
    click(q('[data-testid="stub-pick"]'));
    await settle();
    expect(document.querySelectorAll('[data-testid="edit-line"]')).toHaveLength(3);
    expect(q('[data-testid="edit-save"]').disabled).toBe(true); // price still empty
    expect(bodyText()).toContain('Enter a Purchase Price');
    setInput(document.querySelectorAll('input[aria-label="Purchase Price Ink"]')[1], '7');
    await settle();
    expect(q('[data-testid="edit-save"]').disabled).toBe(false);
    click(q('[data-testid="edit-save"]'));
    await flush();
    expect(api.updatePurchase.mock.calls[0][1].items[2]).toEqual({
      itemId: INK.itemId, bundleId: null, bundleQuantity: 1, unitCost: 7, purchaseItemId: null,
    });
  });

  it('removes a free line but not a referenced one', async () => {
    const api = mount();
    await openEdit();
    expect((q('button[aria-label="Remove Pen"]') as unknown as HTMLButtonElement).disabled).toBe(true);
    click(q('button[aria-label="Remove Ink"]'));
    await settle();
    expect(document.querySelectorAll('[data-testid="edit-line"]')).toHaveLength(1);
    click(q('[data-testid="edit-save"]'));
    await flush();
    expect(api.updatePurchase.mock.calls[0][1].items).toHaveLength(1);
  });

  it('shows the used hint and makes the price read-only on a vendor-returned lot', async () => {
    mount();
    await openEdit();
    expect(q('[data-testid="used-hint"]').textContent).toContain('Used: 4');
    expect(q('input[aria-label="Purchase Price Pen"]').disabled).toBe(true);
    expect(q('input[aria-label="Purchase Price Ink"]').disabled).toBe(false);
  });

  it('a preview error shows above Save and disables it', async () => {
    const api = mount({
      preview: async (p: Parameters<typeof fakeBillPreview>[0]) =>
        fakeBillPreview(p, { errors: p.items.some((i) => i.bundleQuantity < 4) ? [{ field: 'quantity', message: 'Pen: at least 4 already used' }] : [] }),
    });
    await openEdit();
    setInput(q('input[aria-label="Quantity Pen"]'), '3');
    await settle();
    expect(q('[data-testid="edit-errors"]').textContent).toContain('at least 4 already used');
    expect(q('[data-testid="edit-save"]').disabled).toBe(true);
    click(q('[data-testid="edit-save"]'));
    await flush();
    expect(api.updatePurchase).not.toHaveBeenCalled();
  });

  it('keeps edit mode and shows the message when the save fails', async () => {
    const api = mount();
    api.updatePurchase.mockImplementation(() => Promise.reject({ type: 'BillLocked', details: { code: 'X', message: 'Locked now.' } }));
    await openEdit();
    setInput(q('input[aria-label="Quantity Ink"]'), '9');
    await settle();
    click(q('[data-testid="edit-save"]'));
    await flush();
    expect(q('[data-testid="edit-errors"]').textContent).toContain('Locked now.');
    expect(document.querySelector('[data-testid="purchase-edit-form"]')).not.toBeNull();
  });

  it('cancel without changes goes straight back; with changes it asks first', async () => {
    mount();
    await openEdit();
    click(q('[data-testid="edit-cancel"]'));
    expect(document.querySelector('[data-testid="purchase-edit-form"]')).toBeNull();

    click(q('[data-testid="bill-edit"]'));
    await waitFor(() => document.querySelectorAll('[data-testid="edit-line"]').length > 0);
    await settle();
    setInput(q('input[aria-label="Quantity Ink"]'), '9');
    click(q('[data-testid="edit-cancel"]'));
    expect(bodyText()).toContain('Discard changes?');
    click(buttonByText('Keep editing'));
    expect(document.querySelector('[data-testid="purchase-edit-form"]')).not.toBeNull();
    click(q('[data-testid="edit-cancel"]'));
    click(buttonByText('Discard changes'));
    expect(document.querySelector('[data-testid="purchase-edit-form"]')).toBeNull();
  });

  it('closing the drawer while dirty asks before it closes', async () => {
    mount();
    await openEdit();
    setInput(q('input[aria-label="Quantity Ink"]'), '9');
    click(q('button[aria-label="Close"]'));
    expect(bodyText()).toContain('Discard changes?');
  });

  it('under StrictMode the edit form shows the loaded bill, not an empty one', async () => {
    mount({ strict: true });
    await openEdit();
    expect(document.querySelectorAll('[data-testid="edit-line"]')).toHaveLength(2);
    expect(q('input[aria-label="Quantity Pen"]').value).toBe('10');
  });
});
