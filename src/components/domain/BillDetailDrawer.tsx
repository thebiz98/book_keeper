import React, { useEffect, useRef, useState } from 'react';
import { api, errorMessage } from '@/api';
import type {
  BillEditGuard,
  PaymentSettlement,
  PurchaseDetailView,
  SaleDetailView,
  TransactionType,
} from '@/types';
import { Drawer } from '../ui/Drawer';
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '../ui/Table';
import { CurrencyDisplay } from '../common/CurrencyDisplay';
import { PaymentStatusBadge } from '../common/PaymentStatusBadge';
import { ConfirmDialog } from '../common/ConfirmDialog';
import { Button } from '../ui/Button';
import { Pencil, Trash2, DollarSign, Printer } from 'lucide-react';
import { usePrintBill } from '../print/usePrintBill';
import { SettlementModal } from './SettlementModal';
import { SettlementEditModal } from './SettlementEditModal';
import { refreshAppMetrics } from './navParams';
import { PurchaseEditForm } from './billEdit/PurchaseEditForm';
import { SaleEditForm } from './billEdit/SaleEditForm';

export interface BillDetailDrawerProps {
  kind: TransactionType;
  /** null = closed. */
  billId: number | null;
  onClose: () => void;
  /** Called after the bill was deleted (callers reload their list). */
  onChanged?: () => void;
  /**
   * `edit` opens straight into edit mode once the guard says the bill is editable; otherwise the
   * drawer stays in view mode with the guard reasons visible (FR-34).
   */
  initialMode?: 'view' | 'edit';
  /** Sale view: adds Cost / Profit per line and the profit reconciliation footer (FR-34). */
  showProfit?: boolean;
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

export interface LedgerRowsProps {
  rows: PaymentSettlement[];
  /** Edit / delete icons show on SETTLEMENT rows only (D8). */
  onEditSettlement?: (row: PaymentSettlement) => void;
  onDeleteSettlement?: (row: PaymentSettlement) => void;
  /** Adds a "Bill" column (party ledger, where rows span many bills). */
  billCodeOf?: (row: PaymentSettlement) => string;
}

export const LedgerRows: React.FC<LedgerRowsProps> = ({ rows, onEditSettlement, onDeleteSettlement, billCodeOf }) => {
  if (rows.length === 0) return <p className="text-xs text-slate-500">No payments recorded.</p>;
  const hasActions = !!(onEditSettlement || onDeleteSettlement);
  return (
    <Table dense scroll="page">
      <TableHeader>
        <TableRow className="text-left text-slate-400">
          <TableHead className="py-1">Date</TableHead>
          {billCodeOf && <TableHead>Bill</TableHead>}
          <TableHead>Kind</TableHead>
          <TableHead>Mode</TableHead>
          <TableHead className="text-right">Amount</TableHead>
          {hasActions && <TableHead className="text-right w-16"></TableHead>}
        </TableRow>
      </TableHeader>
      <TableBody>
        {rows.map((r) => (
          <TableRow key={r.id}>
            <TableCell className="py-1 font-mono">{r.settlementDate}</TableCell>
            {billCodeOf && <TableCell className="font-mono">{billCodeOf(r)}</TableCell>}
            <TableCell>{KIND_LABEL[r.kind] ?? r.kind}</TableCell>
            <TableCell>{r.paymentMode ?? '-'}</TableCell>
            <TableCell className="text-right">
              <CurrencyDisplay amount={r.amountPaid} size="sm" />
            </TableCell>
            {hasActions && (
              <TableCell className="text-right">
                {r.kind === 'SETTLEMENT' && (
                  <span className="inline-flex gap-1">
                    {onEditSettlement && (
                      <button
                        type="button"
                        aria-label="Edit payment"
                        data-testid={`settlement-edit-${r.id}`}
                        className="p-1 rounded text-slate-500 hover:text-blue-600 hover:bg-slate-100 dark:hover:bg-slate-800"
                        onClick={() => onEditSettlement(r)}
                      >
                        <Pencil size={12} />
                      </button>
                    )}
                    {onDeleteSettlement && (
                      <button
                        type="button"
                        aria-label="Delete payment"
                        data-testid={`settlement-delete-${r.id}`}
                        className="p-1 rounded text-slate-500 hover:text-rose-600 hover:bg-slate-100 dark:hover:bg-slate-800"
                        onClick={() => onDeleteSettlement(r)}
                      >
                        <Trash2 size={12} />
                      </button>
                    )}
                  </span>
                )}
              </TableCell>
            )}
          </TableRow>
        ))}
      </TableBody>
    </Table>
  );
};

export const BillDetailDrawer: React.FC<BillDetailDrawerProps> = ({
  kind,
  billId,
  onClose,
  onChanged,
  initialMode = 'view',
  showProfit = false,
}) => {
  const [sale, setSale] = useState<SaleDetailView | null>(null);
  const [purchase, setPurchase] = useState<PurchaseDetailView | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [guard, setGuard] = useState<BillEditGuard | null>(null);
  const [confirmDelete, setConfirmDelete] = useState(false);
  const [actionError, setActionError] = useState<string | null>(null);
  const [reloadKey, setReloadKey] = useState(0);
  const [settleOpen, setSettleOpen] = useState(false);
  const [editingRow, setEditingRow] = useState<PaymentSettlement | null>(null);
  const [deletingRow, setDeletingRow] = useState<PaymentSettlement | null>(null);
  const [rowError, setRowError] = useState<string | null>(null);
  /** In-drawer edit (DD-57): the drawer body becomes a form; closing with unsaved changes asks first. */
  const [mode, setMode] = useState<'view' | 'edit'>('view');
  const [editDirty, setEditDirty] = useState(false);
  const [confirmLeaveEdit, setConfirmLeaveEdit] = useState<'cancel' | 'close' | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  // A different bill always opens in view mode (unless the caller asked for edit, below).
  const autoEditDone = useRef(false);
  useEffect(() => {
    setMode('view');
    setEditDirty(false);
    setConfirmLeaveEdit(null);
    setNotice(null);
    autoEditDone.current = false;
  }, [kind, billId]);
  const initialModeRef = useRef(initialMode);
  initialModeRef.current = initialMode;

  useEffect(() => {
    if (billId === null) return;
    let live = true;
    setSale(null);
    setPurchase(null);
    setError(null);
    setGuard(null);
    setActionError(null);
    setConfirmDelete(false);
    setSettleOpen(false);
    setEditingRow(null);
    setDeletingRow(null);
    setRowError(null);
    setLoading(true);
    api
      .getBillEditGuard(kind, billId)
      .then((g) => {
        if (!live) return;
        setGuard(g);
        // initialMode="edit": enter edit mode once, when the guard allows it; else stay in view mode
        // (the guard reasons are shown in the footer).
        if (initialModeRef.current === 'edit' && !autoEditDone.current) {
          autoEditDone.current = true;
          if (g.canEdit) setMode('edit');
        }
      })
      .catch(() => {}); // the actions simply stay disabled when the pre-check fails
    const p =
      kind === 'SALE'
        ? api.getSaleDetails(billId).then((d) => live && setSale(d))
        : api.getPurchaseDetails(billId).then((d) => live && setPurchase(d));
    p.catch((e) => live && setError(errorMessage(e))).finally(() => live && setLoading(false));
    return () => {
      live = false;
    };
  }, [kind, billId, reloadKey]);

  const onEditClick = () => {
    setNotice(null);
    setMode('edit');
  };
  const leaveEdit = () => {
    setMode('view');
    setEditDirty(false);
    setConfirmLeaveEdit(null);
  };
  const requestCancelEdit = () => (editDirty ? setConfirmLeaveEdit('cancel') : leaveEdit());
  /** X / Esc / overlay: while editing with unsaved changes ask first. */
  const requestClose = () => (mode === 'edit' && editDirty ? setConfirmLeaveEdit('close') : onClose());
  const onEditSaved = () => {
    leaveEdit();
    setNotice('Bill updated');
    afterMoneyChange();
  };
  const doDelete = async () => {
    if (billId === null) return;
    setActionError(null);
    try {
      if (kind === 'SALE') await api.deleteSale(billId);
      else await api.deletePurchase(billId);
      setConfirmDelete(false);
      onChanged?.();
      onClose();
    } catch (e) {
      setActionError(errorMessage(e));
    }
  };

  /** After a settle / edit / delete: reload this drawer, tell the caller, refresh dashboard metrics. */
  const afterMoneyChange = () => {
    setReloadKey((k) => k + 1);
    onChanged?.();
    refreshAppMetrics();
  };
  const doDeleteRow = async () => {
    if (!deletingRow) return;
    setRowError(null);
    try {
      await api.deleteSettlement(deletingRow.id);
      setDeletingRow(null);
      afterMoneyChange();
    } catch (e) {
      setRowError(errorMessage(e));
    }
  };

  const head = sale?.sale ?? purchase?.purchase ?? null;
  const settlements = sale?.settlements ?? purchase?.settlements ?? [];
  const canSettle = !!head && head.balanceDue > 0 && head.partyId !== null;
  const rowMax = (r: PaymentSettlement) => (head ? head.balanceDue + r.amountPaid : r.amountPaid);
  const charges = sale?.charges ?? purchase?.charges ?? [];
  const profitOn = kind === 'SALE' && showProfit && !!sale;
  const supplierRef = purchase?.purchase.supplierInvoiceRef ?? null;

  const { printDetail } = usePrintBill();
  const doPrint = () => {
    const detail = kind === 'SALE' ? sale : purchase;
    if (!detail) return;
    setActionError(null);
    printDetail(kind, detail).catch((e) => setActionError(errorMessage(e)));
  };
  const printRef = useRef(doPrint);
  printRef.current = doPrint;
  // Print is a view-mode action: in edit mode it would print the saved bill, not the edits.
  const isOpening = !!head?.isOpeningBalance;
  const printable = billId !== null && head !== null && mode === 'view' && !isOpening;
  useEffect(() => {
    if (!printable) return;
    const onKey = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && !e.altKey && !e.shiftKey && e.key.toLowerCase() === 'p') {
        e.preventDefault();
        if (!e.repeat) printRef.current();
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [printable]);

  return (
    <Drawer
      isOpen={billId !== null}
      onClose={requestClose}
      className={mode === 'edit' ? 'w-[900px]' : undefined}
      title={
        head
          ? isOpening
            ? `Opening balance ${head.transactionCode}`
            : `${kind === 'SALE' ? 'Invoice' : 'Purchase'} ${head.transactionCode}`
          : 'Bill details'
      }
      description={head?.updatedAt ? `Edited ${head.updatedAt.slice(0, 10)}` : undefined}
      width="xl"
      footer={
        head && mode === 'view' && (
          <div className="flex w-full flex-col gap-2" data-testid="bill-actions">
            {!isOpening && guard && guard.reasons.length > 0 && (
              <ul className="text-[11px] text-amber-700 dark:text-amber-400 list-disc pl-4" data-testid="guard-reasons">
                {guard.reasons.map((r) => (
                  <li key={r.code}>{r.message}</li>
                ))}
              </ul>
            )}
            <div className="flex justify-end gap-2">
              {!isOpening && (
                <Button
                  variant="outline"
                  size="sm"
                  data-testid="bill-print"
                  leftIcon={<Printer size={12} />}
                  onClick={doPrint}
                  title="Print (Ctrl+P)"
                >
                  Print
                </Button>
              )}
              {canSettle && (
                <Button
                  variant="secondary"
                  size="sm"
                  data-testid="bill-settle"
                  leftIcon={<DollarSign size={12} />}
                  onClick={() => setSettleOpen(true)}
                >
                  Settle
                </Button>
              )}
              {!isOpening && (
                <Button
                  variant="outline"
                  size="sm"
                  data-testid="bill-edit"
                  disabled={!guard?.canEdit}
                  onClick={onEditClick}
                >
                  Edit
                </Button>
              )}
              {!isOpening && (
                <Button
                  variant="danger"
                  size="sm"
                  data-testid="bill-delete"
                  disabled={!guard?.canDelete}
                  onClick={() => setConfirmDelete(true)}
                >
                  Delete
                </Button>
              )}
            </div>
          </div>
        )
      }
    >
      {mode === 'edit' && billId !== null && kind === 'PURCHASE' && (
        <PurchaseEditForm billId={billId} onCancel={requestCancelEdit} onSaved={onEditSaved} onDirtyChange={setEditDirty} />
      )}
      {mode === 'edit' && billId !== null && kind === 'SALE' && (
        <SaleEditForm billId={billId} onCancel={requestCancelEdit} onSaved={onEditSaved} onDirtyChange={setEditDirty} />
      )}
      {mode === 'view' && notice && (
        <p role="status" data-testid="bill-notice" className="mb-2 text-xs text-emerald-700 dark:text-emerald-400">
          {notice}
        </p>
      )}
      {loading && mode === 'view' && <p className="text-xs text-slate-500">Loading...</p>}
      {error && (
        <p role="alert" className="text-xs text-rose-600 dark:text-rose-400">
          {error}
        </p>
      )}
      {head && mode === 'view' && (
        <div className="flex flex-col gap-4 text-xs">
          <div className="grid grid-cols-2 gap-2 bg-slate-50 dark:bg-slate-800 p-3 rounded-md">
            <div>
              <span className="text-slate-500">{kind === 'SALE' ? 'Customer: ' : 'Vendor: '}</span>
              <span className="font-medium">{head.partyName ?? (kind === 'SALE' ? 'Walk-in' : '-')}</span>
            </div>
            <div>
              <span className="text-slate-500">{isOpening ? 'As of: ' : 'Date: '}</span>
              <span className="font-mono">{head.transactionDate}</span>
            </div>
            {isOpening && (
              <div className="col-span-2 text-slate-500" data-testid="opening-note">
                Opening balance: brought forward from before Book Keeper. Change it from the party record.
              </div>
            )}
            <div>
              <span className="text-slate-500">Status: </span>
              <PaymentStatusBadge status={head.paymentStatus} />
            </div>
            {kind === 'PURCHASE' && !isOpening && (
              <div>
                <span className="text-slate-500">Supplier invoice ref: </span>
                <span data-testid="supplier-ref" className="font-mono">
                  {supplierRef ?? '-'}
                </span>
              </div>
            )}
          </div>

          {!isOpening && (
          <Table dense scroll="page">
            <TableHeader>
              <TableRow className="text-left text-slate-400">
                <TableHead className="py-1">Item</TableHead>
                <TableHead>Pack / Qty</TableHead>
                <TableHead className="text-right">{kind === 'SALE' ? 'Rate/unit' : 'Purchase price/unit'}</TableHead>
                <TableHead className="text-right">Total</TableHead>
                <TableHead className="text-right">{kind === 'SALE' ? 'Returned' : 'Remaining'}</TableHead>
                {profitOn && <TableHead className="text-right">Cost</TableHead>}
                {profitOn && <TableHead className="text-right">Profit</TableHead>}
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
                    {profitOn && (
                      <TableCell className="text-right" data-testid="line-cost">
                        <CurrencyDisplay amount={it.costAmount} size="sm" />
                      </TableCell>
                    )}
                    {profitOn && (
                      <TableCell className="text-right" data-testid="line-profit">
                        <CurrencyDisplay amount={it.profit} size="sm" colored />
                      </TableCell>
                    )}
                  </TableRow>
                  {it.allocations.length > 0 && (
                    <TableRow>
                      <TableCell colSpan={profitOn ? 7 : 5}className="pb-1.5 pl-3 text-[11px] text-slate-500">
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
          )}

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

          {profitOn && sale && !isOpening && (
            <div className="flex flex-col items-end gap-0.5" data-testid="profit-footer">
              <div>
                Lines profit: <CurrencyDisplay amount={sale.profitSummary.linesProfit} size="sm" />
              </div>
              <div>
                + Charges: <CurrencyDisplay amount={sale.profitSummary.charges} size="sm" />
              </div>
              <div>
                − Returns / replacements: <CurrencyDisplay amount={Math.abs(sale.profitSummary.returnAdjustment)} size="sm" />
              </div>
              <div className="font-semibold" data-testid="profit-total">
                = Bill profit: <CurrencyDisplay amount={sale.profitSummary.total} size="sm" colored />
              </div>
            </div>
          )}

          <div>
            <h3 className="font-semibold mb-1">Ledger</h3>
            <LedgerRows
              rows={settlements}
              onEditSettlement={setEditingRow}
              onDeleteSettlement={(r) => {
                setRowError(null);
                setDeletingRow(r);
              }}
            />
          </div>
        </div>
      )}
      {head && (
        <ConfirmDialog
          isOpen={confirmDelete}
          variant="danger"
          title={`Delete ${head.transactionCode}?`}
          message={`This removes the bill (${kind === 'SALE' ? sale?.items.length : purchase?.items.length} lines, total ₹${head.totalAmount.toFixed(2)}) and restores stock. ₹${(guard?.removedCashAmount ?? 0).toFixed(2)} ${kind === 'SALE' ? 'cash received for this bill' : 'paid for this bill'} will be removed from ${kind === 'SALE' ? 'cash records' : 'cash out'}. Payments on other bills are unaffected.`}
          confirmText="Delete"
          requireText={head.transactionCode}
          onConfirm={doDelete}
          onCancel={() => {
            setConfirmDelete(false);
            setActionError(null);
          }}
        >
          {actionError && (
            <p role="alert" className="mt-2 text-xs text-rose-600 dark:text-rose-400">
              {actionError}
            </p>
          )}
        </ConfirmDialog>
      )}
      {head && canSettle && head.partyId !== null && settleOpen && (
        <SettlementModal
          isOpen
          onClose={() => setSettleOpen(false)}
          partyId={head.partyId}
          partyName={head.partyName ?? ''}
          totalDue={head.balanceDue}
          transactionType={kind}
          transactionId={head.id}
          billCode={head.transactionCode}
          onSettled={afterMoneyChange}
        />
      )}
      <SettlementEditModal
        settlement={editingRow}
        billCode={head?.transactionCode}
        maxAmount={editingRow ? rowMax(editingRow) : 0}
        onClose={() => setEditingRow(null)}
        onSaved={afterMoneyChange}
      />
      <ConfirmDialog
        isOpen={deletingRow !== null}
        variant="danger"
        title="Delete this payment?"
        message={
          deletingRow
            ? `Removes the payment of ₹${deletingRow.amountPaid.toFixed(2)} against ${head?.transactionCode ?? 'this bill'}. The bill balance goes back up and cash records drop by ₹${deletingRow.amountPaid.toFixed(2)}.`
            : ''
        }
        confirmText="Delete payment"
        onConfirm={doDeleteRow}
        onCancel={() => {
          setDeletingRow(null);
          setRowError(null);
        }}
      >
        {rowError && (
          <p role="alert" className="mt-2 text-xs text-rose-600 dark:text-rose-400">
            {rowError}
          </p>
        )}
      </ConfirmDialog>
      <ConfirmDialog
        isOpen={confirmLeaveEdit !== null}
        variant="warning"
        title="Discard changes?"
        message={`Leave the edit of ${head?.transactionCode ?? 'this bill'} without saving. The original bill stays as it was.`}
        confirmText="Discard changes"
        cancelText="Keep editing"
        onConfirm={() => {
          const closing = confirmLeaveEdit === 'close';
          leaveEdit();
          if (closing) onClose();
        }}
        onCancel={() => setConfirmLeaveEdit(null)}
      />
    </Drawer>
  );
};

export default BillDetailDrawer;
