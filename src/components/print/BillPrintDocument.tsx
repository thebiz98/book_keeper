import React from 'react';
import { formatINR, round2 } from '@/lib/money';
import type { PurchaseDetailView, SaleDetailView, TransactionType } from '@/types';

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
  packLabel: string | null;
  qty: string;
  rate: number;
  amount: number;
}

/** Rate per pack = total / pack quantity; loose lines use the base quantity. */
export function ratePerPack(total: number, packLabel: string | null, packQuantity: number, quantity: number): number {
  const divisor = packLabel && packQuantity > 0 ? packQuantity : quantity;
  return divisor > 0 ? round2(total / divisor) : 0;
}

/** Qty in the line's own unit: the pack count for a pack line, the base quantity otherwise. */
function packQty(packLabel: string | null, packQuantity: number, quantity: number): string {
  return String(packLabel ? packQuantity : quantity);
}

function toLines(kind: TransactionType, detail: SaleDetailView | PurchaseDetailView): PrintLine[] {
  if (kind === 'SALE') {
    return (detail as SaleDetailView).items.map((it) => ({
      id: it.saleItem.id,
      name: it.itemName,
      packLabel: it.saleItem.packLabel,
      qty: packQty(it.saleItem.packLabel, it.saleItem.packQuantity, it.saleItem.quantity),
      rate: ratePerPack(it.saleItem.totalAmount, it.saleItem.packLabel, it.saleItem.packQuantity, it.saleItem.quantity),
      amount: it.saleItem.totalAmount,
    }));
  }
  return (detail as PurchaseDetailView).items.map((it) => ({
    id: it.purchaseItem.id,
    name: it.itemName,
    packLabel: it.purchaseItem.packLabel,
    qty: packQty(it.purchaseItem.packLabel, it.purchaseItem.packQuantity, it.purchaseItem.quantity),
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

/** `2026-11-20` -> `20-11-2026` (display only). */
const displayDate = (iso: string): string => {
  const m = /^(\d{4})-(\d{2})-(\d{2})/.exec(iso);
  return m ? `${m[3]}-${m[2]}-${m[1]}` : iso;
};

/** Pure A4 bill layout (FR-27, FR-43). All styling comes from the `.print-*` classes in index.css. */
export const BillPrintDocument: React.FC<BillPrintDocumentProps> = ({ kind, shop, detail }) => {
  const head = kind === 'SALE' ? (detail as SaleDetailView).sale : (detail as PurchaseDetailView).purchase;
  const supplierRef = kind === 'PURCHASE' ? clean((detail as PurchaseDetailView).purchase.supplierInvoiceRef) : '';
  const lines = toLines(kind, detail);
  const charges = detail.charges;
  const party = clean(head.partyName) || (kind === 'SALE' ? 'Walk-in' : '-');
  const partyPhone = clean(detail.party?.phone);
  const notes = clean(head.notes);

  return (
    <div className="print-doc" data-testid="print-doc">
      <header className="print-shop">
        {clean(shop.shopName) && <h1 data-testid="print-shop-name">{clean(shop.shopName)}</h1>}
        {clean(shop.address) && <div data-testid="print-shop-address">{clean(shop.address)}</div>}
        {clean(shop.phone) && <div data-testid="print-shop-phone">Phone: {clean(shop.phone)}</div>}
        {clean(shop.taxId) && <div data-testid="print-shop-taxid">GSTIN: {clean(shop.taxId)}</div>}
      </header>
      <div className="print-caption">{kind === 'SALE' ? 'Bill' : 'Purchase Bill'}</div>

      <div className="print-parties">
        <div className="print-party-left" data-testid="print-party">
          <b>{party}</b>
          {partyPhone && <div data-testid="print-party-phone">{partyPhone}</div>}
        </div>
        <div className="print-party-right" data-testid="print-bill-meta">
          <div>Date: {displayDate(head.transactionDate)}</div>
          <div>Bill No. {head.transactionCode}</div>
          {kind === 'PURCHASE' && supplierRef && <div data-testid="print-supplier-ref">Supplier Inv: {supplierRef}</div>}
        </div>
      </div>

      <div className="print-items">
        <table className="print-lines">
          <thead>
            <tr>
              <th className="print-num">Sl. No</th>
              <th>Item</th>
              <th className="print-right">Qty</th>
              <th className="print-right">Price</th>
              <th className="print-right">Amount</th>
            </tr>
          </thead>
          <tbody>
            {lines.map((l, i) => (
              <tr key={l.id} data-testid="print-line">
                <td className="print-num">{i + 1}</td>
                <td>{l.packLabel ? `${l.name} — ${l.packLabel}` : l.name}</td>
                <td className="print-right">{l.qty}</td>
                <td className="print-right">{formatINR(l.rate)}</td>
                <td className="print-right">{formatINR(l.amount)}</td>
              </tr>
            ))}
            <tr className="print-spacer" data-testid="print-spacer" aria-hidden="true">
              <td colSpan={5} />
            </tr>
          </tbody>
          <tbody className="print-totals" data-testid="print-totals">
            <tr className="print-subtotal" data-testid="print-subtotal">
              <td />
              <td>Items subtotal</td>
              <td />
              <td />
              <td className="print-right">{formatINR(head.itemsSubtotal)}</td>
            </tr>
            {charges.map((c) => (
              <tr key={c.id} data-testid="print-charge">
                <td />
                <td>{c.effect === 'CHARGE' ? c.chargeName : `Less: ${c.chargeName}`}</td>
                <td />
                <td />
                <td className="print-right">{c.effect === 'CHARGE' ? formatINR(c.amount) : `−${formatINR(c.amount)}`}</td>
              </tr>
            ))}
            <tr className="print-total" data-testid="print-total">
              <td />
              <td>Total</td>
              <td />
              <td />
              <td className="print-right">{formatINR(head.totalAmount)}</td>
            </tr>
          </tbody>
        </table>
      </div>

      <div className="print-footer">
        <div>
          Paid: {formatINR(head.paidAmount)}
          {head.balanceDue > 0 && (
            <span data-testid="print-balance"> · Balance due: {formatINR(head.balanceDue)}</span>
          )}
          <span> · {head.paymentStatus}</span>
        </div>
        {notes && (
          <p className="print-notes" data-testid="print-notes">
            <b>Notes:</b> {notes}
          </p>
        )}
      </div>
    </div>
  );
};

export default BillPrintDocument;