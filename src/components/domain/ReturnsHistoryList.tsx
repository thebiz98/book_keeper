import React, { useCallback, useEffect, useState } from 'react';
import { api, errorMessage } from '@/api';
import type { AdjustmentUndoGuard, ReturnTransaction, ReturnType, StockAdjustment } from '@/types';
import { CurrencyDisplay } from '../common/CurrencyDisplay';
import { DataTable, type DataTableColumn } from './DataTable';
import { ADJUSTMENT_REASONS } from './StockAdjustmentForm';
import { ReturnDetailDrawer } from './ReturnDetailDrawer';
import { Button } from '../ui/Button';
import { ConfirmDialog } from '../common/ConfirmDialog';
import { refreshAppMetrics } from './navParams';
import { formatINR } from '@/lib/money';

const PAGE_SIZE = 10;

export const ReturnsHistoryList: React.FC<{ returnType: ReturnType; reloadKey: number }> = ({ returnType, reloadKey }) => {
  const [page, setPage] = useState(1);
  const [rows, setRows] = useState<ReturnTransaction[]>([]);
  const [total, setTotal] = useState(0);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [detailId, setDetailId] = useState<number | null>(null);

  const load = useCallback(() => {
    let live = true;
    setLoading(true);
    setError(null);
    api
      .listReturns({ returnType, page, limit: PAGE_SIZE })
      .then((r) => {
        if (!live) return;
        setRows(r.items);
        setTotal(r.total);
      })
      .catch((e) => live && setError(errorMessage(e)))
      .finally(() => live && setLoading(false));
    return () => {
      live = false;
    };
  }, [returnType, page, reloadKey]); // eslint-disable-line react-hooks/exhaustive-deps

  useEffect(() => load(), [load]);
  useEffect(() => setPage(1), [returnType]);

  const columns: DataTableColumn<ReturnTransaction>[] = [
    { key: 'code', header: 'Code', mono: true, render: (r) => r.returnCode },
    { key: 'date', header: 'Date', mono: true, render: (r) => r.returnDate },
    { key: 'party', header: returnType === 'CUSTOMER' ? 'Customer' : 'Vendor', render: (r) => r.partyName ?? '-' },
    { key: 'type', header: 'Settlement', render: (r) => r.settlementType },
    { key: 'total', header: 'Value', align: 'right', render: (r) => <CurrencyDisplay amount={r.totalAmount} /> },
    { key: 'reason', header: 'Reason', render: (r) => r.reason ?? '' },
  ];

  return (
    <>
      <DataTable
        columns={columns}
        items={rows}
        rowKey={(r) => r.id}
        isLoading={loading}
        error={error}
        onRetry={load}
        emptyMessage="No returns recorded yet."
        onRowClick={(r) => setDetailId(r.id)}
        total={total}
        page={page}
        limit={PAGE_SIZE}
        onPageChange={setPage}
      />
      <ReturnDetailDrawer returnId={detailId} onClose={() => setDetailId(null)} onChanged={load} />
    </>
  );
};

export const AdjustmentsHistoryList: React.FC<{ reloadKey: number }> = ({ reloadKey }) => {
  const [page, setPage] = useState(1);
  const [rows, setRows] = useState<StockAdjustment[]>([]);
  const [total, setTotal] = useState(0);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [undo, setUndo] = useState<{ row: StockAdjustment; guard: AdjustmentUndoGuard | null; error: string | null } | null>(null);

  const load = useCallback(() => {
    let live = true;
    setLoading(true);
    setError(null);
    api
      .listStockAdjustments(undefined, page, PAGE_SIZE)
      .then((r) => {
        if (!live) return;
        setRows(r.items);
        setTotal(r.total);
      })
      .catch((e) => live && setError(errorMessage(e)))
      .finally(() => live && setLoading(false));
    return () => {
      live = false;
    };
  }, [page, reloadKey]); // eslint-disable-line react-hooks/exhaustive-deps

  useEffect(() => load(), [load]);

  // Backend dry run (DD-56): the dialog only prints the guard's numbers or its reason.
  const openUndo = (row: StockAdjustment) => {
    setUndo({ row, guard: null, error: null });
    api
      .getAdjustmentUndoGuard(row.id)
      .then((guard) => setUndo((u) => (u && u.row.id === row.id ? { ...u, guard } : u)))
      .catch((e) => setUndo((u) => (u && u.row.id === row.id ? { ...u, error: errorMessage(e) } : u)));
  };

  const confirmUndo = async () => {
    if (!undo) return;
    try {
      await api.undoStockAdjustment(undo.row.id);
      setUndo(null);
      load();
      refreshAppMetrics();
    } catch (e) {
      setUndo((u) => (u ? { ...u, error: errorMessage(e) } : u));
    }
  };

  const undoMessage = (g: AdjustmentUndoGuard | null): string => {
    if (!g) return 'Checking what undoing this adjustment would change...';
    if (!g.canUndo) return g.reason?.message ?? 'This adjustment cannot be undone.';
    const qty = `${Math.abs(g.quantity)} ${g.baseUnit} of ${g.itemName}`;
    return g.quantity < 0
      ? `Puts ${qty} back into stock and removes ${formatINR(g.lossRemoved)} from Inventory Losses.`
      : `Removes ${qty} added by this adjustment.`;
  };

  const columns: DataTableColumn<StockAdjustment>[] = [
    { key: 'date', header: 'Date', mono: true, render: (r) => r.adjustmentDate },
    { key: 'item', header: 'Item', mono: true, render: (r) => `#${r.itemId}` },
    {
      key: 'qty',
      header: 'Quantity',
      align: 'right',
      mono: true,
      render: (r) => (r.quantity > 0 ? `+${r.quantity}` : String(r.quantity)),
    },
    {
      key: 'reason',
      header: 'Reason',
      render: (r) => ADJUSTMENT_REASONS.find((x) => x.value === r.reason)?.label ?? r.reason,
    },
    {
      key: 'cost',
      header: 'Unit Cost',
      align: 'right',
      render: (r) => (r.unitCost !== null ? <CurrencyDisplay amount={r.unitCost} /> : '-'),
    },
    { key: 'notes', header: 'Notes', render: (r) => r.notes ?? '' },
    {
      key: 'undo',
      header: '',
      align: 'right',
      render: (r) => (
        <Button variant="outline" size="sm" onClick={() => openUndo(r)} aria-label={`Undo adjustment #${r.id}`}>
          Undo
        </Button>
      ),
    },
  ];

  return (
    <>
    <DataTable
      columns={columns}
      items={rows}
      rowKey={(r) => r.id}
      isLoading={loading}
      error={error}
      onRetry={load}
      emptyMessage="No stock adjustments recorded yet."
      total={total}
      page={page}
      limit={PAGE_SIZE}
      onPageChange={setPage}
    />
    <ConfirmDialog
      isOpen={undo !== null}
      title="Undo stock adjustment"
      message={undoMessage(undo?.guard ?? null)}
      confirmText="Undo"
      variant="warning"
      confirmDisabled={!undo?.guard?.canUndo}
      onConfirm={confirmUndo}
      onCancel={() => setUndo(null)}
    >
      {undo?.error && (
        <p role="alert" className="mt-2 text-xs text-rose-600">
          {undo.error}
        </p>
      )}
    </ConfirmDialog>
    </>
  );
};
