import React from 'react';
import { formatINR, round2 } from '@/lib/money';
import type { PurchaseDetailView, SaleDetailView, TransactionType } from '@/types';
import { packDisplay } from '../domain/BillDetailDrawer';

/** The Shop Profile fields the header uses (store `ShopProfile` or backend `ShopSettings` both fit). */
export interface PrintShop {
  shopName: string;
  phone?: string | null;
  address?: string | null;
  taxId?: string | null;
}

export interface BillPrintDocumentProps {
  kind: TransactionType;
  shop: PrintShop;
  detail: SaleDetailView | PurchaseDetailView;
}

interface PrintLine {
  id: number;
  name: string;
  qty: string;
  rate: number;
  amount: number;
}

/** Rate per pack = total / pack quantity; loose lines use the base quantity. */
export function ratePerPack(total: number, packLabel: string | null, packQuantity: number, quantity: number): number {
  const divisor = packLabel && packQuantity > 0 ? packQuantity : quantity;
  return divisor > 0 ? round2(total / divisor) : 0;
}

function toLines(kind: TransactionType, detail: SaleDetailView | PurchaseDetailView): PrintLine[] {
  if (kind === 'SALE') {
    return (detail as SaleDetailView).items.map((it) => ({
      id: it.saleItem.id,
      name: it.itemName,
      qty: packDisplay(it.saleItem.packLabel, it.saleItem.packQuantity, it.saleItem.quantity),
      rate: ratePerPack(it.saleItem.totalAmount, it.saleItem.packLabel, it.saleItem.packQuantity, it.saleItem.quantity),
      amount: it.saleItem.totalAmount,
    }));
  }
  return (detail as PurchaseDetailView).items.map((it) => ({
    id: it.purchaseItem.id,
    name: it.itemName,
    qty: packDisplay(it.purchaseItem.packLabel, it.purchaseItem.packQuantity, it.purchaseItem.quantity),
    rate: ratePerPack(
      it.purchaseItem.totalAmount,
      it.purchaseItem.packLabel,
      it.purchaseItem.packQuantity,
      it.purchaseItem.quantity
    ),
    amount: it.purchaseItem.totalAmount,
  }));
}

const clean = (s: string | null | undefined): string => (s ?? '').trim();

/** Pure A4 bill layout (FR-27). All styling comes from the `.print-*` classes in index.css. */
export const BillPrintDocument: React.FC<BillPrintDocumentProps> = ({ kind, shop, detail }) => {
  const head = kind === 'SALE' ? (detail as SaleDetailView).sale : (detail as PurchaseDetailView).purchase;
  const supplierRef = kind === 'PURCHASE' ? clean((detail as PurchaseDetailView).purchase.supplierInvoiceRef) : '';
  const lines = toLines(kind, detail);
  const charges = detail.charges;
  const party = clean(head.partyName) || (kind === 'SALE' ? 'Walk-in' : '-');
  const notes = clean(head.notes);

  return (
    <div className="print-doc" data-testid="print-doc">
      <header className="print-shop">
        {clean(shop.shopName) && <h1 data-testid="print-shop-name">{clean(shop.shopName)}</h1>}
        {clean(shop.address) && <div data-testid="print-shop-address">{clean(shop.address)}</div>}
        {clean(shop.phone) && <div data-testid="print-shop-phone">Phone: {clean(shop.phone)}</div>}
        {clean(shop.taxId) && <div data-testid="print-shop-taxid">GSTIN: {clean(shop.taxId)}</div>}
      </header>

      <h2 className="print-title">{kind === 'SALE' ? 'Invoice' : 'Purchase Bill'}</h2>
      <div className="print-meta">
        <div>
          <b>{kind === 'SALE' ? 'Invoice' : 'Purchase'} no:</b> {head.transactionCode}
        </div>
        <div>
          <b>Date:</b> {head.transactionDate}
        </div>
        <div>
          <b>{kind === 'SALE' ? 'Customer' : 'Vendor'}:</b> {party}
        </div>
        {kind === 'PURCHASE' && supplierRef && (
          <div data-testid="print-supplier-ref">
            <b>Supplier invoice ref:</b> {supplierRef}
          </div>
        )}
      </div>

      <table className="print-lines">
        <thead>
          <tr>
            <th className="print-num">#</th>
            <th>Item</th>
            <th>Qty</th>
            <th className="print-right">Rate</th>
            <th className="print-right">Amount</th>
          </tr>
        </thead>
        <tbody>
          {lines.map((l, i) => (
            <tr key={l.id} data-testid="print-line">
              <td className="print-num">{i + 1}</td>
              <td>{l.name}</td>
              <td>{l.qty}</td>
              <td className="print-right">{formatINR(l.rate)}</td>
              <td className="print-right">{formatINR(l.amount)}</td>
            </tr>
          ))}
        </tbody>
      </table>

      <div className="print-totals" data-testid="print-totals">
        {charges.length > 0 && (
          <div>
            <span>Items subtotal</span>
            <span>{formatINR(head.itemsSubtotal)}</span>
          </div>
        )}
        {charges.map((c) => (
          <div key={c.id} data-testid="print-charge">
            <span>
              {c.effect === 'CHARGE' ? '+' : '−'} {c.chargeName}
            </span>
            <span>{formatINR(c.amount)}</span>
          </div>
        ))}
        <div className="print-grand">
          <span>Grand Total</span>
          <span>{formatINR(head.totalAmount)}</span>
        </div>
        <div>
          <span>Paid</span>
          <span>{formatINR(head.paidAmount)}</span>
        </div>
        {head.balanceDue > 0 && (
          <div data-testid="print-balance">
            <span>Balance due</span>
            <span>{formatINR(head.balanceDue)}</span>
          </div>
        )}
        <div>
          <span>Status</span>
          <span>{head.paymentStatus}</span>
        </div>
      </div>

      {notes && (
        <p className="print-notes" data-testid="print-notes">
          <b>Notes:</b> {notes}
        </p>
      )}
    </div>
  );
};

export default BillPrintDocument;
