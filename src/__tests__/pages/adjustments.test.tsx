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

describe('StockAdjustmentModal (item search)', () => {
  beforeEach(() => {
    document.body.innerHTML = '';
  });

  it('opens without an item, searches one, and posts date + unitCost for an increase', async () => {
    const api = installApi({
      getItemLots: [],
      createStockAdjustment: { adjustmentId: 4, lossValue: 0 },
    });
    render(<StockAdjustmentModal isOpen onClose={() => {}} />);
    click(document.querySelector('[data-testid="pick-item"]')!);
    await flush();
    expect(bodyText()).toContain('Gel Pen');
    const select = document.querySelector('select')!;
    setInput(select, 'INCREASE');
    await flush();
    click(buttonByText('Apply Adjustment'));
    await flush();
    expect(bodyText()).toContain('Unit cost is required');
    expect(api.createStockAdjustment).not.toHaveBeenCalled();
    const nums = document.querySelectorAll('input[type="number"]');
    setInput(nums[0], '2.5'); // quantity
    setInput(nums[1], '12.75'); // unit cost
    click(buttonByText('Apply Adjustment'));
    await flush();
    expect(api.createStockAdjustment.mock.calls[0][0]).toEqual({
      adjustmentDate: todayLocal(),
      itemId: 9,
      quantity: 2.5,
      reason: 'DAMAGED',
      unitCost: 12.75,
      notes: null,
    });
    expect(document.querySelector('[data-testid="adjustment-result"]')).not.toBeNull();
  });

  it('shows the write-off value after a decrease', async () => {
    const api = installApi({
      getItemLots: [],
      createStockAdjustment: { adjustmentId: 5, lossValue: 40 },
    });
    render(<StockAdjustmentModal isOpen onClose={() => {}} itemId={9} itemName="Gel Pen" currentStock={5} baseUnit="PCS" />);
    await flush();
    click(buttonByText('Apply Adjustment'));
    await flush();
    expect(api.createStockAdjustment.mock.calls[0][0]).toMatchObject({ quantity: -1, unitCost: null, itemId: 9 });
    expect(document.querySelector('[data-testid="adjustment-result"]')!.textContent).toContain('₹40.00');
  });
});
