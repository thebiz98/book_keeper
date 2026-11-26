import { describe, it, expect, beforeEach, vi } from 'vitest';
import { render } from '../testUtils';
import { installApi, flush, waitFor, click, setInput, fakeBillPreview } from '../pages/helpers';
import { BillDetailDrawer } from '@/components/domain/BillDetailDrawer';
import { purchaseDetail, purchaseGuard, saleDetail, saleGuard, search, getItem, PEN } from './fixtures';
import type { PreviewError, SaleDetailView } from '@/types';

(globalThis as unknown as { IS_REACT_ACT_ENVIRONMENT: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

vi.mock('@/components/domain/ItemSearchTypeahead', () => ({ ItemSearchTypeahead: () => <div /> }));
vi.mock('@/components/domain/PartySelector', () => ({ PartySelector: () => <div data-testid="stub-party" /> }));

const q = (sel: string) => document.querySelector(sel) as HTMLInputElement;
const settle = async () => {
  for (let i = 0; i < 4; i++) await flush(120);
};
const ctrlS = () =>
  window.dispatchEvent(new KeyboardEvent('keydown', { key: 's', ctrlKey: true, bubbles: true, cancelable: true }));

type PreviewIn = Parameters<typeof fakeBillPreview>[0];

function mountSale(opts: { detail?: SaleDetailView; preview?: (p: PreviewIn) => unknown; extra?: Record<string, unknown> } = {}) {
  const api = installApi({
    getSaleDetails: opts.detail ?? saleDetail(),
    getBillEditGuard: saleGuard(),
    searchItemsFts: search,
    getItem,
    previewSale: opts.preview ?? (async (p: PreviewIn) => fakeBillPreview(p)),
    updateSale: { saleId: 5, transactionCode: 'SAL-1005', totalAmount: 0, paidAmount: 0, balanceDue: 0, changeAmount: 0, paymentStatus: 'PAID' },
    getDashboardMetrics: { cashInHand: 0 },
    ...opts.extra,
  });
  render(<BillDetailDrawer kind="SALE" billId={5} onClose={() => {}} />);
  return api;
}

function mountPurchase() {
  const api = installApi({
    getPurchaseDetails: purchaseDetail(),
    getBillEditGuard: purchaseGuard(),
    searchItemsFts: search,
    getItem,
    previewPurchase: async (p: PreviewIn) => fakeBillPreview(p),
    updatePurchase: { purchaseId: 8, transactionCode: 'PUR-1008' },
    getDashboardMetrics: { cashInHand: 0 },
  });
  render(<BillDetailDrawer kind="PURCHASE" billId={8} onClose={() => {}} />);
  return api;
}

async function openEdit() {
  await waitFor(() => !!document.querySelector('[data-testid="bill-edit"]:not([disabled])'));
  click(q('[data-testid="bill-edit"]'));
  await waitFor(() => document.querySelectorAll('[data-testid="edit-line"]').length > 0);
  await settle();
}

describe('change-only bill edit (FR-55, DD-89)', () => {
  beforeEach(() => {
    document.body.innerHTML = '';
  });

  it('E3: an untouched sale form cannot save, by button or Ctrl+S; a change enables it', async () => {
    const api = mountSale();
    await openEdit();
    expect(q('[data-testid="edit-save"]').disabled).toBe(true);
    ctrlS();
    await flush(50);
    expect(api.updateSale).not.toHaveBeenCalled();
    setInput(q('input[aria-label="Quantity Ink"]'), '2');
    await settle();
    expect(q('[data-testid="edit-save"]').disabled).toBe(false);
    ctrlS();
    await settle();
    expect(api.updateSale).toHaveBeenCalledTimes(1);
  });

  it('E3: an untouched purchase form cannot save either', async () => {
    const api = mountPurchase();
    await openEdit();
    expect(q('[data-testid="edit-save"]').disabled).toBe(true);
    ctrlS();
    await flush(50);
    expect(api.updatePurchase).not.toHaveBeenCalled();
  });

  it('E4: lines load by item id and the backend bundle id, so a renamed item keeps its pack', async () => {
    const d = saleDetail();
    // The Pen line was sold as 2 Boxes when the item had another name.
    d.items[0] = {
      ...d.items[0],
      saleItem: { ...d.items[0].saleItem, itemName: 'Old pen', packLabel: 'Old box', packQuantity: 2, quantity: 20 },
      bundleId: PEN.bundles[0].bundleId,
    };
    const api = mountSale({ detail: d });
    await openEdit();
    expect(api.getItem).toHaveBeenCalledWith(1);
    expect(api.searchItemsFts).not.toHaveBeenCalled();
    expect(q('select[aria-label="Unit Old pen"]').value).toBe(String(PEN.bundles[0].bundleId));
    expect(q('input[aria-label="Quantity Old pen"]').value).toBe('2');
    expect(q('input[aria-label="Rate Old pen"]').value).toBe('50');
  });

  it('D10: a deleted item in the preview offers Restore, which calls restoreItem and re-runs the preview', async () => {
    let restored = false;
    const DELETED: PreviewError = {
      field: 'item_id',
      message: "'Pen' is deleted. Restore it first.",
      deleted: { entity: 'ITEM', id: 1, name: 'Pen' },
    };
    const api = mountSale({
      preview: async (p: PreviewIn) => fakeBillPreview(p, { errors: restored ? [] : p.items[0].bundleQuantity !== 2 ? [DELETED] : [] }),
      extra: {
        restoreItem: vi.fn(async () => {
          restored = true;
        }),
      },
    });
    await openEdit();
    setInput(q('input[aria-label="Quantity Pen"]'), '3');
    await settle();
    expect(q('[data-testid="deleted-ref-alert"]').textContent).toContain("'Pen' is deleted. Restore it first.");
    expect(q('[data-testid="edit-save"]').disabled).toBe(true);
    click(q('[data-testid="deleted-ref-restore"]'));
    await settle();
    expect(api.restoreItem).toHaveBeenCalledWith(1);
    expect(document.querySelector('[data-testid="deleted-ref-alert"]')).toBeNull();
    expect(q('input[aria-label="Quantity Pen"]').value).toBe('3');
    expect(q('[data-testid="edit-save"]').disabled).toBe(false);
  });
});
