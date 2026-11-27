import { describe, it, expect, beforeEach, vi } from 'vitest';
import { render } from '../testUtils';
import { StockAdjustmentModal } from '@/components/domain/StockAdjustmentModal';
import { todayLocal } from '@/lib/dates';
import { installApi, flush, click, setInput, buttonByText, bodyText } from './helpers';
import type { ItemSearchResult } from '@/types';

const ITEM: ItemSearchResult = {
  itemId: 9,
  name: 'Gel Pen',
  category: null,
  barcode: null,
  baseUnit: 'PCS', itemType: 'PHYSICAL',
  mrp: null,
  defaultSalePrice: 10,
  currentStock: 5,
  isLowStock: false, lowStockThreshold: 5,
  bundles: [],
  matchedBundleId: null,
};
vi.mock('@/components/domain/ItemSearchTypeahead', () => ({
  ItemSearchTypeahead: (p: { onSelectItem: (i: ItemSearchResult) => void }) => (
    <button type="button" data-testid="pick-item" onClick={() => p.onSelectItem(ITEM)}>
      pick
    </button>
  ),
}));


// DD-56: the default cost, the valuation effect, the write-off and the refusals are the backend's
// `preview_stock_adjustment`; canned here.
const settle = async () => {
  for (let i = 0; i < 3; i++) await flush(120);
};
const previewOf = (p: { quantity: number; unitCost: number | null }) => {
  if (p.quantity > 0 && p.unitCost == null) {
    throw { type: 'ValidationError', details: { field: 'unit_cost', message: 'unit cost required: item has no prior lot' } };
  }
  return {
    previousStock: 5,
    newStock: 5 + p.quantity,
    unitCost: p.quantity > 0 ? p.unitCost : null,
    lossValue: p.quantity < 0 ? 40 : 0,
    valueChange: p.quantity > 0 ? p.quantity * (p.unitCost ?? 0) : -40,
  };
};

describe('StockAdjustmentModal (item search)', () => {
  beforeEach(() => {
    document.body.innerHTML = '';
  });

  it('opens without an item, searches one; an increase needs a cost the backend accepts', async () => {
    const api = installApi({
      previewStockAdjustment: async (p: { quantity: number; unitCost: number | null }) => previewOf(p),
      createStockAdjustment: { adjustmentId: 4, lossValue: 0 },
    });
    render(<StockAdjustmentModal isOpen onClose={() => {}} />);
    click(document.querySelector('[data-testid="pick-item"]')!);
    await flush();
    expect(bodyText()).toContain('Gel Pen');
    const select = document.querySelector('select')!;
    setInput(select, 'INCREASE');
    await settle();
    expect(bodyText()).toContain('unit cost required');
    click(buttonByText('Apply Adjustment'));
    await flush();
    expect(api.createStockAdjustment).not.toHaveBeenCalled();
    const nums = document.querySelectorAll('input[type="number"]');
    setInput(nums[0], '2.5'); // quantity
    setInput(nums[1], '12.75'); // unit cost
    await settle();
    click(buttonByText('Apply Adjustment'));
    await flush();
    expect(api.createStockAdjustment.mock.calls[0][0]).toEqual({
      adjustmentDate: todayLocal(),
      itemId: 9,
      quantity: 2.5,
      reason: 'UNRECORDED_STOCK', // S3: Increase switches a reduction-only reason to found stock
      unitCost: 12.75,
      notes: null,
    });
    expect(document.querySelector('[data-testid="adjustment-result"]')).not.toBeNull();
  });

  it('UNRECORDED_STOCK forces an increase, shows the valuation effect and posts the reason', async () => {
    const api = installApi({
      previewStockAdjustment: async (p: { quantity: number; unitCost: number | null }) => previewOf(p),
      createStockAdjustment: { adjustmentId: 6, lossValue: 0 },
    });
    render(<StockAdjustmentModal isOpen onClose={() => {}} itemId={9} itemName="Gel Pen" currentStock={5} baseUnit="PCS" />);
    await flush();
    const selects = () => document.querySelectorAll('select');
    setInput(selects()[0], 'INCREASE'); // the reason is only offered for increases
    await flush();
    setInput(selects()[1], 'UNRECORDED_STOCK');
    await flush();
    expect((selects()[0] as HTMLSelectElement).value).toBe('INCREASE');
    const nums = document.querySelectorAll('input[type="number"]');
    setInput(nums[0], '2');
    setInput(nums[1], '10');
    await settle();
    expect(document.querySelector('[data-testid="valuation-effect"]')!.textContent).toContain('₹20.00');
    click(buttonByText('Apply Adjustment'));
    await flush();
    expect(api.createStockAdjustment.mock.calls[0][0]).toMatchObject({
      reason: 'UNRECORDED_STOCK', quantity: 2, unitCost: 10,
    });
    expect(document.querySelector('[data-testid="adjustment-result"]')!.textContent).toContain('₹20.00');
  });

  it('the decrease direction does not offer UNRECORDED_STOCK', async () => {
    installApi({ previewStockAdjustment: async (p: { quantity: number; unitCost: number | null }) => previewOf(p) });
    render(<StockAdjustmentModal isOpen onClose={() => {}} itemId={9} itemName="Gel Pen" currentStock={5} baseUnit="PCS" />);
    await flush();
    const values = [...document.querySelectorAll('select')[1].querySelectorAll('option')].map((o) => o.value);
    expect(values).not.toContain('UNRECORDED_STOCK');
    expect(values).toContain('THEFT_LOST');
  });

  it('shows the write-off value after a decrease', async () => {
    const api = installApi({
      previewStockAdjustment: async (p: { quantity: number; unitCost: number | null }) => previewOf(p),
      createStockAdjustment: { adjustmentId: 5, lossValue: 40 },
    });
    render(<StockAdjustmentModal isOpen onClose={() => {}} itemId={9} itemName="Gel Pen" currentStock={5} baseUnit="PCS" />);
    await settle();
    expect(document.querySelector('[data-testid="writeoff-effect"]')!.textContent).toContain('₹40.00');
    click(buttonByText('Apply Adjustment'));
    await flush();
    expect(api.createStockAdjustment.mock.calls[0][0]).toMatchObject({ quantity: -1, unitCost: null, itemId: 9 });
    expect(document.querySelector('[data-testid="adjustment-result"]')!.textContent).toContain('₹40.00');
  });
});

describe('StockAdjustmentModal found stock (DD-92)', () => {
  beforeEach(() => {
    document.body.innerHTML = '';
  });

  it('Increase offers only the found reasons, sends a typed 0 cost and offers Undo of an earlier write-off', async () => {
    const api = installApi({
      previewStockAdjustment: async (p: { quantity: number; unitCost: number | null }) => ({
        ...previewOf({ ...p, unitCost: p.unitCost ?? 6 }),
        undoableWriteoffs: p.quantity > 0 ? [{ adjustmentId: 7, adjustmentDate: '2026-11-20', quantity: 2, reason: 'DAMAGED', loss: 12 }] : [],
      }),
      undoStockAdjustment: { canUndo: true },
    });
    render(<StockAdjustmentModal isOpen onClose={() => {}} itemId={9} itemName="Gel Pen" currentStock={5} baseUnit="PCS" />);
    await flush();
    const selects = () => document.querySelectorAll('select');
    setInput(selects()[0], 'INCREASE');
    await settle();
    const reasons = Array.from((selects()[1] as HTMLSelectElement).options).map((o) => o.value);
    expect(reasons).toEqual(['UNRECORDED_STOCK', 'COUNT_CORRECTION']);
    const nums = document.querySelectorAll('input[type="number"]');
    setInput(nums[1], '0');
    await settle();
    expect(api.previewStockAdjustment.mock.calls.at(-1)![0]).toMatchObject({ quantity: 1, unitCost: 0 });
    expect(document.querySelector('[data-testid="undo-writeoff-hint"]')!.textContent).toContain('#7');
    const calls = api.previewStockAdjustment.mock.calls.length;
    click(buttonByText('Undo'));
    await settle();
    expect(api.undoStockAdjustment).toHaveBeenCalledWith(7);
    expect(api.previewStockAdjustment.mock.calls.length).toBeGreaterThan(calls);
  });
});
