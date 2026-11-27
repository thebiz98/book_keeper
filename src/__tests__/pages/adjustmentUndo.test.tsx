import { describe, it, expect, beforeEach } from 'vitest';
import { render } from '../testUtils';
import { AdjustmentsHistoryList } from '@/components/domain/ReturnsHistoryList';
import type { AdjustmentUndoGuard, AdjustmentListRow } from '@/types';
import { installApi, flush, waitFor, click, buttonByText, bodyText } from './helpers';

const adj = (over: Partial<AdjustmentListRow> = {}): AdjustmentListRow => ({
  id: 7, adjustmentDate: '2026-11-26', itemId: 1, itemName: 'Pen', baseUnit: 'PCS', itemIsActive: true, quantity: -5, unitCost: null,
  reason: 'COUNT_CORRECTION', notes: null, createdAt: '2026-11-26T00:00:00Z', value: 50, ...over,
});
const page = (items: AdjustmentListRow[]) => ({ items, total: items.length, page: 1, limit: 10 });
const guard = (over: Partial<AdjustmentUndoGuard> = {}): AdjustmentUndoGuard => ({
  canUndo: true, reason: null, quantity: -5, itemId: 1, itemName: 'Pen', baseUnit: 'PCS', lossRemoved: 50, valueChange: 50, ...over,
});
const undoBtn = () => document.querySelector('button[aria-label="Undo adjustment #7 (Pen)"]') as HTMLButtonElement;

describe('AdjustmentsHistoryList undo', () => {
  beforeEach(() => {
    document.body.innerHTML = '';
  });

  it('shows the effect and calls undo on confirm, then reloads', async () => {
    const api = installApi({
      listStockAdjustments: page([adj()]),
      getAdjustmentUndoGuard: guard(),
      undoStockAdjustment: guard(),
    });
    render(<AdjustmentsHistoryList reloadKey={0} />);
    await waitFor(() => !!undoBtn());
    click(undoBtn());
    await waitFor(() => bodyText().includes('Puts 5 PCS of Pen back into stock'));
    expect(bodyText()).toContain('Inventory Losses');
    expect(bodyText()).not.toContain('#1');
    expect(bodyText()).toContain('Pen');
    expect(bodyText()).toContain('−5 PCS');
    expect(bodyText()).toContain('Loss');
    expect(bodyText()).toContain('50');
    expect(api.getAdjustmentUndoGuard).toHaveBeenCalledWith(7);
    click(buttonByText(/^Undo/, document.querySelector('[role="dialog"]') ?? document.body));
    await flush();
    expect(api.undoStockAdjustment).toHaveBeenCalledWith(7);
    expect(api.listStockAdjustments).toHaveBeenCalledTimes(2);
  });

  it('shows the blocking reason and disables Undo', async () => {
    const api = installApi({
      undoStockAdjustment: guard(),
      listStockAdjustments: page([adj({ quantity: 2, reason: 'UNRECORDED_STOCK', unitCost: 10 })]),
      getAdjustmentUndoGuard: guard({
        canUndo: false, quantity: 2, lossRemoved: 0,
        reason: { code: 'ADJUSTMENT_STOCK_USED', message: 'Stock added by this adjustment has already been sold or used. Delete those sales first.' },
      }),
    });
    render(<AdjustmentsHistoryList reloadKey={0} />);
    await waitFor(() => !!undoBtn());
    click(undoBtn());
    await waitFor(() => bodyText().includes('Delete those sales first'));
    const confirm = buttonByText(/^Undo/, document.querySelector('[role="dialog"]') ?? document.body);
    expect(confirm.disabled).toBe(true);
    click(confirm);
    await flush();
    expect(api.undoStockAdjustment).not.toHaveBeenCalled();
  });

  it('shows the item name, deleted marker, unit cost and value of an increase', async () => {
    installApi({
      listStockAdjustments: page([adj({ id: 8, itemName: 'Gel Pen', itemIsActive: false, quantity: 35, reason: 'UNRECORDED_STOCK', unitCost: 10, value: 350 })]),
    });
    render(<AdjustmentsHistoryList reloadKey={0} />);
    await waitFor(() => bodyText().includes('Gel Pen'));
    expect(bodyText()).toContain('(deleted)');
    expect(bodyText()).toContain('+35 PCS');
    expect(bodyText()).toContain('Added');
    expect(bodyText()).toContain('350');
    expect(bodyText()).toContain('#8');
    expect(document.querySelector('button[aria-label="Undo adjustment #8 (Gel Pen)"]')).not.toBeNull();
  });
});
