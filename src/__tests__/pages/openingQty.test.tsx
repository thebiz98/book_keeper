import { describe, it, expect, beforeEach, vi } from 'vitest';
import { render } from '../testUtils';
import { OpeningQtyEditModal } from '@/components/domain/OpeningQtyEditModal';
import type { ItemLot, OpeningQuantityResult } from '@/types';
import { installApi, flush, waitFor, click, setInput, buttonByText, bodyText } from './helpers';

const lot = (over: Partial<ItemLot> = {}): ItemLot => ({
  purchaseItemId: 21, purchaseTransactionId: 3, transactionCode: 'OPN-0001', source: 'OPENING',
  transactionDate: '2026-11-01', vendorName: '', packLabel: null, packQuantity: 6, quantity: 6,
  remainingQuantity: 5, unitCost: 10, ...over,
});

const result = (q: number): OpeningQuantityResult => ({
  purchaseItemId: 21, itemId: 1, oldQuantity: 6, newQuantity: q, consumed: 1, unitCost: 10, oldStock: 5,
  newStock: q - 1, valueChange: (q - 6) * 10, lotRemoved: false,
});

const qtyInput = () => document.querySelector('input[aria-label="Opening quantity"]') as HTMLInputElement;

describe('OpeningQtyEditModal', () => {
  beforeEach(() => {
    document.body.innerHTML = '';
  });

  it('shows used and minimum, previews the new stock and saves', async () => {
    const api = installApi({
      previewOpeningLotQuantity: async (p: { quantity: number }) => result(p.quantity),
      updateOpeningLotQuantity: result(1),
    });
    const onSaved = vi.fn();
    render(<OpeningQtyEditModal lot={lot()} baseUnit="PCS" onClose={() => {}} onSaved={onSaved} />);
    await waitFor(() => (document.querySelector('[data-testid="opening-min"]')?.textContent ?? '').includes('1 PCS'));
    expect(bodyText()).toContain('Not recorded as a loss or adjustment');
    expect(buttonByText('Save').disabled).toBe(true); // unchanged
    setInput(qtyInput(), 1);
    await waitFor(() => (document.querySelector('[data-testid="opening-preview"]')?.textContent ?? '').includes('-'));
    expect(api.previewOpeningLotQuantity).toHaveBeenLastCalledWith({ purchaseItemId: 21, quantity: 1 });
    expect(document.querySelector('[data-testid="opening-preview"]')?.textContent).toContain('5');
    await waitFor(() => !buttonByText('Save').disabled);
    click(buttonByText('Save'));
    await flush();
    expect(api.updateOpeningLotQuantity).toHaveBeenCalledWith({ purchaseItemId: 21, quantity: 1 });
    expect(onSaved).toHaveBeenCalled();
  });

  it('shows the backend refusal and blocks Save', async () => {
    installApi({
      previewOpeningLotQuantity: async (p: { quantity: number }) => {
        if (p.quantity < 1) {
          throw { type: 'BillLocked', details: { code: 'LOT_BELOW_CONSUMED', message: "'Pen': at least 1 already used, quantity cannot go below that" } };
        }
        return result(p.quantity);
      },
    });
    render(<OpeningQtyEditModal lot={lot()} baseUnit="PCS" onClose={() => {}} onSaved={() => {}} />);
    await flush(150);
    setInput(qtyInput(), 0);
    await waitFor(() => bodyText().includes('at least 1 already used'));
    expect(buttonByText('Save').disabled).toBe(true);
  });

  it('shows a save error from the backend', async () => {
    installApi({
      previewOpeningLotQuantity: async (p: { quantity: number }) => result(p.quantity),
      updateOpeningLotQuantity: async () => {
        throw { type: 'BillLocked', details: { code: 'LOT_BELOW_CONSUMED', message: 'Used since you opened this' } };
      },
    });
    const onSaved = vi.fn();
    render(<OpeningQtyEditModal lot={lot()} baseUnit="PCS" onClose={() => {}} onSaved={onSaved} />);
    await flush(150);
    setInput(qtyInput(), 3);
    await waitFor(() => !buttonByText('Save').disabled);
    click(buttonByText('Save'));
    await flush();
    expect(bodyText()).toContain('Used since you opened this');
    expect(onSaved).not.toHaveBeenCalled();
  });
});
