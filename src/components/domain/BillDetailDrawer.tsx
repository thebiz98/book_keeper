import React, { useEffect, useState } from 'react';
import { api, errorMessage } from '@/api';
import type {
  PaymentSettlement,
  PurchaseDetailView,
  SaleDetailView,
  TransactionType,
} from '@/types';
import { Drawer } from '../ui/Drawer';
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '../ui/Table';
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
    <Table dense scroll="page">
      <TableHeader>
        <TableRow className="text-left text-slate-400">
          <TableHead className="py-1">Date</TableHead>
          <TableHead>Kind</TableHead>
          <TableHead>Mode</TableHead>
          <TableHead className="text-right">Amount</TableHead>
        </TableRow>
      </TableHeader>
      <TableBody>
        {rows.map((r) => (
          <TableRow key={r.id} >
            <TableCell className="py-1 font-mono">{r.settlementDate}</TableCell>
            <TableCell>{KIND_LABEL[r.kind] ?? r.kind}</TableCell>
            <TableCell>{r.paymentMode ?? '-'}</TableCell>
            <TableCell className="text-right">
              <CurrencyDisplay amount={r.amountPaid} size="sm" />
            </TableCell>
          </TableRow>
        ))}
      </TableBody>
    </Table>
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
  const charges = sale?.charges ?? purchase?.charges ?? [];
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

          <Table dense scroll="page">
            <TableHeader>
              <TableRow className="text-left text-slate-400">
                <TableHead className="py-1">Item</TableHead>
                <TableHead>Pack / Qty</TableHead>
                <TableHead className="text-right">{kind === 'SALE' ? 'Rate/unit' : 'Cost/unit'}</TableHead>
                <TableHead className="text-right">Total</TableHead>
                <TableHead className="text-right">{kind === 'SALE' ? 'Returned' : 'Remaining'}</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {sale?.items.map((it) => (
                <React.Fragment key={it.saleItem.id}>
                  <TableRow >
                    <TableCell className="py-1 font-medium">{it.itemName}</TableCell>
                    <TableCell>{packDisplay(it.saleItem.packLabel, it.saleItem.packQuantity, it.saleItem.quantity)}</TableCell>
                    <TableCell className="text-right">
                      <CurrencyDisplay amount={it.saleItem.unitPrice} size="sm" />
                    </TableCell>
                    <TableCell className="text-right">
                      <CurrencyDisplay amount={it.saleItem.totalAmount} size="sm" />
                    </TableCell>
                    <TableCell className="text-right font-mono">{it.returnedQuantity}</TableCell>
                  </TableRow>
                  {it.allocations.length > 0 && (
                    <TableRow>
                      <TableCell colSpan={5} className="pb-1.5 pl-3 text-[11px] text-slate-500">
                        FIFO cost:{' '}
                        {it.allocations.map((a, i) => (
                          <span key={a.id} className="font-mono">
                            {i > 0 && ', '}
                            {a.quantityDepleted} @ ₹{a.unitCost.toFixed(2)}
                          </span>
                        ))}
                      </TableCell>
                    </TableRow>
                  )}
                </React.Fragment>
              ))}
              {purchase?.items.map((it) => (
                <TableRow key={it.purchaseItem.id} >
                  <TableCell className="py-1 font-medium">{it.itemName}</TableCell>
                  <TableCell>{packDisplay(it.purchaseItem.packLabel, it.purchaseItem.packQuantity, it.purchaseItem.quantity)}</TableCell>
                  <TableCell className="text-right">
                    <CurrencyDisplay amount={it.purchaseItem.unitCost} size="sm" />
                  </TableCell>
                  <TableCell className="text-right">
                    <CurrencyDisplay amount={it.purchaseItem.totalAmount} size="sm" />
                  </TableCell>
                  <TableCell className="text-right font-mono">{it.purchaseItem.remainingQuantity}</TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>

          {charges.length > 0 && (
            <div data-testid="bill-charges">
              <h3 className="font-semibold mb-1">Charges &amp; discounts</h3>
              <Table dense scroll="none">
                <TableBody>
                  {charges.map((c) => (
                    <TableRow key={c.id}>
                      <TableCell className="py-1 font-medium">{c.chargeName}</TableCell>
                      <TableCell>{c.effect === 'CHARGE' ? 'Charge' : 'Discount'}</TableCell>
                      <TableCell className="text-right font-mono">
                        {c.effect === 'CHARGE' ? '+' : '−'}
                        <CurrencyDisplay amount={c.amount} size="sm" />
                      </TableCell>
                    </TableRow>
                  ))}
                </TableBody>
              </Table>
            </div>
          )}

          <div className="flex flex-col items-end gap-0.5" data-testid="bill-totals-block">
            {(head.chargesTotal > 0 || head.discountsTotal > 0) && (
              <>
                <div data-testid="bill-items-subtotal">
                  Items subtotal: <CurrencyDisplay amount={head.itemsSubtotal} size="sm" />
                </div>
                {head.chargesTotal > 0 && (
                  <div data-testid="bill-charges-total">
                    + Charges: <CurrencyDisplay amount={head.chargesTotal} size="sm" />
                  </div>
                )}
                {head.discountsTotal > 0 && (
                  <div data-testid="bill-discounts-total">
                    − Discounts: <CurrencyDisplay amount={head.discountsTotal} size="sm" />
                  </div>
                )}
              </>
            )}
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
