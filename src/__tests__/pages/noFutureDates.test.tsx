import { describe, it, expect, beforeEach, vi } from 'vitest';
import { render } from '../testUtils';
import { installApi, flush, waitFor, click, bodyText, fakeBillPreview } from './helpers';
import { __resetHotkeys } from '@/hooks/useHotkeys';
import { todayLocal } from '@/lib/dates';
import { BillDetailDrawer } from '@/components/domain/BillDetailDrawer';
import { ExpenseForm } from '@/components/domain/ExpenseForm';
import { NewSalePage } from '@/pages/NewSalePage';
import { NewPurchasePage } from '@/pages/NewPurchasePage';
import { useCartStore } from '@/store/useCartStore';
import { saleDetail, saleGuard, purchaseDetail, purchaseGuard, search } from '../billEdit/fixtures';

(globalThis as unknown as { IS_REACT_ACT_ENVIRONMENT: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

vi.mock('@/components/domain/ItemSearchTypeahead', () => ({
  ItemSearchTypeahead: () => <input data-testid="stub-search" />,
}));
vi.mock('@/components/domain/PartySelector', () => ({ PartySelector: () => <div data-testid="stub-party" /> }));

const dates = () => Array.from(document.querySelectorAll<HTMLInputElement>('input[type="date"]'));
const settle = async () => {
  for (let i = 0; i < 4; i++) await flush(120);
};

beforeEach(() => {
  document.body.innerHTML = '';
  __resetHotkeys();
  useCartStore.getState().clearCart();
});

describe('Entry date inputs stop at today (FR-28, DD-58)', () => {
  it('New Sale and New Purchase', async () => {
    installApi({
      createPurchase: vi.fn(),
      previewPurchase: async (p: Parameters<typeof fakeBillPreview>[0]) => fakeBillPreview(p),
      previewSale: async (p: Parameters<typeof fakeBillPreview>[0]) => fakeBillPreview(p),
      getDashboardMetrics: { cashInHand: 0 },
    });
    render(<NewSalePage />);
    await flush(150);
    expect(dates().map((d) => d.max)).toEqual([todayLocal()]);
    document.body.innerHTML = '';
    render(<NewPurchasePage />);
    await flush(150);
    expect(dates().map((d) => d.max)).toEqual([todayLocal()]);
  });

  it('Expense form', async () => {
    installApi({ searchExpenseTypes: [], listExpenseTypes: [], getNextExpenseCode: 'EXP-1' });
    render(<ExpenseForm editing={null} codeKey={0} onSaved={() => {}} onCancelEdit={() => {}} typeInputRef={{ current: null }} />);
    await flush(100);
    expect(dates().map((d) => d.max)).toEqual([todayLocal()]);
  });

  it.each([
    ['SALE', 5, { getSaleDetails: saleDetail(), getBillEditGuard: saleGuard(), previewSale: 'p' }],
    ['PURCHASE', 8, { getPurchaseDetails: purchaseDetail(), getBillEditGuard: purchaseGuard(), previewPurchase: 'p' }],
  ] as const)('%s edit drawer carries max and shows the backend date error', async (kind, id, api) => {
    const previewKey = kind === 'SALE' ? 'previewSale' : 'previewPurchase';
    installApi({
      ...api,
      [previewKey]: async (p: Parameters<typeof fakeBillPreview>[0]) =>
        fakeBillPreview(p, { errors: [{ field: 'transaction_date', message: 'Date cannot be in the future' }] }),
      searchItemsFts: search,
      getDashboardMetrics: { cashInHand: 0 },
    });
    render(<BillDetailDrawer kind={kind} billId={id} onClose={() => {}} />);
    await waitFor(() => !!document.querySelector('[data-testid="bill-edit"]:not([disabled])'));
    click(document.querySelector('[data-testid="bill-edit"]') as HTMLElement);
    await waitFor(() => document.querySelectorAll('[data-testid="edit-line"]').length > 0);
    await settle();
    expect(dates().map((d) => d.max)).toContain(todayLocal());
    expect(bodyText()).toContain('Date cannot be in the future');
  });
});
