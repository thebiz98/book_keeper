import React, { useCallback, useEffect, useState } from 'react';
import { api, errorMessage } from '@/api';
import type { ReturnTransaction, ReturnType, StockAdjustment } from '@/types';
import { CurrencyDisplay } from '../common/CurrencyDisplay';
import { DataTable, type DataTableColumn } from './DataTable';
import { ADJUSTMENT_REASONS } from './StockAdjustmentForm';
import { ReturnDetailDrawer } from './ReturnDetailDrawer';

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
  ];

  return (
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
  );
};
