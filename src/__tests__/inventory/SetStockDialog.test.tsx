(globalThis as unknown as { IS_REACT_ACT_ENVIRONMENT: boolean }).IS_REACT_ACT_ENVIRONMENT = true;
import { describe, it, expect, beforeEach, vi } from 'vitest';
import { render } from '../testUtils';
import { SetStockDialog } from '@/components/domain/SetStockDialog';
import { todayLocal } from '@/lib/dates';
import { installApi, flush, click, setInput, buttonByText, bodyText } from '../pages/helpers';
import type { SetItemStockPayload, SetItemStockResult, SetStockOption, UndoableWriteoff } from '@/types';

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
      itemId: 9, countedQuantity: 15, unitCost: null, adjustmentDate: todayLocal(), notes: null, mode: null,
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

  it('a typed unit cost of 0 is sent as 0, not dropped (S4)', async () => {
    const { api } = await open();
    setInput(nums()[0], '15');
    await settle();
    setInput(nums()[1], '0');
    await settle();
    expect(api.previewSetItemStock.mock.calls.at(-1)![0]).toMatchObject({ unitCost: 0 });
  });
});

// FR-57 scope B: the paths, which are enabled and why not come from the backend preview.
const OPENING_OPT: SetStockOption = {
  mode: 'OPENING_CORRECTION', label: 'Opening stock was entered wrong (no loss)',
  detail: 'Takes 6 PCS off the unused opening stock. No loss is booked.', enabled: true, disabledReason: null, takesUnitCost: false,
};
const LOSS_OPT: SetStockOption = {
  mode: 'ADJUSTMENT', label: 'Lost, damaged or miscounted (loss)',
  detail: 'Records a count correction: its FIFO cost is an Inventory Loss.', enabled: true, disabledReason: null, takesUnitCost: false,
};
const CHOOSE: SetItemStockResult = {
  ...DOWN, reason: null, newStock: 14, valueChange: 0, mode: null, options: [OPENING_OPT, LOSS_OPT], choiceRequired: true, undoableWriteoffs: [],
};
const OPENING_DOWN: SetItemStockResult = {
  ...DOWN, reason: null, valueChange: -60, mode: 'OPENING_CORRECTION', options: [OPENING_OPT, LOSS_OPT], choiceRequired: false, undoableWriteoffs: [],
};
const WRITEOFF: UndoableWriteoff = { adjustmentId: 41, adjustmentDate: '2026-11-20', quantity: 6, reason: 'COUNT_CORRECTION', loss: 60 };

const radio = (mode: string) =>
  document.querySelector(`[data-testid="set-stock-path-${mode}"] input[type="radio"]`) as HTMLInputElement;

describe('SetStockDialog paths (opening correction)', () => {
  beforeEach(() => {
    document.body.innerHTML = '';
  });

  it('two enabled paths: nothing is preselected and Save waits for a pick; the pick is sent', async () => {
    const { api } = await open(
      {
        previewSetItemStock: async (p: SetItemStockPayload) => (p.mode === 'OPENING_CORRECTION' ? OPENING_DOWN : p.mode ? DOWN : CHOOSE),
        setItemStock: { ...OPENING_DOWN, adjustmentId: null },
      },
      { ...ITEM, currentStock: 14 }
    );
    setInput(nums()[0], '8');
    await settle();
    expect(bodyText()).toContain('Why is the stock different?');
    expect(radio('OPENING_CORRECTION').checked).toBe(false);
    expect(radio('ADJUSTMENT').checked).toBe(false);
    expect(preview()).toContain('Choose why the stock is different');
    expect(buttonByText('Save stock').disabled).toBe(true);
    expect(api.getItemLots).toBeUndefined(); // no UI lot read: the paths are the backend's

    click(radio('OPENING_CORRECTION'));
    await settle();
    expect(api.previewSetItemStock.mock.calls.at(-1)![0]).toMatchObject({ countedQuantity: 8, mode: 'OPENING_CORRECTION' });
    expect(preview()).toContain('Opening stock corrected');
    expect(preview()).toContain('No loss');
    click(buttonByText('Save stock'));
    await flush();
    expect(api.setItemStock.mock.calls[0][0]).toMatchObject({ countedQuantity: 8, mode: 'OPENING_CORRECTION' });
    expect(document.querySelector('[data-testid="set-stock-result"]')!.textContent).toContain('No loss was booked');
  });

  it('a disabled path shows the backend reason and the only enabled one is preselected', async () => {
    const disabled = { ...OPENING_OPT, enabled: false, disabledReason: 'Only 0 PCS of the opening stock is unused. The rest is used, including write-off #41 (2026-11-20, 6 PCS): undo those first, then set the stock again.' };
    const { api } = await open(
      { previewSetItemStock: async () => ({ ...DOWN, mode: 'ADJUSTMENT', options: [disabled, LOSS_OPT], choiceRequired: false, undoableWriteoffs: [WRITEOFF] }) },
      { ...ITEM, currentStock: 14 }
    );
    setInput(nums()[0], '8');
    await settle();
    expect(radio('OPENING_CORRECTION').disabled).toBe(true);
    expect(radio('ADJUSTMENT').checked).toBe(true);
    expect(document.querySelector('[data-testid="set-stock-path-disabled"]')!.textContent).toContain('write-off #41');
    expect(document.querySelectorAll('[data-testid="undoable-writeoff"]').length).toBe(1);
    expect(buttonByText('Save stock').disabled).toBe(false);
    click(buttonByText('Save stock'));
    await flush();
    expect(api.setItemStock.mock.calls[0][0]).toMatchObject({ countedQuantity: 8, mode: null });
  });

  it('Undo on a listed write-off calls the G7 undo and re-runs the preview', async () => {
    const onStockChanged = vi.fn();
    const api = installApi({
      previewSetItemStock: async () => ({ ...UP, mode: 'ADJUSTMENT', options: [{ ...LOSS_OPT, takesUnitCost: true }], undoableWriteoffs: [WRITEOFF] }),
      undoStockAdjustment: { canUndo: true },
    });
    render(<SetStockDialog isOpen item={ITEM} onClose={() => {}} onStockChanged={onStockChanged} />);
    await settle();
    setInput(nums()[0], '15');
    await settle();
    expect(document.querySelector('[data-testid="undo-writeoff-hint"]')!.textContent).toContain('#41');
    const calls = api.previewSetItemStock.mock.calls.length;
    click(buttonByText('Undo'));
    await settle();
    expect(api.undoStockAdjustment).toHaveBeenCalledWith(41);
    expect(api.previewSetItemStock.mock.calls.length).toBeGreaterThan(calls);
    expect(onStockChanged).toHaveBeenCalledTimes(1);
  });
});
