import React from 'react';
import { amountInWords, formatINR, round2 } from '@/lib/money';
import type { PurchaseDetailView, SaleDetailView, TransactionType } from '@/types';

/** The Shop Profile fields the header uses (store `ShopProfile` or backend `ShopSettings` both fit). */
export interface PrintShop {
  shopName: string;
  phone?: string | null;
  address?: string | null;
  taxId?: string | null;
}

/** How much paper the bill takes: the top half of an A4, a full A4, or several pages. */
export type PrintSize = 'half' | 'full' | 'multi';

export interface BillPrintDocumentProps {
  kind: TransactionType;
  shop: PrintShop;
  detail: SaleDetailView | PurchaseDetailView;
  /** Set by the print flow after measuring; absent while measuring (natural height). */
  size?: PrintSize;
}

interface PrintLine {
  id: number;
  name: string;
  /** "Box of 12 (12 pcs)" for a bundle line, null for a base-unit line. */
  bundle: string | null;
  qty: string;
  rate: number;
  amount: number;
}

/** Rate per pack = total / pack quantity; loose lines use the base quantity. */
export function ratePerPack(total: number, packLabel: string | null, packQuantity: number, quantity: number): number {
  const divisor = packLabel && packQuantity > 0 ? packQuantity : quantity;
  return divisor > 0 ? round2(total / divisor) : 0;
}

/** A quantity with up to 3 decimals and no trailing zeros. */
const fmtQty = (n: number): string => String(Math.round(n * 1000) / 1000);

/** "Box of 12 (12 pcs)": the bundle name and the base units one bundle holds (backend values, DD-56). */
export function bundleText(
  label: string,
  unitsPerBundle: number | null | undefined,
  baseUnit: string | null | undefined,
): string {
  if (unitsPerBundle == null || !(unitsPerBundle > 0)) return label;
  return `${label} (${fmtQty(unitsPerBundle)} ${clean(baseUnit) || 'units'})`;
}

interface RawLine {
  id: number;
  name: string;
  packLabel: string | null;
  packQuantity: number;
  quantity: number;
  totalAmount: number;
  unitsPerBundle?: number | null;
  baseUnit?: string | null;
}

function toPrintLine(l: RawLine): PrintLine {
  const unit = clean(l.baseUnit);
  return {
    id: l.id,
    name: l.name,
    bundle: l.packLabel ? bundleText(l.packLabel, l.unitsPerBundle, l.baseUnit) : null,
    // A bundle line counts bundles (named in the Item cell); a loose line counts base units.
    qty: l.packLabel ? fmtQty(l.packQuantity) : [fmtQty(l.quantity), unit].filter(Boolean).join(' '),
    rate: ratePerPack(l.totalAmount, l.packLabel, l.packQuantity, l.quantity),
    amount: l.totalAmount,
  };
}

function toLines(kind: TransactionType, detail: SaleDetailView | PurchaseDetailView): PrintLine[] {
  if (kind === 'SALE') {
    return (detail as SaleDetailView).items.map((it) =>
      toPrintLine({
        ...it.saleItem,
        name: it.itemName,
        unitsPerBundle: it.unitsPerBundle,
        baseUnit: it.baseUnit,
      }),
    );
  }
  return (detail as PurchaseDetailView).items.map((it) =>
    toPrintLine({
      ...it.purchaseItem,
      name: it.itemName,
      unitsPerBundle: it.unitsPerBundle,
      baseUnit: it.baseUnit,
    }),
  );
}

const clean = (s: string | null | undefined): string => (s ?? '').trim();

/** `2026-11-20` -> `20-11-2026` (display only). */
const displayDate = (iso: string): string => {
  const m = /^(\d{4})-(\d{2})-(\d{2})/.exec(iso);
  return m ? `${m[3]}-${m[2]}-${m[1]}` : iso;
};

/** Shared column widths: the item, filler and totals tables line up as one ruled grid. */
const Cols: React.FC = () => (
  <colgroup>
    <col className="print-col-num" />
    <col />
    <col className="print-col-qty" />
    <col className="print-col-rate" />
    <col className="print-col-amount" />
  </colgroup>
);

/**
 * Bill layout (FR-27, FR-43) after the owner's sketch: shop band, customer / caption / bill no.,
 * a column-ruled item table, item subtotal, numbered charges, the total in figures and words, and
 * a blank manual footer with a signatory line. Styling lives in the `.print-*` rules in index.css.
 *
 * The grid is three tables with the same columns: items (its header repeats on every page), an
 * empty ruled filler that takes the spare height of a half / full sheet, and the totals. Only the
 * filler grows, so item and charge rows keep their natural height.
 */
export const BillPrintDocument: React.FC<BillPrintDocumentProps> = ({ kind, shop, detail, size }) => {
  const head = kind === 'SALE' ? (detail as SaleDetailView).sale : (detail as PurchaseDetailView).purchase;
  const supplierRef = kind === 'PURCHASE' ? clean((detail as PurchaseDetailView).purchase.supplierInvoiceRef) : '';
  const lines = toLines(kind, detail);
  const charges = detail.charges;
  const party = clean(head.partyName) || (kind === 'SALE' ? 'Walk-in' : '-');
  const partyPhone = clean(detail.party?.phone);
  const notes = clean(head.notes);
  const shopName = clean(shop.shopName);
  const phone = clean(shop.phone);
  const taxId = clean(shop.taxId);

  return (
    <>
      <div className="print-doc" data-testid="print-doc" data-size={size}>
        <header className="print-shop">
          <div />
          <div className="print-shop-main">
            {shopName && <h1 data-testid="print-shop-name">{shopName}</h1>}
            {clean(shop.address) && <div data-testid="print-shop-address">{clean(shop.address)}</div>}
            {(phone || taxId) && (
              <div className="print-shop-contact">
                {phone && <span data-testid="print-shop-phone">Ph: {phone}</span>}
                {phone && taxId && <span> · </span>}
                {taxId && <span data-testid="print-shop-taxid">GSTIN: {taxId}</span>}
              </div>
            )}
          </div>
          <div className="print-shop-date" data-testid="print-date">
            <span className="print-label">Date</span> {displayDate(head.transactionDate)}
          </div>
        </header>

        <div className="print-meta">
          <div className="print-party" data-testid="print-party">
            <span className="print-label">{kind === 'SALE' ? 'Customer' : 'Vendor'}</span> <b>{party}</b>
            {partyPhone && <div data-testid="print-party-phone">{partyPhone}</div>}
          </div>
          <div className="print-caption">{kind === 'SALE' ? 'Invoice / Bill' : 'Purchase Bill'}</div>
          <div className="print-bill-meta" data-testid="print-bill-meta">
            <div>
              <span className="print-label">Bill No.</span> <b>{head.transactionCode}</b>
            </div>
            {kind === 'PURCHASE' && supplierRef && (
              <div data-testid="print-supplier-ref">Supplier Inv: {supplierRef}</div>
            )}
          </div>
        </div>

        <div className="print-items">
          <table className="print-lines">
            <Cols />
            <thead>
              <tr>
                <th className="print-num">Sl.</th>
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
                  <td>
                    <span className="print-item-name">{l.name}</span>
                    {l.bundle && <span className="print-bundle"> · {l.bundle}</span>}
                  </td>
                  <td className="print-right">{l.qty}</td>
                  <td className="print-right">{formatINR(l.rate)}</td>
                  <td className="print-right">{formatINR(l.amount)}</td>
                </tr>
              ))}
            </tbody>
          </table>
          <table className="print-lines print-filler" data-testid="print-spacer" aria-hidden="true">
            <Cols />
            <tbody>
              <tr>
                <td className="print-num" />
                <td />
                <td />
                <td />
                <td />
              </tr>
            </tbody>
          </table>
          <table className="print-lines print-totals-table">
            <Cols />
            <tbody className="print-totals" data-testid="print-totals">
              <tr className="print-subtotal" data-testid="print-subtotal">
                <td className="print-num" />
                <td />
                <td colSpan={2} className="print-right">
                  Item subtotal
                </td>
                <td className="print-right">{formatINR(head.itemsSubtotal)}</td>
              </tr>
              {charges.length > 0 && (
                <tr className="print-charges-head" data-testid="print-charges-head">
                  <td className="print-num" />
                  <td>Charges / Discount</td>
                  <td colSpan={2} />
                  <td />
                </tr>
              )}
              {charges.map((c, i) => (
                <tr key={c.id} className="print-charge" data-testid="print-charge">
                  <td className="print-num">{i + 1}</td>
                  <td>{c.effect === 'CHARGE' ? c.chargeName : `Less: ${c.chargeName}`}</td>
                  <td colSpan={2} />
                  <td className="print-right">
                    {c.effect === 'CHARGE' ? formatINR(c.amount) : `−${formatINR(c.amount)}`}
                  </td>
                </tr>
              ))}
              <tr className="print-total" data-testid="print-total">
                <td colSpan={2} className="print-words" data-testid="print-words">
                  {amountInWords(head.totalAmount)}
                </td>
                <td colSpan={2} className="print-right">
                  Total
                </td>
                <td className="print-right">{formatINR(head.totalAmount)}</td>
              </tr>
            </tbody>
          </table>
        </div>

        <div className="print-footer print-manual-footer" data-testid="print-manual-footer">
          <div className="print-manual-space">
            <div className="print-summary">
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
          <div className="print-sign">
            {shopName && <div className="print-sign-for">For {shopName}</div>}
            <div className="print-sign-line">Authorised signatory</div>
          </div>
        </div>
      </div>
      {size === 'half' && <div className="print-cut" data-testid="print-cut" aria-hidden="true" />}
    </>
  );
};

export default BillPrintDocument;
