import { describe, it, expect, beforeEach, vi } from 'vitest';
import { render } from '../testUtils';
import { BillDetailDrawer } from '@/components/domain/BillDetailDrawer';
import { ReturnDetailDrawer } from '@/components/domain/ReturnDetailDrawer';
import { NewSalePage } from '@/pages/NewSalePage';
import { useAppStore } from '@/store/useAppStore';
import { useCartStore } from '@/store/useCartStore';
import { queueBillEdit, takeBillEdit } from '@/pages/billEditHandoff';
import { __resetHotkeys } from '@/hooks/useHotkeys';
import type { BillEditGuard, SaleDetailView } from '@/types';
import { installApi, flush, waitFor, click, setInput, bodyText, fakeBillPreview } from './helpers';

const guard = (over: Partial<BillEditGuard> = {}): BillEditGuard => ({
  target: 'SALE', id: 5, code: 'SAL-1005', canEdit: true, canDelete: true, canChangeParty: true, reasons: [],
  upfrontAmount: 100, appliedAmount: 0, removedCashAmount: 100, removedCreditAmount: 0, lotLimits: [], ...over,
});

const saleDetail = (): SaleDetailView =>
  ({
    sale: {
      id: 5, transactionCode: 'SAL-1005', transactionDate: '2026-11-20', partyId: null, partyName: null, paymentMode: 'CASH',
      paymentStatus: 'PAID', totalAmount: 100, paidAmount: 100, balanceDue: 0, notes: 'hi', createdAt: '', itemsSubtotal: 100,
      chargesTotal: 0, discountsTotal: 0, updatedAt: '2026-11-21T10:00:00Z',
    },
    party: null,
    settlements: [],
    charges: [],
    items: [
      {
        saleItem: { id: 1, saleTransactionId: 5, itemId: 1, itemName: 'Pen', packLabel: null, packQuantity: 2, quantity: 2, unitPrice: 50, totalAmount: 100 },
        itemName: 'Pen', bundleName: null, returnedQuantity: 0, allocations: [], netUnitPrice: 50,
      },
    ],
  }) as unknown as SaleDetailView;

const PEN = {
  itemId: 1, name: 'Pen', category: null, barcode: null, baseUnit: 'PCS', itemType: 'PHYSICAL', mrp: null,
  defaultSalePrice: 50, currentStock: 0, isLowStock: false, lowStockThreshold: 5, bundles: [], matchedBundleId: null,
};

describe('BillDetailDrawer actions (FR-20)', () => {
  beforeEach(() => {
    document.body.innerHTML = '';
    useCartStore.getState().clearCart();
    takeBillEdit();
  });

  it('enables Edit and Delete from the guard; Edit opens the form in the drawer without navigating', async () => {
    installApi({ getSaleDetails: saleDetail(), getBillEditGuard: guard(), searchItemsFts: [PEN], previewSale: async (p: Parameters<typeof fakeBillPreview>[0]) => fakeBillPreview(p) });
    const onClose = vi.fn();
    render(<BillDetailDrawer kind="SALE" billId={5} onClose={onClose} />);
    await waitFor(() => !!document.querySelector('[data-testid="bill-edit"]:not([disabled])'));
    expect(bodyText()).toContain('Edited 2026-11-21');
    click(document.querySelector('[data-testid="bill-edit"]')!);
    await waitFor(() => !!document.querySelector('[data-testid="sale-edit-form"] [data-testid="edit-line"]'));
    expect(bodyText()).toContain('Editing SAL-1005');
    expect(useAppStore.getState().activeRoute).not.toBe('sale');
    expect(onClose).not.toHaveBeenCalled();
  });

  it('disables both buttons and lists the reasons when the guard blocks', async () => {
    installApi({
      getSaleDetails: saleDetail(),
      getBillEditGuard: guard({
        canEdit: false, canDelete: false,
        reasons: [{ code: 'LINKED_RETURNS', message: 'This bill has returns (RET-1001). Delete them first.', blocksEdit: true, blocksDelete: true }],
      }),
    });
    render(<BillDetailDrawer kind="SALE" billId={5} onClose={() => {}} />);
    await waitFor(() => bodyText().includes('RET-1001'));
    expect((document.querySelector('[data-testid="bill-edit"]') as HTMLButtonElement).disabled).toBe(true);
    expect((document.querySelector('[data-testid="bill-delete"]') as HTMLButtonElement).disabled).toBe(true);
  });

  it('delete needs the typed code, then calls deleteSale and onChanged', async () => {
    const api = installApi({ getSaleDetails: saleDetail(), getBillEditGuard: guard(), deleteSale: { removedCashAmount: 100 } });
    const onChanged = vi.fn();
    const onClose = vi.fn();
    render(<BillDetailDrawer kind="SALE" billId={5} onClose={onClose} onChanged={onChanged} />);
    await waitFor(() => !!document.querySelector('[data-testid="bill-delete"]:not([disabled])'));
    click(document.querySelector('[data-testid="bill-delete"]')!);
    expect(bodyText()).toContain('Delete SAL-1005?');
    expect(bodyText()).toContain('₹100.00 cash received');
    const confirm = Array.from(document.querySelectorAll('button')).find((b) => b.textContent?.trim().startsWith('Delete') && b.closest('[role="dialog"]') && b.textContent?.includes('Enter'))!;
    expect((confirm as HTMLButtonElement).disabled).toBe(true);
    setInput(document.querySelector('[data-testid="confirm-require-text"]')!, 'SAL-1005');
    click(confirm);
    await flush();
    expect(api.deleteSale).toHaveBeenCalledWith(5);
    expect(onChanged).toHaveBeenCalled();
    expect(onClose).toHaveBeenCalled();
  });

  it('shows a BillLocked rejection inline and does not close', async () => {
    installApi({
      getSaleDetails: saleDetail(),
      getBillEditGuard: guard(),
      deleteSale: () => Promise.reject({ type: 'BillLocked', details: { code: 'LINKED_RETURNS', message: 'Locked now.' } }),
    });
    const onClose = vi.fn();
    render(<BillDetailDrawer kind="SALE" billId={5} onClose={onClose} />);
    await waitFor(() => !!document.querySelector('[data-testid="bill-delete"]:not([disabled])'));
    click(document.querySelector('[data-testid="bill-delete"]')!);
    setInput(document.querySelector('[data-testid="confirm-require-text"]')!, 'SAL-1005');
    const confirm = Array.from(document.querySelectorAll('button')).find((b) => b.textContent?.includes('Delete') && b.textContent?.includes('Enter'))!;
    click(confirm);
    await waitFor(() => bodyText().includes('Locked now.'));
    expect(onClose).not.toHaveBeenCalled();
  });
});

describe('ReturnDetailDrawer (FR-20)', () => {
  beforeEach(() => {
    document.body.innerHTML = '';
  });

  const ret = {
    returnTx: { id: 3, returnCode: 'RET-1003', returnDate: '2026-11-20', returnType: 'CUSTOMER', partyId: null, partyName: null, originalTransactionId: 5, settlementType: 'REFUND', totalAmount: 30, reason: null, notes: null, createdAt: '' },
    party: null, items: [], allocations: [], ledger: [],
  };

  it('delete calls deleteReturn after the typed code and reloads the list', async () => {
    const api = installApi({
      getReturnDetails: ret,
      getBillEditGuard: guard({ target: 'RETURN', id: 3, code: 'RET-1003', canEdit: false, removedCashAmount: 30 }),
      deleteReturn: { removedCashAmount: 30, removedCreditAmount: 0 },
    });
    const onChanged = vi.fn();
    render(<ReturnDetailDrawer returnId={3} onClose={() => {}} onChanged={onChanged} />);
    await waitFor(() => !!document.querySelector('[data-testid="return-delete"]:not([disabled])'));
    click(document.querySelector('[data-testid="return-delete"]')!);
    setInput(document.querySelector('[data-testid="confirm-require-text"]')!, 'RET-1003');
    click(Array.from(document.querySelectorAll('button')).find((b) => b.textContent?.includes('Delete return') && b.textContent?.includes('Enter'))!);
    await flush();
    expect(api.deleteReturn).toHaveBeenCalledWith(3);
    expect(onChanged).toHaveBeenCalled();
  });
});

describe('NewSalePage edit mode (FR-20)', () => {
  beforeEach(() => {
    document.body.innerHTML = '';
    useCartStore.getState().clearCart();
    __resetHotkeys();
  });

  it('prefills from the queued bill, shows the banner and submits updateSale', async () => {
    const api = installApi({
      getSaleDetails: saleDetail(),
      getBillEditGuard: guard(),
      searchItemsFts: [PEN],
      getNextSaleCode: 'SAL-9999',
      listParties: [],
      searchParties: [],
      searchChargeTypes: [],
      getDashboardMetrics: { liveCashInDrawer: 0 },
      previewSale: async (p: Parameters<typeof fakeBillPreview>[0]) => fakeBillPreview(p),
      updateSale: { saleId: 5, transactionCode: 'SAL-1005', totalAmount: 100, paidAmount: 100, balanceDue: 0, changeAmount: 0, paymentStatus: 'PAID' },
    });
    queueBillEdit({ kind: 'SALE', id: 5 });
    render(<NewSalePage />);
    await waitFor(() => !!document.querySelector('[data-testid="edit-banner"]'));
    expect(bodyText()).toContain('Editing SAL-1005');
    expect(document.querySelector('[data-testid="invoice-preview"]')?.textContent).toBe('SAL-1005');
    expect(useCartStore.getState().items[0]).toMatchObject({ itemId: 1, quantity: 2, rate: 50 });
    expect(api.getNextSaleCode).not.toHaveBeenCalled();
    click(document.querySelector('[data-testid="complete-sale-btn"]')!);
    await waitFor(() => bodyText().includes('Update bill'));
    // The edit previews carry the sale id so the backend runs the edit rules.
    await waitFor(() => !(document.querySelector('[data-testid="confirm-btn"]') as HTMLButtonElement).disabled);
    expect(api.previewSale.mock.calls.at(-1)![1]).toBe(5);
    click(document.querySelector('[data-testid="confirm-btn"]')!);
    await flush();
    expect(api.updateSale).toHaveBeenCalledTimes(1);
    expect(api.updateSale.mock.calls[0][0]).toBe(5);
    expect(api.updateSale.mock.calls[0][1]).toMatchObject({ transactionDate: '2026-11-20', paidAmount: 100, items: [{ itemId: 1, bundleQuantity: 2 }] });
    expect(bodyText()).toContain('Updated SAL-1005');
  });

  it('a blocked bill stays in create mode with the reason', async () => {
    installApi({
      getSaleDetails: saleDetail(),
      getBillEditGuard: guard({ canEdit: false, reasons: [{ code: 'LINKED_RETURNS', message: 'Has returns.', blocksEdit: true, blocksDelete: true }] }),
      getNextSaleCode: 'SAL-9999',
      listParties: [],
      searchParties: [],
    });
    queueBillEdit({ kind: 'SALE', id: 5 });
    render(<NewSalePage />);
    await waitFor(() => bodyText().includes('Has returns.'));
    expect(document.querySelector('[data-testid="edit-banner"]')).toBeNull();
    expect(useCartStore.getState().editing).toBeNull();
  });
});
