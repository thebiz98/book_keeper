import { describe, it, expect, beforeEach, vi } from 'vitest';
import { render } from '../testUtils';
import { installApi, flush, waitFor, click, setInput, buttonByText, bodyText, fakeBillPreview } from '../pages/helpers';
import { BillDetailDrawer } from '@/components/domain/BillDetailDrawer';
import { DeletedReferenceAlert } from '@/components/common/DeletedReferenceAlert';
import { saleDetail, saleGuard, search } from './fixtures';
import type { PreviewError } from '@/types';

(globalThis as unknown as { IS_REACT_ACT_ENVIRONMENT: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

vi.mock('@/components/domain/ItemSearchTypeahead', () => ({ ItemSearchTypeahead: () => <div /> }));
// The party picker is a stub that picks customer 7 'Ravi'.
vi.mock('@/components/domain/PartySelector', () => ({
  PartySelector: ({ onSelectParty }: { onSelectParty: (p: unknown) => void }) => (
    <button type="button" data-testid="stub-party" onClick={() => onSelectParty({ id: 7, name: 'Ravi', partyType: 'CUSTOMER' })}>
      party
    </button>
  ),
}));

const q = (sel: string) => document.querySelector(sel) as HTMLInputElement;
const settle = async () => {
  for (let i = 0; i < 4; i++) await flush(120);
};

type PreviewIn = Parameters<typeof fakeBillPreview>[0] & { partyId: number | null; confirmMoveSettlements?: boolean };

function mount(preview: (p: PreviewIn) => unknown, extra: Record<string, unknown> = {}) {
  const api = installApi({
    getSaleDetails: saleDetail(),
    getBillEditGuard: saleGuard({ settlementsToMove: { count: 2, amount: 1500 } }),
    searchItemsFts: search,
    previewSale: preview,
    updateSale: { saleId: 5, transactionCode: 'SAL-1005', totalAmount: 0, paidAmount: 0, balanceDue: 0, changeAmount: 0, paymentStatus: 'PAID' },
    getDashboardMetrics: { cashInHand: 0 },
    ...extra,
  });
  render(<BillDetailDrawer kind="SALE" billId={5} onClose={() => {}} />);
  return api;
}

async function openEdit() {
  await waitFor(() => !!document.querySelector('[data-testid="bill-edit"]:not([disabled])'));
  click(q('[data-testid="bill-edit"]'));
  await waitFor(() => document.querySelectorAll('[data-testid="edit-line"]').length > 0);
  await settle();
}

const MOVE: PreviewError = {
  field: 'SETTLEMENTS_ON_BILL',
  message: "This bill has 2 later payments (₹1500.00). Confirm moving them to 'Ravi'.",
};

describe('bill edit party change moves payments only after a confirm (DD-88)', () => {
  beforeEach(() => {
    document.body.innerHTML = '';
  });

  // Test double of the backend: a party change without the flag reports SETTLEMENTS_ON_BILL.
  const preview = async (p: PreviewIn) =>
    fakeBillPreview(p, { errors: p.partyId === 7 && !p.confirmMoveSettlements ? [MOVE] : [] });

  it('shows an info line, opens the dialog with the guard figures, and Cancel keeps the form', async () => {
    const api = mount(preview);
    await openEdit();
    click(q('[data-testid="stub-party"]'));
    await settle();
    expect(q('[data-testid="edit-move-info"]').textContent).toBe('Saving will ask to move 2 payments');
    expect(q('[data-testid="edit-save"]').disabled).toBe(false);
    expect(document.querySelector('[data-testid="edit-errors"]')).toBeNull();

    click(q('[data-testid="edit-save"]'));
    await flush();
    expect(bodyText()).toContain("Move 2 earlier payments (₹1,500.00) to 'Ravi'?");
    expect(api.updateSale).not.toHaveBeenCalled();
    click(buttonByText('Cancel', buttonByText('Move and save').parentElement!));
    await flush();
    expect(bodyText()).not.toContain('Move 2 earlier payments');
    expect(api.updateSale).not.toHaveBeenCalled();
    expect(document.querySelector('[data-testid="sale-edit-form"]')).not.toBeNull();
  });

  it('Move and save re-sends the edit with confirmMoveSettlements', async () => {
    const api = mount(preview);
    await openEdit();
    click(q('[data-testid="stub-party"]'));
    await settle();
    click(q('[data-testid="edit-save"]'));
    await flush();
    click(buttonByText('Move and save'));
    await settle();
    expect(api.updateSale).toHaveBeenCalledTimes(1);
    const [id, payload] = api.updateSale.mock.calls[0];
    expect(id).toBe(5);
    expect(payload).toMatchObject({ partyId: 7, confirmMoveSettlements: true });
    expect(document.querySelector('[data-testid="sale-edit-form"]')).toBeNull();
  });

  it('a SETTLEMENTS_ON_BILL refusal on save (no preview notice) also opens the dialog', async () => {
    const api = mount(async (p: PreviewIn) => fakeBillPreview(p), {
      updateSale: vi.fn(async (_id: number, p: { confirmMoveSettlements?: boolean }) => {
        if (!p.confirmMoveSettlements) throw { type: 'BillLocked', details: { code: 'SETTLEMENTS_ON_BILL', message: MOVE.message } };
        return { saleId: 5 };
      }),
    });
    await openEdit();
    click(q('[data-testid="stub-party"]'));
    await settle();
    click(q('[data-testid="edit-save"]'));
    await settle();
    expect(bodyText()).toContain("Move 2 earlier payments (₹1,500.00) to 'Ravi'?");
    click(buttonByText('Move and save'));
    await settle();
    expect(api.updateSale).toHaveBeenCalledTimes(2);
    expect(api.updateSale.mock.calls[1][1].confirmMoveSettlements).toBe(true);
  });
});

describe('DeletedReference with Restore (DD-90)', () => {
  beforeEach(() => {
    document.body.innerHTML = '';
  });

  it('restores the party, re-runs the preview and keeps the draft', async () => {
    let restored = false;
    const DELETED: PreviewError = {
      field: 'party_id',
      message: "'Gone' is deleted. Restore it first.",
      deleted: { entity: 'PARTY', id: 3, name: 'Gone' },
    };
    const api = mount(async (p: PreviewIn) => fakeBillPreview(p, { errors: restored ? [] : [DELETED] }), {
      restoreParty: vi.fn(async () => {
        restored = true;
      }),
    });
    await openEdit();
    setInput(q('input[aria-label="Quantity Pen"]'), '4');
    await settle();
    expect(q('[data-testid="deleted-ref-alert"]').textContent).toContain("'Gone' is deleted. Restore it first.");
    expect(q('[data-testid="edit-save"]').disabled).toBe(true);
    expect(document.querySelector('[data-testid="edit-errors"]')).toBeNull();
    const calls = api.previewSale.mock.calls.length;

    click(q('[data-testid="deleted-ref-restore"]'));
    await settle();
    expect(api.restoreParty).toHaveBeenCalledWith(3);
    expect(api.previewSale.mock.calls.length).toBeGreaterThan(calls);
    expect(document.querySelector('[data-testid="deleted-ref-alert"]')).toBeNull();
    expect(q('input[aria-label="Quantity Pen"]').value).toBe('4');
    expect(q('[data-testid="edit-save"]').disabled).toBe(false);
  });

  it('the alert restores an item through restoreItem and reports a failure', async () => {
    const api = installApi({
      restoreItem: vi.fn(async () => {
        throw { type: 'ValidationError', details: { field: 'item_id', message: 'Already active' } };
      }),
    });
    const onRestored = vi.fn();
    render(
      <DeletedReferenceAlert
        deleted={{ entity: 'ITEM', id: 9, name: 'Pen' }}
        message="'Pen' is deleted. Restore it first."
        onRestored={onRestored}
      />
    );
    expect(q('[data-testid="deleted-ref-restore"]').textContent).toContain("Restore 'Pen'");
    click(q('[data-testid="deleted-ref-restore"]'));
    await flush(20);
    expect(api.restoreItem).toHaveBeenCalledWith(9);
    expect(onRestored).not.toHaveBeenCalled();
    expect(bodyText()).toContain('Already active');
  });
});
