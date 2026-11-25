import React, { act } from 'react';
import { describe, it, expect } from 'vitest';
import { render } from '../testUtils';
import { installApi, flush, waitFor } from '../pages/helpers';
import { usePurchaseEditDraft, type PurchaseEditDraftApi } from '@/components/domain/billEdit/usePurchaseEditDraft';
import type { BillEditGuard } from '@/types';
import { purchaseDetail, purchaseGuard, INK, search } from './fixtures';

(globalThis as unknown as { IS_REACT_ACT_ENVIRONMENT: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

function mount(strict = false, g: BillEditGuard = purchaseGuard()) {
  installApi({ getPurchaseDetails: purchaseDetail(), getBillEditGuard: g, searchItemsFts: search });
  const out: { current: PurchaseEditDraftApi } = { current: undefined as unknown as PurchaseEditDraftApi };
  const Probe: React.FC = () => {
    out.current = usePurchaseEditDraft(8);
    return null;
  };
  render(strict ? <React.StrictMode><Probe /></React.StrictMode> : <Probe />);
  return out;
}

describe('usePurchaseEditDraft', () => {
  it('maps the stored purchase and the guard flags onto the lines', async () => {
    const out = mount();
    await waitFor(() => out.current.load.status === 'ready');
    const d = out.current.draft!;
    expect(d.code).toBe('PUR-1008');
    expect(d.supplierInvoiceRef).toBe('INV-1');
    expect(d.lines.map((l) => [l.itemName, l.quantity, l.unitCost, l.purchaseItemId])).toEqual([
      ['Pen', 10, 10, 21],
      ['Ink', 5, 20, 22],
    ]);
    expect(d.lines[0]).toMatchObject({ minBaseQty: 4, locked: true, costLocked: true });
    expect(d.lines[1]).toMatchObject({ minBaseQty: 0, locked: false, costLocked: false });
    expect(d.lines[0].bundles).toHaveLength(1);
    // paid in full upfront -> seeded as PAID; the backend preview judges it
    expect(d.paymentStatus).toBe('PAID');
    expect(out.current.dirty).toBe(false);
  });

  it('seeds PARTIAL with the stored upfront amount when the bill is not paid', async () => {
    const detail = purchaseDetail();
    detail.purchase.paymentStatus = 'PARTIAL';
    installApi({ getPurchaseDetails: detail, getBillEditGuard: purchaseGuard({ upfrontAmount: 50 }), searchItemsFts: search });
    const out: { current: PurchaseEditDraftApi } = { current: undefined as unknown as PurchaseEditDraftApi };
    const Probe: React.FC = () => {
      out.current = usePurchaseEditDraft(8);
      return null;
    };
    render(<Probe />);
    await waitFor(() => out.current.load.status === 'ready');
    expect(out.current.draft).toMatchObject({ paymentStatus: 'PARTIAL', paidInput: 50 });
  });

  it('surfaces the guard reason when the bill cannot be edited', async () => {
    const out = mount(false, purchaseGuard({ canEdit: false, reasons: [{ code: 'LINKED_RETURNS', message: 'Has returns.', blocksEdit: true, blocksDelete: true }] }));
    await waitFor(() => out.current.load.status === 'error');
    expect(out.current.load).toEqual({ status: 'error', message: 'Has returns.' });
  });

  it('keeps purchaseItemId in the payload; a new line has none and no price until typed', async () => {
    const out = mount();
    await waitFor(() => out.current.load.status === 'ready');
    expect(out.current.toPayload(200)!.items).toEqual([
      { itemId: 1, bundleId: null, bundleQuantity: 10, unitCost: 10, purchaseItemId: 21 },
      { itemId: 2, bundleId: null, bundleQuantity: 5, unitCost: 20, purchaseItemId: 22 },
    ]);
    let key = 0;
    act(() => { key = out.current.addLine(INK); });
    expect(out.current.draft!.lines).toHaveLength(3);
    expect(out.current.incomplete).toBe(true); // price still empty: nothing to preview
    expect(out.current.bill).toBeNull();
    act(() => out.current.setLine(key, { unitCost: 7 }));
    expect(out.current.incomplete).toBe(false);
    expect(out.current.toPayload(0)!.items[2]).toEqual({ itemId: 2, bundleId: null, bundleQuantity: 1, unitCost: 7, purchaseItemId: null });
    expect(out.current.dirty).toBe(true);
  });

  it('lets a free lot be removed but never a referenced one', async () => {
    const out = mount();
    await waitFor(() => out.current.load.status === 'ready');
    const [pen, ink] = out.current.draft!.lines;
    act(() => out.current.removeLine(pen.key));
    expect(out.current.draft!.lines).toHaveLength(2);
    act(() => out.current.removeLine(ink.key));
    expect(out.current.draft!.lines.map((l) => l.itemName)).toEqual(['Pen']);
    expect(out.current.dirty).toBe(true);
  });

  it('switching the unit changes the pack and multiplier and keeps what was typed', async () => {
    const out = mount();
    await waitFor(() => out.current.load.status === 'ready');
    const pen = out.current.draft!.lines[0];
    act(() => out.current.setUnit(pen.key, 5));
    expect(out.current.draft!.lines[0]).toMatchObject({ bundleId: 5, packLabel: 'Box', unitsPerBundle: 10, quantity: 10, unitCost: 10 });
    expect(out.current.toPayload(0)!.items[0]).toMatchObject({ bundleId: 5, bundleQuantity: 10, unitCost: 10 });
    act(() => out.current.setUnit(pen.key, null));
    expect(out.current.draft!.lines[0]).toMatchObject({ bundleId: null, unitsPerBundle: 1 });
    expect(out.current.dirty).toBe(false); // back to the stored values
  });

  it('survives a StrictMode double mount: the bill is loaded, not empty', async () => {
    const out = mount(true);
    await waitFor(() => out.current.load.status === 'ready');
    await flush(20);
    expect(out.current.load.status).toBe('ready');
    expect(out.current.draft!.lines).toHaveLength(2);
    expect(out.current.draft!.code).toBe('PUR-1008');
  });
});
