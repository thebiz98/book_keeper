import { describe, it, expect, beforeEach, vi } from 'vitest';
import { render } from '../testUtils';
import { SetStockDialog } from '@/components/domain/SetStockDialog';
import { todayLocal } from '@/lib/dates';
import { installApi, flush, click, setInput, buttonByText, bodyText } from '../pages/helpers';
import type { ItemLot } from '@/types';

const lot = (id: number, date: string, cost: number, remaining: number): ItemLot => ({
  purchaseItemId: id, purchaseTransactionId: id, transactionCode: `PUR-${id}`, source: 'PURCHASE',
  transactionDate: date, vendorName: null, quantity: remaining, remainingQuantity: remaining, unitCost: cost,
});

const ITEM = { itemId: 9, name: 'Gel Pen', currentStock: 10, baseUnit: 'PCS' };
const RESULT = {
  adjustmentId: 1, delta: 5, reason: 'UNRECORDED_STOCK', previousStock: 10, newStock: 15, unitCost: 6, valueChange: 30,
};

const open = async (lots: ItemLot[], extra: Record<string, unknown> = {}, item = ITEM) => {
  const onClose = vi.fn();
  const onDone = vi.fn();
  const api = installApi({ getItemLots: lots, setItemStock: RESULT, createStockAdjustment: { adjustmentId: 2, lossValue: 0 }, ...extra });
  render(<SetStockDialog isOpen item={item} onClose={onClose} onDone={onDone} />);
  await flush();
  return { api, onClose, onDone };
};

const nums = () => document.querySelectorAll('input[type="number"]');
const preview = () => document.querySelector('[data-testid="set-stock-preview"]')!.textContent ?? '';

describe('SetStockDialog', () => {
  beforeEach(() => {
    document.body.innerHTML = '';
  });

  it('prefills the unit cost from the lot with the highest purchaseItemId', async () => {
    await open([lot(2, '2026-10-01', 6, 5), lot(1, '2026-11-05', 5, 5)]);
    setInput(nums()[0], '15');
    await flush();
    expect((nums()[1] as HTMLInputElement).value).toBe('6');
  });

  it('set up: previews the value, posts setItemStock and shows the result', async () => {
    const { api, onDone } = await open([lot(2, '2026-10-01', 6, 5), lot(1, '2026-11-05', 5, 5)]);
    setInput(nums()[0], '15');
    await flush();
    expect(preview()).toContain('+5');
    expect(preview()).toContain('₹30.00');
    click(buttonByText('Save stock'));
    await flush();
    expect(api.setItemStock.mock.calls[0][0]).toEqual({
      itemId: 9, countedQuantity: 15, unitCost: 6, adjustmentDate: todayLocal(), notes: null,
    });
    expect(document.querySelector('[data-testid="set-stock-result"]')!.textContent).toContain('15');
    expect(onDone).toHaveBeenCalledTimes(1);
  });

  it('requires a unit cost when there are no lots', async () => {
    const { api } = await open([]);
    setInput(nums()[0], '15');
    await flush();
    expect(bodyText()).toContain('required (no lots yet)');
    click(buttonByText('Save stock'));
    await flush();
    expect(bodyText()).toContain('Unit cost is required');
    expect(api.setItemStock).not.toHaveBeenCalled();
  });

  it('set down: previews a FIFO count correction and sends no unit cost', async () => {
    const { api } = await open([lot(1, '2026-10-01', 5, 4), lot(2, '2026-10-02', 7, 10)], {}, { ...ITEM, currentStock: 14 });
    setInput(nums()[0], '8');
    await flush();
    expect(preview()).toContain('Count correction');
    expect(preview()).toContain('₹34.00');
    click(buttonByText('Save stock'));
    await flush();
    expect(api.setItemStock.mock.calls[0][0]).toMatchObject({ countedQuantity: 8, unitCost: null });
  });

  it('equal count shows "No change" and disables Save', async () => {
    const { api } = await open([lot(1, '2026-10-01', 5, 10)]);
    expect(preview()).toContain('No change');
    const save = buttonByText('Save stock');
    expect(save.disabled).toBe(true);
    click(save);
    await flush();
    expect(api.setItemStock).not.toHaveBeenCalled();
  });

  it('add mode posts an UNRECORDED_STOCK adjustment', async () => {
    const { api } = await open([lot(2, '2026-10-01', 6, 5), lot(1, '2026-11-05', 5, 5)]);
    setInput(document.querySelector('select')!, 'ADD');
    await flush();
    setInput(nums()[0], '3');
    await flush();
    click(buttonByText('Save stock'));
    await flush();
    expect(api.createStockAdjustment.mock.calls[0][0]).toMatchObject({
      quantity: 3, reason: 'UNRECORDED_STOCK', unitCost: 6, itemId: 9,
    });
    expect(api.setItemStock).not.toHaveBeenCalled();
    expect(document.querySelector('[data-testid="set-stock-result"]')!.textContent).toContain('13');
  });

  it('Cancel closes without any write', async () => {
    const { api, onClose } = await open([lot(1, '2026-10-01', 5, 10)]);
    click(buttonByText('Cancel'));
    expect(onClose).toHaveBeenCalledTimes(1);
    expect(api.setItemStock).not.toHaveBeenCalled();
    expect(api.createStockAdjustment).not.toHaveBeenCalled();
  });
});
