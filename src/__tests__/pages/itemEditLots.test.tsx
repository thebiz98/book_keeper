import { describe, it, expect, beforeEach, vi } from 'vitest';
import { act } from 'react';
import { render } from '../testUtils';
import { ItemEditDrawer } from '@/components/domain/ItemEditDrawer';
import { __resetHotkeys } from '@/hooks/useHotkeys';
import type { ItemLot, ItemSearchResult } from '@/types';
import { installApi, flush, waitFor, click, setInput, buttonByText, bodyText } from './helpers';

const item = (over: Partial<ItemSearchResult> = {}): ItemSearchResult => ({
  itemId: 1, name: 'Pen', category: null, barcode: null, baseUnit: 'PCS', itemType: 'PHYSICAL', mrp: null,
  defaultSalePrice: 12, currentStock: 10, isLowStock: false, lowStockThreshold: 5, bundles: [],
  matchedBundleId: null, serviceCost: null, ...over,
} as ItemSearchResult);

const lot: ItemLot = {
  purchaseItemId: 11, purchaseTransactionId: 5, transactionCode: 'PUR-1005', source: 'PURCHASE',
  transactionDate: '2026-11-01', vendorName: 'V', packLabel: null, packQuantity: 10, quantity: 10,
  remainingQuantity: 10, unitCost: 10,
};

const section = () => document.querySelector('[data-testid="item-edit-lots"]');
const editButton = () => document.querySelector('button[aria-label="Edit purchase price of PUR-1005"]') as HTMLButtonElement;
const nameInput = () => document.querySelector('input[type="text"], input:not([type])') as HTMLInputElement;
const saveBtn = () => buttonByText(/^Save(?! Changes)/);
const escape = () =>
  act(() => {
    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true }));
  });

function lotApi(extra: Record<string, unknown> = {}) {
  return installApi({
    getItemLots: [lot],
    getPurchaseDetails: undefined,
    previewLotPurchasePrice: { purchaseItemId: 11, oldUnitCost: 10, newUnitCost: 12, bill: null },
    updateLotPurchasePrice: { purchaseItemId: 11, oldUnitCost: 10, newUnitCost: 12, bill: null },
    ...extra,
  });
}

describe('ItemEditDrawer lots section (FR-44)', () => {
  beforeEach(() => {
    document.body.innerHTML = '';
    __resetHotkeys();
  });

  it('shows the lots of a physical item in edit mode', async () => {
    const api = lotApi();
    render(<ItemEditDrawer isOpen item={item()} onClose={() => {}} onSaved={() => {}} />);
    await waitFor(() => document.querySelectorAll('[data-testid="lot-row"]').length === 1);
    expect(section()).not.toBeNull();
    expect(bodyText()).toContain('Lots & Purchase Price');
    expect(api.getItemLots).toHaveBeenCalledWith(1);
  });

  it('is absent for a service and in create mode', async () => {
    lotApi();
    render(<ItemEditDrawer isOpen item={item({ itemType: 'SERVICE', serviceCost: null })} onClose={() => {}} onSaved={() => {}} />);
    await flush();
    expect(section()).toBeNull();
    document.body.innerHTML = '';
    render(<ItemEditDrawer isOpen item={null} onClose={() => {}} onSaved={() => {}} />);
    await flush();
    expect(section()).toBeNull();
  });

  it('Edit opens the modal for that lot, save updates the price and reloads the lots', async () => {
    const api = lotApi();
    const onSaved = vi.fn();
    render(<ItemEditDrawer isOpen item={item()} onClose={() => {}} onSaved={onSaved} />);
    await waitFor(() => editButton() !== null);
    click(editButton());
    await waitFor(() => document.querySelector('input[aria-label="Purchase price"]') !== null);
    setInput(document.querySelector('input[aria-label="Purchase price"]') as HTMLInputElement, 12);
    await waitFor(() => !saveBtn().disabled);
    click(saveBtn());
    await flush();
    expect(api.updateLotPurchasePrice).toHaveBeenCalledWith({ purchaseItemId: 11, unitPrice: 12 });
    await waitFor(() => api.getItemLots.mock.calls.length >= 2);
    expect(onSaved).toHaveBeenCalled();
  });

  it('keeps a typed name after a lot price save', async () => {
    lotApi();
    render(<ItemEditDrawer isOpen item={item()} onClose={() => {}} onSaved={() => {}} />);
    await waitFor(() => editButton() !== null);
    setInput(nameInput(), 'Pen Deluxe');
    click(editButton());
    await waitFor(() => document.querySelector('input[aria-label="Purchase price"]') !== null);
    setInput(document.querySelector('input[aria-label="Purchase price"]') as HTMLInputElement, 12);
    await waitFor(() => !saveBtn().disabled);
    click(saveBtn());
    await flush(20);
    expect(nameInput().value).toBe('Pen Deluxe');
  });

  it('Esc closes the price modal only, not the drawer', async () => {
    lotApi();
    const onClose = vi.fn();
    render(<ItemEditDrawer isOpen item={item()} onClose={onClose} onSaved={() => {}} />);
    await waitFor(() => editButton() !== null);
    click(editButton());
    await waitFor(() => document.querySelector('input[aria-label="Purchase price"]') !== null);
    escape();
    await flush();
    expect(document.querySelector('input[aria-label="Purchase price"]')).toBeNull();
    expect(onClose).not.toHaveBeenCalled();
    escape();
    await flush();
    expect(onClose).toHaveBeenCalled();
  });
});
