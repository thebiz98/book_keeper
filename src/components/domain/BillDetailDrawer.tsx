import React, { useEffect, useState } from 'react';
import { api, errorMessage } from '@/api';
import type {
  PaymentSettlement,
  PurchaseDetailView,
  SaleDetailView,
  TransactionType,
} from '@/types';
import { Drawer } from '../ui/Drawer';
import { CurrencyDisplay } from '../common/CurrencyDisplay';
import { PaymentStatusBadge } from '../common/PaymentStatusBadge';

export interface BillDetailDrawerProps {
  kind: TransactionType;
  /** null = closed. */
  billId: number | null;
  onClose: () => void;
}

/** "Box of 12 x 2 (24 base)" for pack lines, plain base quantity otherwise. */
export function packDisplay(packLabel: string | null, packQuantity: number, baseQuantity: number): string {
  if (packLabel) return `${packLabel} × ${packQuantity} (${baseQuantity} base)`;
  return String(baseQuantity);
}

const KIND_LABEL: Record<string, string> = {
  UPFRONT: 'Upfront',
  SETTLEMENT: 'Settlement',
  RETURN_CREDIT: 'Return credit',
  REFUND_OUT: 'Refund paid',
  REFUND_IN: 'Refund received',
};

export const LedgerRows: React.FC<{ rows: PaymentSettlement[] }> = ({ rows }) =>
  rows.length === 0 ? (
    <p className="text-xs text-slate-500">No payments recorded.</p>
  ) : (
    <table className="w-full text-xs">
      <thead>
        <tr className="text-left text-slate-400">
          <th className="py-1">Date</th>
          <th>Kind</th>
          <th>Mode</th>
          <th className="text-right">Amount</th>
        </tr>
      </thead>
      <tbody>
        {rows.map((r) => (
          <tr key={r.id} className="border-t border-slate-100 dark:border-slate-800">
            <td className="py-1 font-mono">{r.settlementDate}</td>
            <td>{KIND_LABEL[r.kind] ?? r.kind}</td>
            <td>{r.paymentMode ?? '-'}</td>
            <td className="text-right">
              <CurrencyDisplay amount={r.amountPaid} size="sm" />
            </td>
          </tr>
        ))}
      </tbody>
    </table>
  );

export const BillDetailDrawer: React.FC<BillDetailDrawerProps> = ({ kind, billId, onClose }) => {
  const [sale, setSale] = useState<SaleDetailView | null>(null);
  const [purchase, setPurchase] = useState<PurchaseDetailView | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    if (billId === null) return;
    let live = true;
    setSale(null);
    setPurchase(null);
    setError(null);
    setLoading(true);
    const p =
      kind === 'SALE'
        ? api.getSaleDetails(billId).then((d) => live && setSale(d))
        : api.getPurchaseDetails(billId).then((d) => live && setPurchase(d));
    p.catch((e) => live && setError(errorMessage(e))).finally(() => live && setLoading(false));
    return () => {
      live = false;
    };
  }, [kind, billId]);

  const head = sale?.sale ?? purchase?.purchase ?? null;
  const supplierRef = purchase?.purchase.supplierInvoiceRef ?? null;

  return (
    <Drawer
      isOpen={billId !== null}
      onClose={onClose}
      title={head ? `${kind === 'SALE' ? 'Invoice' : 'Purchase'} ${head.transactionCode}` : 'Bill details'}
      width="xl"
    >
      {loading && <p className="text-xs text-slate-500">Loading...</p>}
      {error && (
        <p role="alert" className="text-xs text-rose-600 dark:text-rose-400">
          {error}
        </p>
      )}
      {head && (
        <div className="flex flex-col gap-4 text-xs">
          <div className="grid grid-cols-2 gap-2 bg-slate-50 dark:bg-slate-800 p-3 rounded-md">
            <div>
              <span className="text-slate-500">{kind === 'SALE' ? 'Customer: ' : 'Vendor: '}</span>
              <span className="font-medium">{head.partyName ?? (kind === 'SALE' ? 'Walk-in' : '-')}</span>
            </div>
            <div>
              <span className="text-slate-500">Date: </span>
              <span className="font-mono">{head.transactionDate}</span>
            </div>
            <div>
              <span className="text-slate-500">Status: </span>
              <PaymentStatusBadge status={head.paymentStatus} />
            </div>
            {kind === 'PURCHASE' && (
              <div>
                <span className="text-slate-500">Supplier invoice ref: </span>
                <span data-testid="supplier-ref" className="font-mono">
                  {supplierRef ?? '-'}
                </span>
              </div>
            )}
          </div>

          <table className="w-full">
            <thead>
              <tr className="text-left text-slate-400">
                <th className="py-1">Item</th>
                <th>Pack / Qty</th>
                <th className="text-right">{kind === 'SALE' ? 'Rate/unit' : 'Cost/unit'}</th>
                <th className="text-right">Total</th>
                <th className="text-right">{kind === 'SALE' ? 'Returned' : 'Remaining'}</th>
              </tr>
            </thead>
            <tbody>
              {sale?.items.map((it) => (
                <React.Fragment key={it.saleItem.id}>
                  <tr className="border-t border-slate-100 dark:border-slate-800">
                    <td className="py-1 font-medium">{it.itemName}</td>
                    <td>{packDisplay(it.saleItem.packLabel, it.saleItem.packQuantity, it.saleItem.quantity)}</td>
                    <td className="text-right">
                      <CurrencyDisplay amount={it.saleItem.unitPrice} size="sm" />
                    </td>
                    <td className="text-right">
                      <CurrencyDisplay amount={it.saleItem.totalAmount} size="sm" />
                    </td>
                    <td className="text-right font-mono">{it.returnedQuantity}</td>
                  </tr>
                  {it.allocations.length > 0 && (
                    <tr>
                      <td colSpan={5} className="pb-1.5 pl-3 text-[11px] text-slate-500">
                        FIFO cost:{' '}
                        {it.allocations.map((a, i) => (
                          <span key={a.id} className="font-mono">
                            {i > 0 && ', '}
                            {a.quantityDepleted} @ ₹{a.unitCost.toFixed(2)}
                          </span>
                        ))}
                      </td>
                    </tr>
                  )}
                </React.Fragment>
              ))}
              {purchase?.items.map((it) => (
                <tr key={it.purchaseItem.id} className="border-t border-slate-100 dark:border-slate-800">
                  <td className="py-1 font-medium">{it.itemName}</td>
                  <td>{packDisplay(it.purchaseItem.packLabel, it.purchaseItem.packQuantity, it.purchaseItem.quantity)}</td>
                  <td className="text-right">
                    <CurrencyDisplay amount={it.purchaseItem.unitCost} size="sm" />
                  </td>
                  <td className="text-right">
                    <CurrencyDisplay amount={it.purchaseItem.totalAmount} size="sm" />
                  </td>
                  <td className="text-right font-mono">{it.purchaseItem.remainingQuantity}</td>
                </tr>
              ))}
            </tbody>
          </table>

          <div className="flex flex-col items-end gap-0.5">
            <div>
              Total: <CurrencyDisplay amount={head.totalAmount} size="sm" />
            </div>
            <div>
              Paid: <CurrencyDisplay amount={head.paidAmount} size="sm" />
            </div>
            <div className="font-semibold">
              Balance: <CurrencyDisplay amount={head.balanceDue} size="sm" colored />
            </div>
          </div>

          <div>
            <h3 className="font-semibold mb-1">Ledger</h3>
            <LedgerRows rows={sale?.settlements ?? purchase?.settlements ?? []} />
          </div>
        </div>
      )}
    </Drawer>
  );
};

export default BillDetailDrawer;
