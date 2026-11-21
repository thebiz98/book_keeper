import React, { useState, useEffect } from 'react';
import { CheckCircle, Trash2 } from 'lucide-react';
import { api } from '@/api';
import { useCartStore } from '@/store/useCartStore';
import { useAppStore } from '@/store/useAppStore';
import { ItemSearchTypeahead } from '@/components/domain/ItemSearchTypeahead';
import type { CreateSalePayload, ItemSearchResult } from '@/types';
import { CartTable } from '@/components/domain/CartTable';
import { SaleSummaryModal } from '@/components/domain/SaleSummaryModal';
import { BundleCreationModal } from '@/components/domain/BundleCreationModal';
import { PartySelector } from '@/components/domain/PartySelector';
import { CurrencyDisplay } from '@/components/common/CurrencyDisplay';
import { ConfirmDialog } from '@/components/common/ConfirmDialog';
import { Button } from '@/components/ui/Button';

export const NewSalePage: React.FC = () => {
  const {
    items,
    selectedCustomer,
    paymentStatus,
    paidAmount,
    balanceDue,
    subtotal,
    totalBaseUnits,
    totalItems,
    addItem,
    updateQuantity,
    removeItem,
    clearCart,
    setCustomer,
    setPaymentStatus,
    setPaidAmount,
  } = useCartStore();

  const customerId = selectedCustomer?.id ?? null;
  const customerName = selectedCustomer?.name ?? null;
  const recordCashFlow = useAppStore((s) => s.recordCashFlow);

  const [invoiceCode, setInvoiceCode] = useState<string>('SAL-1001');
  const [invoiceDate, setInvoiceDate] = useState<string>(
    new Date().toISOString().split('T')[0]
  );
  const [cashTendered, setCashTendered] = useState<number>(0);
  const [isSummaryOpen, setIsSummaryOpen] = useState<boolean>(false);
  const [isBundleModalOpen, setIsBundleModalOpen] = useState<boolean>(false);
  const [isClearConfirmOpen, setIsClearConfirmOpen] = useState<boolean>(false);
  const [isSubmitting, setIsSubmitting] = useState<boolean>(false);
  const [error, setError] = useState<string | null>(null);

  // Fetch next sale code on mount
  useEffect(() => {
    api
      .getNextSaleCode()
      .then((code) => {
        if (code) setInvoiceCode(code);
      })
      .catch(() => {});
  }, []);

  // Update default cash tendered when subtotal changes
  useEffect(() => {
    if (paymentStatus === 'PAID') {
      setPaidAmount(subtotal);
      setCashTendered(subtotal);
    }
  }, [subtotal, paymentStatus, setPaidAmount]);

  // Global hotkeys (Ctrl+Enter to complete, Esc to clear)
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key === 'Enter') {
        e.preventDefault();
        if (items.length > 0) {
          setIsSummaryOpen(true);
        }
      } else if (e.key === 'Escape' && !isSummaryOpen && !isBundleModalOpen) {
        if (items.length > 0) {
          e.preventDefault();
          setIsClearConfirmOpen(true);
        }
      }
    };
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [items.length, isSummaryOpen, isBundleModalOpen]);

  const handleSelectItem = (item: ItemSearchResult) => {
    const ok = addItem({ ...item, id: item.itemId }, 1);
    if (!ok) {
      setError(`Cannot add "${item.name}": out of stock or insufficient quantity`);
    } else {
      setError(null);
    }
  };

  const handleCompleteSale = async () => {
    if (items.length === 0) {
      setError('Cannot checkout with an empty cart');
      return;
    }

    setIsSubmitting(true);
    setError(null);

    const payload: CreateSalePayload = {
      transactionCode: invoiceCode,
      transactionDate: invoiceDate,
      partyId: customerId,
      paidAmount: paymentStatus === 'UNPAID' ? 0 : paidAmount,
      paymentMode: 'CASH',
      notes: null,
      items: items.map((it) => ({
        itemId: it.itemId,
        bundleId: it.bundleId ?? null,
        bundleQuantity: it.bundleId ? it.quantity : it.baseQuantity,
        unitPrice: it.bundleId ? it.rate : it.rate / it.unitsPerBundle,
      })),
    };

    try {
      await api.createSale(payload);

      // Record cash inflow into cash register
      const actualPaid = paymentStatus === 'UNPAID' ? 0 : paidAmount;
      if (actualPaid > 0) {
        recordCashFlow({ salesCash: actualPaid });
      }

      // Clear cart & advance to next sale
      clearCart();
      setIsSummaryOpen(false);

      // Fetch next code
      const nextCode = await api.getNextSaleCode();
      if (nextCode) setInvoiceCode(nextCode);
    } catch (err: any) {
      setError(err?.message || 'Failed to complete sale');
    } finally {
      setIsSubmitting(false);
    }
  };

  return (
    <div className="flex flex-col gap-4">
      {/* Top Invoice Header Bar */}
      <div className="flex flex-wrap items-center justify-between gap-3 p-3.5 rounded-lg bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 shadow-xs">
        <div className="flex items-center gap-3">
          <div className="flex items-center gap-1.5 px-3 py-1 rounded-md bg-blue-50 dark:bg-blue-950/60 border border-blue-200 dark:border-blue-800">
            <span className="text-[10px] uppercase font-bold text-blue-600 dark:text-blue-400">
              Invoice
            </span>
            <span className="text-sm font-bold font-mono text-blue-950 dark:text-blue-100">
              {invoiceCode}
            </span>
          </div>

          <div className="flex items-center gap-1 text-xs">
            <label className="text-slate-400">Date:</label>
            <input
              type="date"
              value={invoiceDate}
              onChange={(e) => setInvoiceDate(e.target.value)}
              className="bg-transparent font-mono font-medium text-slate-800 dark:text-slate-200 focus:outline-none"
            />
          </div>
        </div>

        {/* Customer Selector */}
        <div className="w-72">
          <PartySelector
            partyType="CUSTOMER"
            selectedPartyId={customerId}
            onSelectParty={(party) => {
              if (party) {
                setCustomer({ id: party.id, name: party.name, phone: party.phone });
              } else {
                setCustomer(null);
              }
            }}
            allowWalkIn
          />
        </div>
      </div>

      {/* Rapid Billing Entry Cockpit */}
      <div className="flex items-center gap-2 p-3 rounded-lg bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 shadow-xs">
        <ItemSearchTypeahead onSelectItem={handleSelectItem} autoFocus className="flex-1" />
        <Button variant="outline" size="md" onClick={() => setIsBundleModalOpen(true)} hotkey="Alt+B">
          New Bundle
        </Button>
      </div>

      {/* Cart Line Items Table */}
      <div className="flex-1 min-h-[220px]">
        <CartTable
          items={items}
          onUpdateQuantity={updateQuantity}
          onRemoveItem={removeItem}
        />
      </div>

      {/* Bottom Payment & Checkout Ribbon */}
      <div className="p-4 rounded-lg bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 shadow-xs flex flex-wrap items-center justify-between gap-4">
        {/* Left: Summary totals */}
        <div className="flex items-center gap-6">
          <div className="flex flex-col">
            <span className="text-xs text-slate-500 font-medium">Subtotal</span>
            <div className="text-2xl font-bold font-mono text-slate-900 dark:text-slate-100">
              <CurrencyDisplay amount={subtotal} size="xl" />
            </div>
          </div>

          <div className="h-8 w-px bg-slate-200 dark:bg-slate-800" />

          <div className="flex flex-col text-xs text-slate-500 font-mono">
            <span>Items: {totalItems}</span>
            <span>Total Units: {totalBaseUnits} PCS</span>
          </div>
        </div>

        {/* Middle: Payment Status Toggle */}
        <div className="flex items-center gap-3">
          <div className="flex p-0.5 rounded-lg border border-slate-200 dark:border-slate-700 bg-slate-100 dark:bg-slate-800 text-xs">
            {(['PAID', 'PARTIAL', 'UNPAID'] as const).map((status) => (
              <button
                key={status}
                type="button"
                onClick={() => setPaymentStatus(status)}
                className={`px-3 py-1.5 rounded-md font-semibold transition-colors cursor-pointer ${
                  paymentStatus === status
                    ? 'bg-white dark:bg-slate-900 text-blue-600 dark:text-blue-400 shadow-2xs'
                    : 'text-slate-600 dark:text-slate-400 hover:text-slate-900'
                }`}
              >
                {status}
              </button>
            ))}
          </div>

          {paymentStatus === 'PARTIAL' && (
            <div className="flex items-center gap-1.5 bg-slate-50 dark:bg-slate-800 px-2 py-1 rounded border border-slate-200 dark:border-slate-700 text-xs">
              <span className="text-slate-400">Paid: ₹</span>
              <input
                type="number"
                value={paidAmount}
                onChange={(e) => setPaidAmount(parseFloat(e.target.value) || 0)}
                className="w-20 bg-transparent font-mono font-bold text-right focus:outline-none"
              />
            </div>
          )}

          {balanceDue > 0 && (
            <span className="text-xs font-mono font-bold text-rose-600 dark:text-rose-400">
              Due: <CurrencyDisplay amount={balanceDue} size="sm" />
            </span>
          )}
        </div>

        {/* Right: Actions */}
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
            disabled={items.length === 0 || isSubmitting}
            onClick={() => setIsSummaryOpen(true)}
            hotkey="Ctrl+Enter"
            leftIcon={<CheckCircle className="h-5 w-5" />}
          >
            Complete Sale
          </Button>
        </div>
      </div>

      {error && (
        <p role="alert" className="text-xs text-rose-600 dark:text-rose-400 font-medium px-1">
          {error}
        </p>
      )}

      {/* Sale Summary Checkout Modal */}
      <SaleSummaryModal
        isOpen={isSummaryOpen}
        onClose={() => setIsSummaryOpen(false)}
        onConfirm={handleCompleteSale}
        grandTotal={subtotal}
        invoiceCode={invoiceCode}
        customerName={customerName || 'Walk-in Customer'}
        defaultCashReceived={cashTendered || subtotal}
      />

      {/* Bundle Creation Modal (Alt+B) */}
      <BundleCreationModal
        isOpen={isBundleModalOpen}
        onClose={() => setIsBundleModalOpen(false)}
      />

      {/* Clear Cart Confirmation Dialog */}
      <ConfirmDialog
        isOpen={isClearConfirmOpen}
        title="Clear Current Cart?"
        message="Are you sure you want to discard all items currently in the active billing cart? This action cannot be undone."
        variant="danger"
        confirmText="Discard Cart"
        cancelText="Keep Items"
        onConfirm={() => {
          clearCart();
          setIsClearConfirmOpen(false);
        }}
        onCancel={() => setIsClearConfirmOpen(false)}
      />
    </div>
  );
};

export { NewSalePage as BillingPage };
export default NewSalePage;
