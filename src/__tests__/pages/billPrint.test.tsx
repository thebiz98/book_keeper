import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { act } from 'react';
import { render } from '../testUtils';
import { BillPrintDocument, bundleText, ratePerPack } from '@/components/print/BillPrintDocument';
import { cleanupPrintRoot, pickPrintSize, printBillDocument } from '@/components/print/usePrintBill';
import { BillDetailDrawer } from '@/components/domain/BillDetailDrawer';
import { SaleSummaryModal } from '@/components/domain/SaleSummaryModal';
import { useAppStore } from '@/store/useAppStore';
import type { PurchaseDetailView, SaleDetailView } from '@/types';
import { installApi, flush, waitFor, click, buttonByText, bodyText, fakeBillPreview } from './helpers';

const sale = {
  sale: {
    id: 5, transactionCode: 'SAL-1005', transactionDate: '2026-11-20', partyId: null, partyName: null,
    paymentMode: 'CASH', paymentStatus: 'PARTIAL', totalAmount: 530, paidAmount: 300, balanceDue: 230,
    notes: 'Deliver tomorrow', createdAt: '', itemsSubtotal: 540, chargesTotal: 40, discountsTotal: 50, updatedAt: null,
  },
  party: null,
  settlements: [],
  charges: [
    { id: 1, transactionType: 'SALE', transactionId: 5, chargeTypeId: 1, chargeName: 'Transport', effect: 'CHARGE', amount: 40 },
    { id: 2, transactionType: 'SALE', transactionId: 5, chargeTypeId: 2, chargeName: 'Festival', effect: 'DISCOUNT', amount: 50 },
  ],
  items: [
    { itemName: 'Pencil', returnedQuantity: 0, allocations: [], bundleName: null, netUnitPrice: 20, unitsPerBundle: 12, baseUnit: 'pcs',
      saleItem: { id: 1, packLabel: 'Box of 12', packQuantity: 2, quantity: 24, unitPrice: 20, totalAmount: 480 } },
    { itemName: 'Eraser', returnedQuantity: 0, allocations: [], bundleName: null, netUnitPrice: 20, unitsPerBundle: null, baseUnit: 'pcs',
      saleItem: { id: 2, packLabel: null, packQuantity: 1, quantity: 3, unitPrice: 20, totalAmount: 60 } },
  ],
} as unknown as SaleDetailView;

const purchase = {
  purchase: {
    id: 9, transactionCode: 'PUR-1009', source: 'PURCHASE', supplierInvoiceRef: 'INV-77', transactionDate: '2026-11-21',
    partyId: 3, partyName: 'Acme Vendor', paymentMode: 'CASH', paymentStatus: 'PAID', totalAmount: 100, paidAmount: 100,
    balanceDue: 0, notes: null, createdAt: '', itemsSubtotal: 100, chargesTotal: 0, discountsTotal: 0, updatedAt: null,
  },
  party: null,
  settlements: [],
  charges: [],
  items: [
    { itemName: 'Notebook', bundleName: null, unitsPerBundle: 10, baseUnit: null,
      purchaseItem: { id: 1, packLabel: 'Carton', packQuantity: 4, quantity: 40, unitCost: 2.5, totalAmount: 100, remainingQuantity: 40 } },
  ],
} as unknown as PurchaseDetailView;

const FULL_SHOP = { shopName: 'Sharma Stores', address: '12 Main Road', phone: '98765', taxId: '27ABCDE1234F1Z5' };

const text = (id: string) => document.querySelector(`[data-testid="${id}"]`)?.textContent ?? null;

beforeEach(() => {
  document.body.innerHTML = '';
});
afterEach(() => {
  cleanupPrintRoot();
  vi.restoreAllMocks();
});

describe('ratePerPack', () => {
  it('is total / pack quantity for packs and total / base quantity for loose lines', () => {
    expect(ratePerPack(480, 'Box of 12', 2, 24)).toBe(240);
    expect(ratePerPack(60, null, 1, 3)).toBe(20);
    expect(ratePerPack(0, null, 1, 0)).toBe(0);
  });
});

describe('bundleText', () => {
  it('names the bundle and the base units it holds', () => {
    expect(bundleText('Box', 12, 'pcs')).toBe('Box (12 pcs)');
    expect(bundleText('Roll', 2.5, 'm')).toBe('Roll (2.5 m)');
    expect(bundleText('Box', 12, null)).toBe('Box (12 units)');
    expect(bundleText('Box', null, 'pcs')).toBe('Box');
  });
});

describe('pickPrintSize', () => {
  it('starts at half an A4, grows to a full A4, then to several pages', () => {
    expect(pickPrintSize(0)).toBe('half');
    expect(pickPrintSize(136)).toBe('half');
    expect(pickPrintSize(136.1)).toBe('full');
    expect(pickPrintSize(280)).toBe('full');
    expect(pickPrintSize(280.1)).toBe('multi');
  });
});

describe('BillPrintDocument', () => {
  it('prints a sale with packs, charges, a discount, a balance and a walk-in customer', () => {
    render(<BillPrintDocument kind="SALE" shop={FULL_SHOP} detail={sale} />);
    expect(text('print-shop-name')).toBe('Sharma Stores');
    expect(text('print-shop-address')).toBe('12 Main Road');
    expect(text('print-shop-phone')).toContain('98765');
    expect(text('print-shop-taxid')).toContain('27ABCDE1234F1Z5');
    const body = bodyText();
    expect(body).toContain('Walk-in');
    expect(body).toContain('SAL-1005');
    const lines = Array.from(document.querySelectorAll('[data-testid="print-line"]')).map((r) => r.textContent ?? '');
    expect(lines).toHaveLength(2);
    expect(lines[0]).toContain('Pencil · Box of 12 (12 pcs)'); // bundle name and the units it holds
    expect(lines[0]).toContain('1Pencil');
    expect(lines[0]).toContain('(12 pcs)2₹'); // Qty: 2 bundles
    expect(lines[1]).toMatch(/^2Eraser3 pcs/); // a loose line counts base units
    expect(lines[0]).toContain('240.00'); // rate per pack, not per base unit
    expect(lines[0]).toContain('480.00');
    expect(lines[1]).toContain('Eraser');
    expect(text('print-charges-head')).toContain('Charges / Discount');
    const charges = Array.from(document.querySelectorAll('[data-testid="print-charge"]')).map((r) => r.textContent ?? '');
    expect(charges[0]).toMatch(/^1Transport/);
    expect(charges[1]).toMatch(/^2Less: Festival/);
    expect(charges[1]).toContain('Less: Festival');
    expect(charges[1]).toContain('−₹50.00');
    expect(text('print-total')).toContain('530.00');
    expect(text('print-subtotal')).toContain('540.00');
    expect(text('print-balance')).toContain('230.00');
    expect(text('print-notes')).toContain('Deliver tomorrow');
    expect(text('print-words')).toBe('Rupees Five Hundred Thirty Only');
    expect(text('print-manual-footer')).toContain('For Sharma Stores');
    expect(text('print-manual-footer')).toContain('Authorised signatory');
    expect(text('print-party')).toContain('Customer');
  });

  it('omits empty profile fields and the balance row when nothing is due', () => {
    const paid = { ...sale, sale: { ...sale.sale, balanceDue: 0, notes: null }, charges: [] } as SaleDetailView;
    render(<BillPrintDocument kind="SALE" shop={{ shopName: 'Only Name', phone: '  ', address: null }} detail={paid} />);
    expect(text('print-shop-name')).toBe('Only Name');
    expect(text('print-shop-address')).toBeNull();
    expect(text('print-shop-phone')).toBeNull();
    expect(text('print-shop-taxid')).toBeNull();
    expect(text('print-balance')).toBeNull();
    expect(text('print-notes')).toBeNull();
    expect(text('print-charges-head')).toBeNull(); // no charges block without charges
  });

  it('prints a purchase with the vendor and the supplier invoice ref', () => {
    render(<BillPrintDocument kind="PURCHASE" shop={FULL_SHOP} detail={purchase} />);
    const body = bodyText();
    expect(body).toContain('Purchase Bill');
    expect(body).toContain('Acme Vendor');
    expect(text('print-supplier-ref')).toContain('INV-77');
    expect(text('print-line')).toContain('25.00'); // 100 / 4 cartons
    expect(text('print-line')).toContain('Notebook · Carton (10 units)'); // no base unit: "units"
    expect(text('print-party')).toContain('Vendor');
    expect(text('print-date')).toContain('21-11-2026');
  });
});

describe('printBillDocument', () => {
  it('renders into #print-root, calls window.print and cleans up on afterprint', async () => {
    const print = vi.spyOn(window, 'print').mockImplementation(() => {});
    let sizeAtPrint: string | null | undefined;
    print.mockImplementation(() => {
      sizeAtPrint = document.querySelector('[data-testid="print-doc"]')?.getAttribute('data-size');
    });
    await printBillDocument('SALE', FULL_SHOP, sale);
    expect(print).toHaveBeenCalledTimes(1);
    expect(sizeAtPrint).toBe('half'); // jsdom measures 0mm: a short bill
    const root = document.getElementById('print-root');
    expect(root).not.toBeNull();
    expect(root?.classList.contains('print-measure')).toBe(false);
    expect(root?.querySelector('[data-testid="print-cut"]')).not.toBeNull();
    expect(root?.textContent).toContain('SAL-1005');
    act(() => {
      window.dispatchEvent(new Event('afterprint'));
    });
    expect(document.getElementById('print-root')).toBeNull();
  });

  it('falls back to a timeout when afterprint never fires', async () => {
    vi.spyOn(window, 'print').mockImplementation(() => {});
    vi.useFakeTimers({ toFake: ['setTimeout'] });
    try {
      await printBillDocument('PURCHASE', FULL_SHOP, purchase);
      expect(document.getElementById('print-root')).not.toBeNull();
      act(() => {
        vi.advanceTimersByTime(61_000);
      });
      expect(document.getElementById('print-root')).toBeNull();
    } finally {
      vi.useRealTimers();
    }
  });
});

const guard = {
  target: 'SALE', id: 5, code: 'SAL-1005', canEdit: true, canDelete: true, canChangeParty: true, reasons: [],
  upfrontAmount: 0, appliedAmount: 0, removedCashAmount: 0, removedCreditAmount: 0, lotLimits: [], settlementsToMove: { count: 0, amount: 0 },
};

describe('BillDetailDrawer Print', () => {
  beforeEach(() => {
    useAppStore.setState({ shopProfile: { shopName: 'Sharma Stores' }, shopLoaded: true });
  });

  it('the Print button prints the loaded bill', async () => {
    const print = vi.spyOn(window, 'print').mockImplementation(() => {});
    installApi({ getSaleDetails: sale, getBillEditGuard: guard });
    render(<BillDetailDrawer kind="SALE" billId={5} onClose={() => {}} />);
    await waitFor(() => bodyText().includes('SAL-1005'));
    click(document.querySelector('[data-testid="bill-print"]')!);
    await waitFor(() => print.mock.calls.length === 1);
    expect(document.getElementById('print-root')?.textContent).toContain('Sharma Stores');
  });

  it('Ctrl+P prevents the browser default and prints (purchase too)', async () => {
    const print = vi.spyOn(window, 'print').mockImplementation(() => {});
    installApi({ getPurchaseDetails: purchase, getBillEditGuard: { ...guard, target: 'PURCHASE' } });
    render(<BillDetailDrawer kind="PURCHASE" billId={9} onClose={() => {}} />);
    await waitFor(() => bodyText().includes('PUR-1009'));
    const ev = new KeyboardEvent('keydown', { key: 'p', ctrlKey: true, bubbles: true, cancelable: true });
    act(() => {
      window.dispatchEvent(ev);
    });
    expect(ev.defaultPrevented).toBe(true);
    await waitFor(() => print.mock.calls.length === 1);
    expect(document.getElementById('print-root')?.textContent).toContain('INV-77');
  });

  it('Ctrl+P does nothing while the drawer is closed', async () => {
    const print = vi.spyOn(window, 'print').mockImplementation(() => {});
    installApi({});
    render(<BillDetailDrawer kind="SALE" billId={null} onClose={() => {}} />);
    const ev = new KeyboardEvent('keydown', { key: 'p', ctrlKey: true, bubbles: true, cancelable: true });
    act(() => {
      window.dispatchEvent(ev);
    });
    await flush(20);
    expect(ev.defaultPrevented).toBe(false);
    expect(print).not.toHaveBeenCalled();
  });
});

describe('SaleSummaryModal result screen', () => {
  const result = { saleId: 5, transactionCode: 'SAL-1005', totalAmount: 100, paidAmount: 100, balanceDue: 0, changeAmount: 0, paymentStatus: 'PAID' as const };

  async function toResult() {
    const api = installApi({ getSaleDetails: sale, previewSale: async (p: Parameters<typeof fakeBillPreview>[0]) => fakeBillPreview(p) });
    const bill = {
      transactionDate: '2026-11-20', partyId: null, paymentMode: 'CASH', notes: null, charges: [],
      items: [{ itemId: 1, bundleId: null, bundleQuantity: 1, unitPrice: 100 }],
    };
    render(
      <SaleSummaryModal
        isOpen
        onClose={() => {}}
        bill={bill}
        totals={fakeBillPreview(bill)}
        customer={null}
        onSubmit={async () => result}
        onDone={() => {}}
      />
    );
    await waitFor(() => !(document.querySelector('[data-testid="confirm-btn"]') as HTMLButtonElement).disabled);
    click(document.querySelector('[data-testid="confirm-btn"]')!);
    await waitFor(() => !!document.querySelector('[data-testid="sale-result"]'));
    return api;
  }

  beforeEach(() => {
    useAppStore.setState({ shopProfile: { shopName: 'Sharma Stores' }, shopLoaded: true });
  });

  it('Print bill loads the saved sale and prints it', async () => {
    const print = vi.spyOn(window, 'print').mockImplementation(() => {});
    const api = await toResult();
    click(buttonByText('Print bill'));
    await waitFor(() => print.mock.calls.length === 1);
    expect(api.getSaleDetails.mock.calls[0][0]).toBe(5);
    expect(document.getElementById('print-root')?.textContent).toContain('SAL-1005');
  });

  it('the P key prints from the result screen', async () => {
    const print = vi.spyOn(window, 'print').mockImplementation(() => {});
    await toResult();
    const ev = new KeyboardEvent('keydown', { key: 'p', bubbles: true, cancelable: true });
    act(() => {
      window.dispatchEvent(ev);
    });
    await waitFor(() => print.mock.calls.length === 1);
    expect(ev.defaultPrevented).toBe(true);
  });
});
