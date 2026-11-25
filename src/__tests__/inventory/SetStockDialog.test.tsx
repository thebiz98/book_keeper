(globalThis as unknown as { IS_REACT_ACT_ENVIRONMENT: boolean }).IS_REACT_ACT_ENVIRONMENT = true;
import { describe, it, expect, beforeEach, vi } from 'vitest';
import { render } from '../testUtils';
import { SetStockDialog } from '@/components/domain/SetStockDialog';
import { todayLocal } from '@/lib/dates';
import { installApi, flush, click, setInput, buttonByText, bodyText } from '../pages/helpers';
import type { SetItemStockPayload, SetItemStockResult } from '@/types';

// DD-56: the delta, the default cost (latest lot), the FIFO loss and the value change are the backend's
// preview. These tests canned its answers and check what the dialog sends and shows.

const ITEM = { itemId: 9, name: 'Gel Pen', currentStock: 10, baseUnit: 'PCS' };
const RESULT: SetItemStockResult = {
  adjustmentId: 1, delta: 5, reason: 'UNRECORDED_STOCK', previousStock: 10, newStock: 15, unitCost: 6, valueChange: 30,
};
const UP: SetItemStockResult = { ...RESULT, adjustmentId: null };
const DOWN: SetItemStockResult = {
  adjustmentId: null, delta: -6, reason: 'COUNT_CORRECTION', previousStock: 14, newStock: 8, unitCost: null, valueChange: -34,
};
const SAME: SetItemStockResult = {
  adjustmentId: null, delta: 0, reason: null, previousStock: 10, newStock: 10, unitCost: null, valueChange: 0,
};

const settle = async () => {
  for (let i = 0; i < 3; i++) await flush(120);
};

const open = async (extra: Record<string, unknown> = {}, item = ITEM) => {
  const onClose = vi.fn();
  const onDone = vi.fn();
  const api = installApi({
    previewSetItemStock: async () => UP,
    setItemStock: RESULT,
    previewStockAdjustment: async () => ({ previousStock: 10, newStock: 13, unitCost: 6, lossValue: 0, valueChange: 18 }),
    createStockAdjustment: { adjustmentId: 2, lossValue: 0 },
    ...extra,
  });
  render(<SetStockDialog isOpen item={item} onClose={onClose} onDone={onDone} />);
  await settle();
  return { api, onClose, onDone };
};

const nums = () => document.querySelectorAll('input[type="number"]');
const preview = () => document.querySelector('[data-testid="set-stock-preview"]')!.textContent ?? '';

describe('SetStockDialog', () => {
  beforeEach(() => {
    document.body.innerHTML = '';
  });

  it("shows the unit cost the backend used (the latest-lot default) and sends none until one is typed", async () => {
    const { api } = await open();
    setInput(nums()[0], '15');
    await settle();
    expect((nums()[1] as HTMLInputElement).value).toBe('6');
    expect(api.previewSetItemStock.mock.calls.at(-1)![0]).toMatchObject({ countedQuantity: 15, unitCost: null });
  });

  it('set up: shows the previewed value, posts setItemStock and shows the result', async () => {
    const { api, onDone } = await open();
    setInput(nums()[0], '15');
    await settle();
    expect(preview()).toContain('+5');
    expect(preview()).toContain('₹30.00');
    click(buttonByText('Save stock'));
    await flush();
    expect(api.setItemStock.mock.calls[0][0]).toEqual({
      itemId: 9, countedQuantity: 15, unitCost: null, adjustmentDate: todayLocal(), notes: null,
    });
    expect(document.querySelector('[data-testid="set-stock-result"]')!.textContent).toContain('15');
    expect(onDone).toHaveBeenCalledTimes(1);
  });

  it('a typed unit cost is sent', async () => {
    const { api } = await open();
    setInput(nums()[0], '15');
    await settle();
    setInput(nums()[1], '4.5');
    await settle();
    expect(api.previewSetItemStock.mock.calls.at(-1)![0]).toMatchObject({ unitCost: 4.5 });
    click(buttonByText('Save stock'));
    await flush();
    expect(api.setItemStock.mock.calls[0][0]).toMatchObject({ unitCost: 4.5 });
  });

  it('shows the backend refusal (no lots, no cost) and does not save', async () => {
    const { api } = await open({
      previewSetItemStock: async () => {
        throw { type: 'ValidationError', details: { field: 'unit_cost', message: 'unit cost required: item has no prior lot' } };
      },
    });
    setInput(nums()[0], '15');
    await settle();
    expect(bodyText()).toContain('unit cost required');
    expect((buttonByText('Save stock') as HTMLButtonElement).disabled).toBe(true);
    click(buttonByText('Save stock'));
    await flush();
    expect(api.setItemStock).not.toHaveBeenCalled();
  });

  it('set down: shows the backend write-off for a count correction and sends no unit cost', async () => {
    const { api } = await open({ previewSetItemStock: async () => DOWN }, { ...ITEM, currentStock: 14 });
    setInput(nums()[0], '8');
    await settle();
    expect(preview()).toContain('Count correction');
    expect(preview()).toContain('₹34.00');
    click(buttonByText('Save stock'));
    await flush();
    expect(api.setItemStock.mock.calls[0][0]).toMatchObject({ countedQuantity: 8, unitCost: null });
  });

  it('equal count shows "No change" and disables Save', async () => {
    const { api } = await open({ previewSetItemStock: async (_p: SetItemStockPayload) => SAME });
    expect(preview()).toContain('No change');
    const save = buttonByText('Save stock');
    expect(save.disabled).toBe(true);
    click(save);
    await flush();
    expect(api.setItemStock).not.toHaveBeenCalled();
  });

  it('add mode posts an UNRECORDED_STOCK adjustment', async () => {
    const { api } = await open();
    setInput(document.querySelector('select')!, 'ADD');
    await flush();
    setInput(nums()[0], '3');
    await settle();
    click(buttonByText('Save stock'));
    await flush();
    expect(api.createStockAdjustment.mock.calls[0][0]).toMatchObject({
      quantity: 3, reason: 'UNRECORDED_STOCK', unitCost: null, itemId: 9,
    });
    expect(api.setItemStock).not.toHaveBeenCalled();
    expect(document.querySelector('[data-testid="set-stock-result"]')!.textContent).toContain('13');
  });

  it('Cancel closes without any write', async () => {
    const { api, onClose } = await open();
    click(buttonByText('Cancel'));
    expect(onClose).toHaveBeenCalledTimes(1);
    expect(api.setItemStock).not.toHaveBeenCalled();
    expect(api.createStockAdjustment).not.toHaveBeenCalled();
  });
});
