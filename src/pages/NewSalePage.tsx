import React, { useState, useEffect, useRef, useCallback } from 'react';
import { CheckCircle, Trash2 } from 'lucide-react';
import { api } from '@/api';
import { todayLocal } from '@/lib/dates';
import { useCartStore } from '@/store/useCartStore';
import { useAppStore } from '@/store/useAppStore';
import { useHotkeyAction, useHotkey } from '@/hooks/useHotkeys';
import type { CreateSalePayload, ItemBundle, ItemSearchResult, SaleSummaryResponse } from '@/types';
import { CartTable } from '@/components/domain/CartTable';
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

export const NewSalePage: React.FC = () => {
  const {
    items,
    selectedCustomer,
    subtotal,
    totalBaseUnits,
    totalItems,
    addItem,
    updateQuantity,
    removeItem,
    clearCart,
    setCustomer,
  } = useCartStore();
  const refreshMetrics = useAppStore((s) => s.refreshMetrics);

  const [invoiceCode, setInvoiceCode] = useState<string>('');
  const [invoiceDate, setInvoiceDate] = useState<string>(todayLocal());
  const [notes, setNotes] = useState('');
  const [isSummaryOpen, setIsSummaryOpen] = useState(false);
  const [isBundleModalOpen, setIsBundleModalOpen] = useState(false);
  const [bundleItem, setBundleItem] = useState<ItemSearchResult | null>(null);
  const [newBundle, setNewBundle] = useState<{ bundle: ItemBundle; seq: number } | null>(null);
  const [isClearConfirmOpen, setIsClearConfirmOpen] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const rowRef = useRef<RapidBillingRowHandle>(null);
  const bundleSeq = useRef(0);

  const loadCodePreview = useCallback(() => {
    api
      .getNextSaleCode()
      .then((code) => setInvoiceCode(code ?? ''))
      .catch(() => {});
  }, []);

  useEffect(() => {
    loadCodePreview();
  }, [loadCodePreview]);

  const focusSearchSoon = () => setTimeout(() => rowRef.current?.focusSearch(), 0);

  const openPayment = useCallback(() => {
    if (items.length === 0) {
      setError('Add at least one item before completing the sale');
      return;
    }
    setError(null);
    setIsSummaryOpen(true);
  }, [items.length]);

  // Ctrl+Enter (blocked by the registry while a modal is open or a submit is in flight).
  useHotkeyAction('completeBill', openPayment, { scope: 'page' });
  // Esc with nothing pending in the entry row: offer to discard the cart.
  useHotkey(
    'Escape',
    () => {
      if (items.length > 0) setIsClearConfirmOpen(true);
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
        currentStock: entry.item.currentStock,
      },
      entry.quantity,
      entry.bundle,
      entry.rate
    );
    setError(ok ? null : `Cannot add "${entry.item.name}": out of stock or insufficient quantity`);
    return ok;
  };

  const handleUpdateQuantity = (lineId: string, qty: number): boolean => {
    const ok = updateQuantity(lineId, qty);
    setError(ok ? null : 'Not enough stock for that quantity');
    return ok;
  };

  const handleSubmit = async (values: SalePaymentValues): Promise<SaleSummaryResponse> => {
    const payload: CreateSalePayload = {
      transactionDate: invoiceDate || todayLocal(),
      partyId: selectedCustomer?.id ?? null,
      paymentStatus: values.paymentStatus,
      paidAmount: values.cashReceived,
      paymentMode: 'CASH',
      charges: [],
      notes: notes.trim() || null,
      items: items.map((it) => ({
        itemId: it.itemId,
        bundleId: it.bundleId ?? null,
        bundleQuantity: it.bundleId != null ? it.quantity : it.baseQuantity,
        unitPrice: it.bundleId != null ? it.rate : it.rate / it.unitsPerBundle,
      })),
    };
    const res = await api.createSale(payload);
    void refreshMetrics();
    return res;
  };

  const handleDone = () => {
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
      {/* Top Invoice Header Bar */}
      <div className="flex flex-wrap items-center justify-between gap-3 p-3.5 rounded-lg bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 shadow-xs">
        <div className="flex items-center gap-3">
          <div className="flex items-center gap-1.5 px-3 py-1 rounded-md bg-blue-50 dark:bg-blue-950/60 border border-blue-200 dark:border-blue-800">
            <span className="text-[10px] uppercase font-bold text-blue-600 dark:text-blue-400">
              Next Invoice
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
        cart={items}
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
          onUpdateQuantity={handleUpdateQuantity}
          onRemoveItem={removeItem}
          onFocusSearch={() => rowRef.current?.focusSearch()}
        />
      </div>

      {/* Bottom Totals & Checkout Ribbon */}
      <div className="p-4 rounded-lg bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 shadow-xs flex flex-wrap items-center justify-between gap-4">
        <div className="flex items-center gap-6">
          <div className="flex flex-col">
            <span className="text-xs text-slate-500 font-medium">Subtotal</span>
            <div data-testid="cart-subtotal" className="text-2xl font-bold font-mono text-slate-900 dark:text-slate-100">
              <CurrencyDisplay amount={subtotal} size="xl" />
            </div>
          </div>

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
            Complete Sale
          </Button>
        </div>
      </div>

      {error && (
        <p role="alert" data-testid="page-error" className="text-xs text-rose-600 dark:text-rose-400 font-medium px-1">
          {error}
        </p>
      )}

      {/* Payment dialog: payment mode, then result mode */}
      <SaleSummaryModal
        isOpen={isSummaryOpen}
        onClose={() => {
          setIsSummaryOpen(false);
          focusSearchSoon();
        }}
        grandTotal={subtotal}
        customer={selectedCustomer}
        onCustomerChange={setCustomer}
        onSubmit={handleSubmit}
        onDone={handleDone}
        invoiceCode={invoiceCode}
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
