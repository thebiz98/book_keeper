import React, { useEffect, useState } from 'react';
import { Pencil } from 'lucide-react';
import { api } from '@/api';
import { errorMessage } from '@/api/errors';
import type { ItemBillRef, ItemSearchResult, TransactionType } from '@/types';
import type { Paginated } from '@/types/api';
import { Drawer } from '../ui/Drawer';
import { Button } from '../ui/Button';
import { Tabs } from '../ui/Tabs';
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '../ui/Table';
import { CurrencyDisplay } from '../common/CurrencyDisplay';
import { StockStatusBadge } from '../common/StockStatusBadge';
import { LotInspector } from './LotInspector';
import { BillDetailDrawer } from './BillDetailDrawer';
import { Pager } from './Pager';
import { LowStockLevelField } from './LowStockLevelField';

export const ITEM_HISTORY_PAGE_SIZE = 10;

export interface ItemDetailsDrawerProps {
  /** null = closed. */
  item: ItemSearchResult | null;
  onClose: () => void;
  /** Opens the Edit drawer for the item (the caller closes this one). */
  onEdit: (item: ItemSearchResult) => void;
  /** FR-50: the server row after the Low Stock Level is saved here (lets the caller patch its list). */
  onItemUpdated?: (row: ItemSearchResult) => void;
}

type TabId = 'lots' | 'sales' | 'purchases';

const Stat: React.FC<{ label: string; children: React.ReactNode }> = ({ label, children }) => (
  <div>
    <div className="text-[10px] uppercase tracking-wide text-slate-400">{label}</div>
    <div className="text-sm font-semibold text-slate-900 dark:text-slate-100">{children}</div>
  </div>
);

/** One lazily loaded, paged list of the bills containing the item (one row per bill). */
const BillsTab: React.FC<{
  itemId: number;
  kind: TransactionType;
  onView: (billId: number) => void;
}> = ({ itemId, kind, onView }) => {
  const [page, setPage] = useState(1);
  const [data, setData] = useState<Paginated<ItemBillRef> | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    setError(null);
    (kind === 'SALE'
      ? api.getItemSales(itemId, page, ITEM_HISTORY_PAGE_SIZE)
      : api.getItemPurchases(itemId, page, ITEM_HISTORY_PAGE_SIZE)
    )
      .then((res) => {
        if (!cancelled) setData(res);
      })
      .catch((e) => {
        if (!cancelled) setError(errorMessage(e));
      });
    return () => {
      cancelled = true;
    };
  }, [itemId, kind, page]);

  if (error) {
    return (
      <p role="alert" className="text-xs text-rose-600 dark:text-rose-400 px-2 py-1">
        Could not load {kind === 'SALE' ? 'sales' : 'purchases'}: {error}
      </p>
    );
  }
  if (data === null) return <p className="text-xs text-slate-400 px-2 py-1">Loading...</p>;
  if (data.total === 0) {
    return (
      <p className="text-xs text-slate-400 px-2 py-1">
        {kind === 'SALE' ? 'No sales of this item yet.' : 'No purchases of this item yet.'}
      </p>
    );
  }
  return (
    <div data-testid={`${kind === 'SALE' ? 'sales' : 'purchases'}-tab`}>
      <Table dense scroll="none">
        <TableHeader>
          <TableRow className="text-left text-slate-400">
            <TableHead>Code</TableHead>
            <TableHead>Date</TableHead>
            <TableHead>{kind === 'SALE' ? 'Customer' : 'Vendor'}</TableHead>
            <TableHead className="text-right" />
          </TableRow>
        </TableHeader>
        <TableBody>
          {data.items.map((b) => (
            <TableRow key={b.billId} data-testid="bill-row">
              <TableCell className="font-mono">{b.transactionCode}</TableCell>
              <TableCell className="font-mono">{b.transactionDate}</TableCell>
              <TableCell>{b.partyName || 'Walk-in'}</TableCell>
              <TableCell className="text-right">
                <Button
                  variant="outline"
                  size="sm"
                  aria-label={`View ${b.transactionCode}`}
                  onClick={() => onView(b.billId)}
                >
                  View
                </Button>
              </TableCell>
            </TableRow>
          ))}
        </TableBody>
      </Table>
      <Pager page={page} total={data.total} limit={ITEM_HISTORY_PAGE_SIZE} onPageChange={setPage} />
    </div>
  );
};

/** FR-23 Item details: summary, Lots, and the Sales / Purchases tabs (10 per page, loaded when opened). */
export const ItemDetailsDrawer: React.FC<ItemDetailsDrawerProps> = ({
  item: itemProp,
  onClose,
  onEdit,
  onItemUpdated,
}) => {
  const [tab, setTab] = useState<TabId>('lots');
  // FR-50: the last saved low stock level, so the drawer is right even when the caller does not patch.
  const [savedLevel, setSavedLevel] = useState<ItemSearchResult | null>(null);
  const item =
    itemProp && savedLevel && savedLevel.itemId === itemProp.itemId
      ? { ...itemProp, lowStockThreshold: savedLevel.lowStockThreshold, isLowStock: savedLevel.isLowStock }
      : itemProp;
  const [viewBill, setViewBill] = useState<{ kind: TransactionType; id: number } | null>(null);
  const itemId = item?.itemId ?? null;
  // FR-45: a deleted item opens read-only (from past bills and reports); Restore lives only in Deleted items.
  const deleted = item?.isActive === false;

  useEffect(() => {
    setTab('lots');
    setViewBill(null);
    setSavedLevel(null);
  }, [itemId]);

  return (
    <>
      <Drawer
        isOpen={item !== null}
        onClose={onClose}
        title={item?.name ?? 'Item details'}
        description={item ? `Base unit: ${item.baseUnit}` : undefined}
        width="xl"
      >
        {item && (
          <div className="space-y-4" data-testid="item-details">
            <div className="flex items-start justify-between gap-3">
              <div className="grid grid-cols-3 gap-x-6 gap-y-3 flex-1">
                <Stat label="Stock">
                  <span className="font-mono">
                    {item.currentStock} {item.baseUnit}
                  </span>
                </Stat>
                <Stat label="Status">
                  <StockStatusBadge
                    currentStock={item.currentStock}
                    isLowStock={item.isLowStock}
                    lowStockThreshold={item.lowStockThreshold}
                    baseUnit={item.baseUnit}
                  />
                </Stat>
                <Stat label="Low Stock Level">
                  <LowStockLevelField
                    itemId={item.itemId}
                    itemName={item.name}
                    value={item.lowStockThreshold}
                    baseUnit={item.baseUnit}
                    editable={!deleted && item.itemType !== 'SERVICE'}
                    onSaved={(row) => {
                      setSavedLevel(row);
                      onItemUpdated?.(row);
                    }}
                  />
                </Stat>
                <Stat label="MRP">{item.mrp !== null ? <CurrencyDisplay amount={item.mrp} size="sm" /> : '—'}</Stat>
                <Stat label="Sale Price">
                  <CurrencyDisplay amount={item.defaultSalePrice} size="sm" />
                </Stat>
              </div>
              {deleted ? (
                <div className="flex flex-col items-end gap-1">
                  <span
                    data-testid="item-deleted-badge"
                    className="rounded-full border border-rose-200 bg-rose-50 px-2 py-0.5 text-[10px] font-semibold text-rose-700 dark:border-rose-800 dark:bg-rose-950/60 dark:text-rose-300"
                  >
                    Deleted
                  </span>
                </div>
              ) : (
                <Button
                  variant="outline"
                  size="sm"
                  onClick={() => onEdit(item)}
                  leftIcon={<Pencil className="h-3.5 w-3.5" />}
                >
                  Edit
                </Button>
              )}
            </div>

            <Tabs
              tabs={[
                { id: 'lots', label: 'Lots' },
                { id: 'sales', label: 'Sales' },
                { id: 'purchases', label: 'Purchases' },
              ]}
              activeTab={tab}
              onChange={(id) => setTab(id as TabId)}
            />

            {tab === 'lots' && (
              <LotInspector key={item.currentStock} itemId={item.itemId} baseUnit={item.baseUnit} editable={!deleted} />
            )}
            {tab === 'sales' && (
              <BillsTab itemId={item.itemId} kind="SALE" onView={(id) => setViewBill({ kind: 'SALE', id })} />
            )}
            {tab === 'purchases' && (
              <BillsTab itemId={item.itemId} kind="PURCHASE" onView={(id) => setViewBill({ kind: 'PURCHASE', id })} />
            )}
          </div>
        )}
      </Drawer>
      <BillDetailDrawer
        kind={viewBill?.kind ?? 'SALE'}
        billId={viewBill?.id ?? null}
        onClose={() => setViewBill(null)}
        onChanged={() => setViewBill(null)}
      />
    </>
  );
};

export default ItemDetailsDrawer;
