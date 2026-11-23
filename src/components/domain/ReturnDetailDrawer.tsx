import React, { useEffect, useState } from 'react';
import { api, errorMessage } from '@/api';
import type { BillEditGuard, ReturnDetailView } from '@/types';
import { Drawer } from '../ui/Drawer';
import { Button } from '../ui/Button';
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '../ui/Table';
import { CurrencyDisplay } from '../common/CurrencyDisplay';
import { ConfirmDialog } from '../common/ConfirmDialog';
import { LedgerRows } from './BillDetailDrawer';

export interface ReturnDetailDrawerProps {
  /** null = closed. */
  returnId: number | null;
  onClose: () => void;
  /** Called after the return was deleted (callers reload their list). */
  onChanged?: () => void;
}

/** Return details with the FR-20 delete action (typed confirmation, guard reasons when blocked). */
export const ReturnDetailDrawer: React.FC<ReturnDetailDrawerProps> = ({ returnId, onClose, onChanged }) => {
  const [detail, setDetail] = useState<ReturnDetailView | null>(null);
  const [guard, setGuard] = useState<BillEditGuard | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [confirming, setConfirming] = useState(false);
  const [actionError, setActionError] = useState<string | null>(null);

  useEffect(() => {
    if (returnId === null) return;
    let live = true;
    setDetail(null);
    setGuard(null);
    setError(null);
    setActionError(null);
    setConfirming(false);
    api
      .getReturnDetails(returnId)
      .then((d) => live && setDetail(d))
      .catch((e) => live && setError(errorMessage(e)));
    api
      .getBillEditGuard('RETURN', returnId)
      .then((g) => live && setGuard(g))
      .catch(() => {});
    return () => {
      live = false;
    };
  }, [returnId]);

  const doDelete = async () => {
    if (returnId === null) return;
    setActionError(null);
    try {
      await api.deleteReturn(returnId);
      setConfirming(false);
      onChanged?.();
      onClose();
    } catch (e) {
      setActionError(errorMessage(e));
    }
  };

  const head = detail?.returnTx ?? null;

  return (
    <Drawer
      isOpen={returnId !== null}
      onClose={onClose}
      title={head ? `Return ${head.returnCode}` : 'Return details'}
      width="xl"
      footer={
        head && (
          <div className="flex w-full flex-col gap-2" data-testid="return-actions">
            {guard && guard.reasons.length > 0 && (
              <ul className="text-[11px] text-amber-700 dark:text-amber-400 list-disc pl-4" data-testid="guard-reasons">
                {guard.reasons.map((r) => (
                  <li key={r.code}>{r.message}</li>
                ))}
              </ul>
            )}
            <div className="flex justify-end">
              <Button
                variant="danger"
                size="sm"
                data-testid="return-delete"
                disabled={!guard?.canDelete}
                onClick={() => setConfirming(true)}
              >
                Delete return
              </Button>
            </div>
          </div>
        )
      }
    >
      {error && (
        <p role="alert" className="text-xs text-rose-600 dark:text-rose-400">
          {error}
        </p>
      )}
      {!detail && !error && <p className="text-xs text-slate-500">Loading...</p>}
      {head && detail && (
        <div className="flex flex-col gap-4 text-xs">
          <div className="grid grid-cols-2 gap-2 bg-slate-50 dark:bg-slate-800 p-3 rounded-md">
            <div>
              <span className="text-slate-500">{head.returnType === 'CUSTOMER' ? 'Customer: ' : 'Vendor: '}</span>
              <span className="font-medium">{head.partyName ?? '-'}</span>
            </div>
            <div>
              <span className="text-slate-500">Date: </span>
              <span className="font-mono">{head.returnDate}</span>
            </div>
            <div>
              <span className="text-slate-500">Settlement: </span>
              {head.settlementType}
            </div>
            <div>
              <span className="text-slate-500">Value: </span>
              <CurrencyDisplay amount={head.totalAmount} size="sm" />
            </div>
          </div>

          <Table dense scroll="page">
            <TableHeader>
              <TableRow className="text-left text-slate-400">
                <TableHead className="py-1">Item</TableHead>
                <TableHead>Kind</TableHead>
                <TableHead className="text-right">Qty</TableHead>
                <TableHead className="text-right">Value</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {detail.items.map((it) => (
                <TableRow key={it.returnItem.id}>
                  <TableCell className="py-1 font-medium">{it.itemName}</TableCell>
                  <TableCell>
                    {it.returnItem.lineType === 'REPLACEMENT_ISSUED' ? 'Replacement issued' : (it.returnItem.condition ?? 'Returned')}
                  </TableCell>
                  <TableCell className="text-right font-mono">{it.returnItem.quantity}</TableCell>
                  <TableCell className="text-right">
                    <CurrencyDisplay amount={it.returnItem.totalAmount} size="sm" />
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>

          <div>
            <h3 className="font-semibold mb-1">Ledger</h3>
            <LedgerRows rows={detail.ledger} />
          </div>
        </div>
      )}
      {head && (
        <ConfirmDialog
          isOpen={confirming}
          variant="danger"
          title={`Delete ${head.returnCode}?`}
          message={`This reverses the stock and ledger effects of the return. ₹${(guard?.removedCashAmount ?? 0).toFixed(2)} cash ${head.returnType === 'CUSTOMER' ? 'refunded' : 'received'} will be removed from cash records and ₹${(guard?.removedCreditAmount ?? 0).toFixed(2)} of credit applied to bills will be taken back.`}
          confirmText="Delete return"
          requireText={head.returnCode}
          onConfirm={doDelete}
          onCancel={() => {
            setConfirming(false);
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
    </Drawer>
  );
};

export default ReturnDetailDrawer;
