import React, { useState, useEffect, useRef, useCallback, useMemo } from 'react';
import { CheckCircle, Trash2 } from 'lucide-react';
import { api } from '@/api';
import { todayLocal } from '@/lib/dates';
import { useCartStore } from '@/store/useCartStore';
import { useAppStore } from '@/store/useAppStore';
import { useHotkeyAction, useHotkey } from '@/hooks/useHotkeys';
import type {
  BillEditGuard,
  BillPreview,
  CreateSalePayload,
  SavedBundle,
  ItemSearchResult,
  PaymentStatus,
  PreviewSalePayload,
  SaleSummaryResponse,
} from '@/types';
import { errorMessage } from '@/api';
import { takeBillEdit } from '@/pages/billEditHandoff';
import { saleLineToCart } from '@/lib/billEditDraft';
import { usePreview } from '@/hooks/usePreview';
import type { CartLine } from '@/store/useCartStore';
import { CartTable } from '@/components/domain/CartTable';
import { BillChargesPanel, type BillChargesPanelHandle } from '@/components/domain/BillChargesPanel';
import { toChargeInputs } from '@/lib/billMath';
import {
  RapidBillingRow,
  type BillingLineEntry,
  type RapidBillingRowHandle,
} from '@/components/domain/RapidBillingRow';
import { SaleSummaryModal, type SalePaymentValues } from '@/components/domain/SaleSummaryModal';
import { BundleCreationModal } from '@/components/domain/BundleCreationModal';
import { PartySelector } from '@/components/domain/PartySelector';
import { CurrencyDisplay } from '@/components/common/CurrencyDisplay';
import { ConfirmDialog } from '@/components/common/ConfirmDialog';
import { Button } from '@/components/ui/Button';

/** Cart lines as the command's item lines: a pure mapping (loose lines are sent in base units). */
function toSaleItems(items: CartLine[]): CreateSalePayload['items'] {
  return items.map((it) => ({
    itemId: it.itemId,
    bundleId: it.bundleId ?? null,
    bundleQuantity: it.bundleId != null ? it.quantity : it.quantity * it.unitsPerBundle,
    unitPrice: it.bundleId != null ? it.rate : it.rate / it.unitsPerBundle,
  }));
}

export const NewSalePage: React.FC = () => {
  const {
    items,
    selectedCustomer,
    charges,
    totalBaseUnits,
    totalItems,
    addItem,
    updateQuantity,
    removeItem,
    addCharge,
    updateChargeAmount,
    removeCharge,
    clearCart,
    setCustomer,
    editing,
    loadForEdit,
  } = useCartStore();
  const setActiveRoute = useAppStore((s) => s.setActiveRoute);
  const refreshMetrics = useAppStore((s) => s.refreshMetrics);

  const [invoiceCode, setInvoiceCode] = useState<string>('');
  const [invoiceDate, setInvoiceDate] = useState<string>(todayLocal());
  const [notes, setNotes] = useState('');
  const [isSummaryOpen, setIsSummaryOpen] = useState(false);
  const [isBundleModalOpen, setIsBundleModalOpen] = useState(false);
  const [bundleItem, setBundleItem] = useState<ItemSearchResult | null>(null);
  const [newBundle, setNewBundle] = useState<{ bundle: SavedBundle; seq: number } | null>(null);
  const [isClearConfirmOpen, setIsClearConfirmOpen] = useState(false);
  const [error, setError] = useState<string | null>(null);
  /** FR-20: the stored payment status and old upfront payment of the bill being edited (dialog prefill). */
  const [editPayment, setEditPayment] = useState<{ status: PaymentStatus; upfront: number } | null>(null);
  const [isCancelEditOpen, setIsCancelEditOpen] = useState(false);

  // DD-56: every figure shown before Save comes from the backend. The bill without a payment gives
  // the line totals, charges, total and the stock check; the payment dialog adds its own preview.
  const bill = useMemo<PreviewSalePayload | null>(
    () =>
      items.length === 0
        ? null
        : {
            transactionDate: invoiceDate || todayLocal(),
            partyId: selectedCustomer?.id ?? null,
            paymentMode: 'CASH',
            charges: toChargeInputs(charges),
            notes: notes.trim() || null,
            items: toSaleItems(items),
          },
    [items, charges, selectedCustomer, invoiceDate, notes]
  );
  const preview = usePreview<PreviewSalePayload, BillPreview>({
    input: bill,
    fetcher: (p) => api.previewSale(p, editing?.saleId ?? null),
  });
  const totals = preview.data;

  const rowRef = useRef<RapidBillingRowHandle>(null);
  const chargesRef = useRef<BillChargesPanelHandle>(null);
  const bundleSeq = useRef(0);

  const loadCodePreview = useCallback(() => {
    api
      .getNextSaleCode()
      .then((code) => setInvoiceCode(code ?? ''))
      .catch(() => {});
  }, []);

  useEffect(() => {
    // FR-20: a queued bill edit takes over the page; otherwise show the next-code preview.
    const req = takeBillEdit();
    if (req?.kind !== 'SALE') {
      if (!useCartStore.getState().editing) loadCodePreview();
      return;
    }
    let live = true;
    (async () => {
      try {
        const [detail, guard]: [Awaited<ReturnType<typeof api.getSaleDetails>>, BillEditGuard] = await Promise.all([
          api.getSaleDetails(req.id),
          api.getBillEditGuard('SALE', req.id),
        ]);
        if (!live) return;
        if (!guard.canEdit) {
          setError(guard.reasons.find((r) => r.blocksEdit)?.message ?? 'This bill cannot be edited');
          loadCodePreview();
          return;
        }
        const lines = [];
        for (const it of detail.items) {
          const found = (await api.searchItemsFts(it.saleItem.itemName, 8).catch(() => [])).find(
            (r) => r.itemId === it.saleItem.itemId
          );
          lines.push(saleLineToCart(it.saleItem, found ?? null));
        }
        if (!live) return;
        loadForEdit(
          lines,
          detail.party ? { id: detail.party.id, name: detail.party.name, phone: detail.party.phone } : null,
          { saleId: detail.sale.id, code: detail.sale.transactionCode },
          detail.charges.map((c) => ({ chargeTypeId: c.chargeTypeId, name: c.chargeName, effect: c.effect, amount: c.amount }))
        );
        setInvoiceCode(detail.sale.transactionCode);
        setInvoiceDate(detail.sale.transactionDate);
        setNotes(detail.sale.notes ?? '');
        setEditPayment({ status: detail.sale.paymentStatus, upfront: guard.upfrontAmount });
      } catch (e) {
        if (live) {
          setError(errorMessage(e));
          loadCodePreview();
        }
      }
    })();
    return () => {
      live = false;
    };
  }, [loadCodePreview, loadForEdit]);

  const focusSearchSoon = () => setTimeout(() => rowRef.current?.focusSearch(), 0);

  const openPayment = useCallback(() => {
    if (items.length === 0) {
      setError('Add at least one item before completing the sale');
      return;
    }
    // The backend preview says what is wrong (discounts above the bill, stock, ...). While it is
    // still pending the dialog opens and runs its own preview, which blocks saving the same way.
    if (!preview.isLoading && preview.errors.length > 0) {
      setError(preview.errors[0].message);
      return;
    }
    setError(null);
    setIsSummaryOpen(true);
  }, [items.length, preview.isLoading, preview.errors]);

  // Ctrl+Enter (blocked by the registry while a modal is open or a submit is in flight).
  useHotkeyAction('completeBill', openPayment, { scope: 'page' });
  // Alt+X: add a charge / discount line (FR-19).
  useHotkeyAction('addCharge', () => chargesRef.current?.open(), { scope: 'page', allowInInputs: true });
  // Esc with nothing pending in the entry row: offer to discard the cart.
  useHotkey(
    'Escape',
    () => {
      if (editing) setIsCancelEditOpen(true);
      else if (items.length > 0) setIsClearConfirmOpen(true);
    },
    { scope: 'page', allowInInputs: true }
  );

  const handleAddLine = (entry: BillingLineEntry): boolean => {
    const ok = addItem(
      {
        id: entry.item.itemId,
        name: entry.item.name,
        barcode: entry.item.barcode,
        baseUnit: entry.item.baseUnit,
        defaultSalePrice: entry.item.defaultSalePrice,
        mrp: entry.item.mrp,
      },
      entry.quantity,
      entry.bundle,
      entry.rate
    );
    setError(ok ? null : `Cannot add "${entry.item.name}": invalid quantity or rate`);
    return ok;
  };

  const handleUpdateQuantity = (lineId: string, qty: number): boolean => {
    const ok = updateQuantity(lineId, qty);
    setError(ok ? null : 'Enter a quantity greater than 0');
    return ok;
  };

  const handleSubmit = async (values: SalePaymentValues): Promise<SaleSummaryResponse> => {
    if (!bill) throw new Error('The cart is empty');
    const payload: CreateSalePayload = {
      ...bill,
      paymentStatus: values.paymentStatus,
      paidAmount: values.cashReceived,
    };
    const res = editing ? await api.updateSale(editing.saleId, payload) : await api.createSale(payload);
    void refreshMetrics();
    return res;
  };

  const handleDone = () => {
    if (editing) {
      clearCart();
      setNotes('');
      setError(null);
      setIsSummaryOpen(false);
      setActiveRoute('sales-history');
      return;
    }
    clearCart();
    setNotes('');
    setError(null);
    setInvoiceDate(todayLocal());
    setIsSummaryOpen(false);
    loadCodePreview();
    focusSearchSoon();
  };

  return (
    <div className="flex flex-col gap-4">
      {editing && (
        <div
          data-testid="edit-banner"
          className="flex items-center justify-between gap-3 px-3.5 py-2 rounded-lg bg-amber-50 dark:bg-amber-950/40 border border-amber-300 dark:border-amber-800 text-xs text-amber-900 dark:text-amber-200"
        >
          <span>
            Editing <span className="font-mono font-bold">{editing.code}</span> — saving replaces the original bill
          </span>
          <Button variant="outline" size="sm" data-testid="cancel-edit" onClick={() => setIsCancelEditOpen(true)}>
            Cancel edit
          </Button>
        </div>
      )}

      {/* Top Invoice Header Bar */}
      <div className="flex flex-wrap items-center justify-between gap-3 p-3.5 rounded-lg bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 shadow-xs">
        <div className="flex items-center gap-3">
          <div className="flex items-center gap-1.5 px-3 py-1 rounded-md bg-blue-50 dark:bg-blue-950/60 border border-blue-200 dark:border-blue-800">
            <span className="text-[10px] uppercase font-bold text-blue-600 dark:text-blue-400">
              {editing ? 'Invoice' : 'Next Invoice'}
            </span>
            <span data-testid="invoice-preview" className="text-sm font-bold font-mono text-blue-950 dark:text-blue-100">
              {invoiceCode || '—'}
            </span>
          </div>

          <div className="flex items-center gap-1 text-xs">
            <label htmlFor="sale-date" className="text-slate-400">
              Date:
            </label>
            <input
              id="sale-date"
              type="date"
              value={invoiceDate}
              onChange={(e) => setInvoiceDate(e.target.value)}
              className="bg-transparent font-mono font-medium text-slate-800 dark:text-slate-200 focus:outline-none"
            />
          </div>
        </div>

        <div className="w-72">
          <PartySelector
            partyType="CUSTOMER"
            selectedPartyId={selectedCustomer?.id ?? null}
            onSelectParty={(party) =>
              setCustomer(party ? { id: party.id, name: party.name, phone: party.phone } : null)
            }
            allowWalkIn
          />
        </div>
      </div>

      {/* Rapid Billing Entry Cockpit */}
      <RapidBillingRow
        ref={rowRef}
        autoFocus
        onAddLine={handleAddLine}
        onCompleteBill={openPayment}
        onOpenBundleModal={(item) => {
          setBundleItem(item);
          setIsBundleModalOpen(true);
        }}
        newBundle={newBundle}
      />

      {/* Cart Line Items Table */}
      <div className="flex-1 min-h-[220px]">
        <CartTable
          items={items}
          lineTotals={totals?.lineTotals}
          onUpdateQuantity={handleUpdateQuantity}
          onRemoveItem={removeItem}
          onFocusSearch={() => rowRef.current?.focusSearch()}
        />
      </div>

      <BillChargesPanel
        ref={chargesRef}
        side="SALE"
        lines={charges}
        onAdd={addCharge}
        onUpdateAmount={updateChargeAmount}
        onRemove={removeCharge}
        onDone={focusSearchSoon}
      />

      {/* Bottom Totals & Checkout Ribbon */}
      <div className="p-4 rounded-lg bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 shadow-xs flex flex-wrap items-center justify-between gap-4">
        <div className="flex items-center gap-6">
          <div className="flex flex-col">
            <span className="text-xs text-slate-500 font-medium">{charges.length > 0 ? 'Items Subtotal' : 'Subtotal'}</span>
            <div data-testid="cart-subtotal" className="text-2xl font-bold font-mono text-slate-900 dark:text-slate-100">
              <CurrencyDisplay amount={totals?.itemsSubtotal ?? 0} size="xl" />
            </div>
          </div>

          {charges.length > 0 && (
            <>
              <div className="flex flex-col text-xs font-mono">
                <span className="text-slate-500">Charges (+)</span>
                <span data-testid="cart-charges-total" className="text-emerald-700 dark:text-emerald-400">
                  <CurrencyDisplay amount={totals?.chargesTotal ?? 0} size="sm" />
                </span>
              </div>
              <div className="flex flex-col text-xs font-mono">
                <span className="text-slate-500">Discounts (−)</span>
                <span data-testid="cart-discounts-total" className="text-rose-700 dark:text-rose-400">
                  <CurrencyDisplay amount={totals?.discountsTotal ?? 0} size="sm" />
                </span>
              </div>
              <div className="flex flex-col">
                <span className="text-xs text-slate-500 font-medium">Grand Total</span>
                <div data-testid="cart-grand-total" className="text-2xl font-bold font-mono text-slate-900 dark:text-slate-100">
                  <CurrencyDisplay amount={totals?.totalAmount ?? 0} size="xl" />
                </div>
              </div>
            </>
          )}

          <div className="h-8 w-px bg-slate-200 dark:bg-slate-800" />

          <div className="flex flex-col text-xs text-slate-500 font-mono">
            <span>Items: {totalItems}</span>
            <span>Total Units: {Number(totalBaseUnits.toFixed(3))}</span>
          </div>
        </div>

        <div className="flex-1 min-w-[160px] max-w-sm">
          <input
            type="text"
            aria-label="Notes"
            placeholder="Notes (optional)"
            value={notes}
            onChange={(e) => setNotes(e.target.value)}
            className="w-full rounded-md border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-900 text-slate-900 dark:text-slate-100 text-xs px-3 py-1.5 focus:outline-none focus:ring-2 focus:ring-blue-500"
          />
        </div>

        <div className="flex items-center gap-2">
          <Button
            variant="outline"
            size="md"
            disabled={items.length === 0}
            onClick={() => setIsClearConfirmOpen(true)}
            hotkey="Esc"
            leftIcon={<Trash2 className="h-4 w-4" />}
          >
            Clear
          </Button>

          <Button
            variant="primary"
            size="lg"
            data-testid="complete-sale-btn"
            disabled={items.length === 0}
            onClick={openPayment}
            hotkey="Ctrl+Enter"
            leftIcon={<CheckCircle className="h-5 w-5" />}
          >
            {editing ? 'Update Sale' : 'Complete Sale'}
          </Button>
        </div>
      </div>

      {error && (
        <p role="alert" data-testid="page-error" className="text-xs text-rose-600 dark:text-rose-400 font-medium px-1">
          {error}
        </p>
      )}
      {!preview.isLoading && preview.errors.length > 0 && (
        <ul data-testid="preview-errors" role="alert" className="text-xs text-rose-600 dark:text-rose-400 font-medium px-1">
          {preview.errors.map((e, i) => (
            <li key={i}>{e.message}</li>
          ))}
        </ul>
      )}

      {/* Payment dialog: payment mode, then result mode */}
      <SaleSummaryModal
        isOpen={isSummaryOpen}
        onClose={() => {
          setIsSummaryOpen(false);
          focusSearchSoon();
        }}
        bill={bill}
        totals={totals}
        saleId={editing?.saleId ?? null}
        charges={charges}
        customer={selectedCustomer}
        onCustomerChange={setCustomer}
        onSubmit={handleSubmit}
        onDone={handleDone}
        invoiceCode={invoiceCode}
        mode={editing ? 'edit' : 'create'}
        initialStatus={editPayment?.status}
        initialCash={editPayment?.upfront}
      />

      <ConfirmDialog
        isOpen={isCancelEditOpen}
        title="Discard changes?"
        message={`Leave the edit of ${editing?.code ?? 'this bill'} without saving. The original bill stays as it was.`}
        variant="warning"
        confirmText="Discard changes"
        cancelText="Keep editing"
        onConfirm={() => {
          setIsCancelEditOpen(false);
          clearCart();
          setNotes('');
          setError(null);
          setActiveRoute('sales-history');
        }}
        onCancel={() => setIsCancelEditOpen(false)}
      />

      <BundleCreationModal
        isOpen={isBundleModalOpen}
        onClose={() => setIsBundleModalOpen(false)}
        initialItemId={bundleItem?.itemId}
        baseUnit={bundleItem?.baseUnit}
        onBundleCreated={(bundle) => setNewBundle({ bundle, seq: ++bundleSeq.current })}
      />

      <ConfirmDialog
        isOpen={isClearConfirmOpen}
        title="Clear Current Cart?"
        message="Are you sure you want to discard all items currently in the active billing cart? This action cannot be undone."
        variant="danger"
        confirmText="Discard Cart"
        cancelText="Keep Items"
        onConfirm={() => {
          clearCart();
          setError(null);
          setIsClearConfirmOpen(false);
          focusSearchSoon();
        }}
        onCancel={() => {
          setIsClearConfirmOpen(false);
          focusSearchSoon();
        }}
      />
    </div>
  );
};

export { NewSalePage as BillingPage };
export default NewSalePage;
