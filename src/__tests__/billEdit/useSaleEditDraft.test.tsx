import React, { act } from 'react';
import { describe, it, expect } from 'vitest';
import { render } from '../testUtils';
import { installApi, waitFor } from '../pages/helpers';
import { useSaleEditDraft, type SaleEditDraftApi } from '@/components/domain/billEdit/useSaleEditDraft';
import { useCartStore } from '@/store/useCartStore';
import { saleDetail, saleGuard, search, INK, PEN, getItem } from './fixtures';

(globalThis as unknown as { IS_REACT_ACT_ENVIRONMENT: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

function mount(strict = false) {
  installApi({ getSaleDetails: saleDetail(), getBillEditGuard: saleGuard(), searchItemsFts: search, getItem });
  const out: { current: SaleEditDraftApi } = { current: undefined as unknown as SaleEditDraftApi };
  const Probe: React.FC = () => {
    out.current = useSaleEditDraft(5);
    return null;
  };
  render(strict ? <React.StrictMode><Probe /></React.StrictMode> : <Probe />);
  return out;
}

describe('useSaleEditDraft', () => {
  it('loads the stored sale into a local draft and leaves the cart alone', async () => {
    useCartStore.getState().clearCart();
    useCartStore.getState().addItem({ id: 9, name: 'Other', baseUnit: 'PCS', defaultSalePrice: 5 });
    const out = mount();
    await waitFor(() => out.current.load.status === 'ready');
    const d = out.current.draft!;
    expect(d.lines.map((l) => [l.name, l.quantity, l.rate])).toEqual([['Pen', 2, 50], ['Ink', 1, 30]]);
    expect(d.paymentStatus).toBe('PAID');
    expect(out.current.dirty).toBe(false);
    expect(useCartStore.getState().items).toHaveLength(1); // a sale in progress is untouched
    useCartStore.getState().clearCart();
  });

  it('sends bundle quantity and rate per unit; a new line takes the suggested rate', async () => {
    const out = mount();
    await waitFor(() => out.current.load.status === 'ready');
    expect(out.current.toPayload(130)!.items).toEqual([
      { itemId: 1, bundleId: null, bundleQuantity: 2, unitPrice: 50, saleItemId: 1 },
      { itemId: 2, bundleId: null, bundleQuantity: 1, unitPrice: 30, saleItemId: 2 },
    ]);
    act(() => { out.current.addLine({ ...INK, defaultSalePrice: 11 }); });
    expect(out.current.draft!.lines[2]).toMatchObject({ itemId: 2, quantity: 1, rate: 11 });
    expect(out.current.dirty).toBe(true);
  });

  it('switching the unit takes the backend suggested rate of that unit', async () => {
    const out = mount();
    await waitFor(() => out.current.load.status === 'ready');
    const pen = out.current.draft!.lines[0];
    act(() => out.current.setUnit(pen.key, 5));
    expect(out.current.draft!.lines[0]).toMatchObject({ bundleId: 5, unitsPerBundle: 10, rate: PEN.bundles[0].defaultRate });
    act(() => out.current.setUnit(pen.key, null));
    expect(out.current.draft!.lines[0]).toMatchObject({ bundleId: null, rate: PEN.defaultSalePrice });
  });

  it('is incomplete while a rate is empty or there are no lines, and removes any line', async () => {
    const out = mount(true); // StrictMode double mount still loads
    await waitFor(() => out.current.load.status === 'ready');
    const [pen, ink] = out.current.draft!.lines;
    act(() => out.current.setLine(pen.key, { rate: null }));
    expect(out.current.incomplete).toBe(true);
    expect(out.current.bill).toBeNull();
    act(() => out.current.removeLine(pen.key));
    act(() => out.current.removeLine(ink.key));
    expect(out.current.draft!.lines).toHaveLength(0);
    expect(out.current.incomplete).toBe(true);
  });
});
