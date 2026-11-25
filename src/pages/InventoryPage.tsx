import React, { useState, useEffect, useCallback, useRef } from 'react';
import {
  Search,
  Plus,
  RefreshCw,
  SlidersHorizontal,
  Pencil,
  ChevronRight,
  ChevronDown,
  Upload,
  PackagePlus,
  Boxes,
} from 'lucide-react';
import { api } from '@/api';
import { errorMessage } from '@/api/errors';
import { totalPages } from '@/lib/pagination';
import type { ItemSearchResult } from '@/types';
import type { Paginated } from '@/types/api';
import { useGridKeyboardNav } from '@/hooks/useGridKeyboardNav';
import { useLiveSearch, LIST_SEARCH_DEBOUNCE_MS } from '@/hooks/useLiveSearch';
import { LowStockLevelCell } from '@/components/domain/LowStockLevelCell';
import { StockAdjustmentModal } from '@/components/domain/StockAdjustmentModal';
import { SetStockDialog } from '@/components/domain/SetStockDialog';
import { BundleCreationModal } from '@/components/domain/BundleCreationModal';
import { ItemEditDrawer } from '@/components/domain/ItemEditDrawer';
import { ItemDetailsDrawer } from '@/components/domain/ItemDetailsDrawer';
import { LotInspector } from '@/components/domain/LotInspector';
import { ImportItemsDialog } from '@/components/domain/ImportItemsDialog';
import { StockStatusBadge } from '@/components/common/StockStatusBadge';
import { CurrencyDisplay } from '@/components/common/CurrencyDisplay';
import { Button } from '@/components/ui/Button';
import { Table, TableHeader, TableBody, TableRow, TableHead, TableCell } from '@/components/ui/Table';

const PAGE_SIZE = 50;
const COLS = 10;

const EMPTY_PAGE: Paginated<ItemSearchResult> = { items: [], total: 0, page: 1, limit: PAGE_SIZE };

export const InventoryPage: React.FC = () => {
  const [page, setPage] = useState(1);
  const [lowStockOnly, setLowStockOnly] = useState(false);
  const [expanded, setExpanded] = useState<Set<number>>(new Set());
  /** Rows saved inline since the last fetch (server response wins over the stale list row). */
  const [patched, setPatched] = useState<Map<number, ItemSearchResult>>(new Map());

  const [drawerOpen, setDrawerOpen] = useState(false);
  const [editingItem, setEditingItem] = useState<ItemSearchResult | null>(null);
  const [adjustItem, setAdjustItem] = useState<ItemSearchResult | null>(null);
  const [setStockItem, setSetStockItem] = useState<ItemSearchResult | null>(null);
  const [importOpen, setImportOpen] = useState(false);
  const [bundleItem, setBundleItem] = useState<ItemSearchResult | null>(null);
  const [detailsItem, setDetailsItem] = useState<ItemSearchResult | null>(null);
  const searchRef = useRef<HTMLInputElement>(null);

  const live = useLiveSearch<Paginated<ItemSearchResult>>({
    initial: EMPTY_PAGE,
    emptyBehavior: 'fetch',
    debounceMs: LIST_SEARCH_DEBOUNCE_MS,
    deps: [lowStockOnly, page],
    onQueryCommit: () => setPage(1),
    fetcher: (q) =>
      api.getItemsCatalog({
        category: null,
        lowStockOnly: lowStockOnly || null,
        search: q || null,
        page,
        limit: PAGE_SIZE,
      }),
  });
  const { results, isLoading: fetching, resultsFor } = live;
  const loadError = live.error ? errorMessage(live.error) : null;
  const total = loadError ? 0 : results.total;
  // Initial load only: later refetches keep the current rows on screen.
  const isLoading = resultsFor === null && !live.error && fetching;
  const items = loadError ? [] : results.items.map((i) => patched.get(i.itemId) ?? i);

  useEffect(() => {
    setPatched(new Map());
  }, [results]);

  const patchRow = useCallback((row: ItemSearchResult) => {
    setPatched((prev) => new Map(prev).set(row.itemId, row));
  }, []);

  const refresh = live.refresh;
  const refreshAll = useCallback(() => {
    void refresh();
  }, [refresh]);

  const toggleExpanded = (itemId: number) =>
    setExpanded((prev) => {
      const next = new Set(prev);
      if (next.has(itemId)) next.delete(itemId);
      else next.add(itemId);
      return next;
    });

  const pages = totalPages(total, PAGE_SIZE);

  const anyOverlayOpen =
    drawerOpen || detailsItem !== null || adjustItem !== null || setStockItem !== null || importOpen;
  const nav = useGridKeyboardNav<ItemSearchResult>({
    items,
    getId: (i) => i.itemId,
    enabled: !anyOverlayOpen && !isLoading && !loadError,
    onOpen: setDetailsItem,
    onExpand: (item, open) =>
      setExpanded((prev) => {
        if (prev.has(item.itemId) === open) return prev;
        const next = new Set(prev);
        if (open) next.add(item.itemId);
        else next.delete(item.itemId);
        return next;
      }),
    onPage: (dir) => setPage((p) => Math.min(pages, Math.max(1, p + dir))),
    onLeaveTop: () => searchRef.current?.focus(),
    letters: {
      e: (item) => {
        setEditingItem(item);
        setDrawerOpen(true);
      },
      s: (item) => {
        if (item.itemType !== 'SERVICE') setSetStockItem(item);
      },
    },
  });

  // The drawer shows the freshest catalog row for the item being edited.
  const drawerItem = editingItem
    ? (items.find((i) => i.itemId === editingItem.itemId) ?? editingItem)
    : null;

  return (
    <div className="flex flex-col gap-4 h-full min-h-0">
      {/* Top Filter & Toolbar */}
      <div className="shrink-0 p-3.5 rounded-lg bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 shadow-xs flex flex-wrap items-center justify-between gap-3">
        <div className="relative flex-1 max-w-md">
          <Search className="absolute left-2.5 top-1/2 -translate-y-1/2 h-4 w-4 text-slate-400" />
          <input
            type="text"
            aria-label="Search items"
            ref={searchRef}
            placeholder="Search items by name…"
            value={live.query}
            onChange={(e) => live.setQuery(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === 'Enter') {
                e.preventDefault();
                void live.flush();
              } else if (e.key === 'ArrowDown' && items.length > 0) {
                e.preventDefault();
                searchRef.current?.blur();
                nav.enterGrid();
              }
            }}
            className="w-full text-xs pl-8 pr-3 py-1.5 rounded-md border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-900 focus:outline-none focus:ring-2 focus:ring-blue-500"
          />
        </div>

        <div className="flex flex-wrap items-center gap-2.5">
          <label className="flex items-center gap-1.5 text-xs font-medium text-slate-600 dark:text-slate-300 cursor-pointer select-none">
            <input
              type="checkbox"
              aria-label="Low stock only"
              checked={lowStockOnly}
              onChange={(e) => {
                setPage(1);
                setLowStockOnly(e.target.checked);
              }}
              className="rounded text-amber-600 focus:ring-amber-500"
            />
            <span>Show Only Low Stock (below each item&apos;s level)</span>
          </label>

          <Button
            variant="outline"
            size="sm"
            onClick={refreshAll}
            title="Refresh Catalog"
            leftIcon={<RefreshCw className="h-3.5 w-3.5" />}
          >
            Refresh
          </Button>

          <Button
            variant="outline"
            size="sm"
            onClick={() => setImportOpen(true)}
            leftIcon={<Upload className="h-3.5 w-3.5" />}
          >
            Import Items
          </Button>

          <Button
            variant="primary"
            size="sm"
            onClick={() => {
              setEditingItem(null);
              setDrawerOpen(true);
            }}
            leftIcon={<Plus className="h-4 w-4" />}
          >
            Add Item
          </Button>
        </div>
      </div>

      {/* Catalog Table */}
      <div className="rounded-lg border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 overflow-hidden shadow-xs flex-1 min-h-[240px] flex flex-col">
        <Table dense scroll="fill">
          <TableHeader>
            <TableRow>
              <TableHead className="w-8" />
              <TableHead>Item Name</TableHead>
              <TableHead>Base Unit</TableHead>
              <TableHead align="right">Current Stock</TableHead>
              <TableHead className="text-center">Status</TableHead>
              <TableHead align="right">Low Stock Level</TableHead>
              <TableHead align="right">MRP</TableHead>
              <TableHead align="right">Sale Price</TableHead>
              <TableHead className="text-center">Bundles</TableHead>
              <TableHead className="text-center w-28">Actions</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {isLoading ? (
              <TableRow>
                <TableCell colSpan={COLS} className="py-12 text-center text-slate-400 text-xs">
                  Loading item catalog...
                </TableCell>
              </TableRow>
            ) : loadError ? (
              <TableRow>
                <TableCell colSpan={COLS} className="py-12 text-center text-rose-600 text-xs" role="alert">
                  {loadError}
                </TableCell>
              </TableRow>
            ) : items.length === 0 ? (
              <TableRow>
                <TableCell colSpan={COLS} className="py-12 text-center text-slate-400 text-xs">
                  No inventory items match current filter criteria.
                </TableCell>
              </TableRow>
            ) : (
              items.map((item) => {
                const isOpen = expanded.has(item.itemId);
                const isService = item.itemType === 'SERVICE';
                return (
                  <React.Fragment key={item.itemId}>
                    <TableRow
                      data-testid="item-row"
                      data-row-id={item.itemId}
                      selected={nav.activeId === item.itemId}
                      aria-selected={nav.activeId === item.itemId}
                      className="cursor-pointer"
                      onClick={(e) => {
                        if ((e.target as HTMLElement).closest('button, input, select, textarea, a')) return;
                        nav.setActiveId(item.itemId);
                        setDetailsItem(item);
                      }}
                    >
                      <TableCell className="text-center">
                        {!isService && (
                          <button
                            type="button"
                            aria-label={`${isOpen ? 'Hide' : 'Show'} lots for ${item.name}`}
                            aria-expanded={isOpen}
                            onClick={() => toggleExpanded(item.itemId)}
                            className="p-0.5 text-slate-400 hover:text-slate-700 cursor-pointer"
                          >
                            {isOpen ? <ChevronDown className="h-4 w-4" /> : <ChevronRight className="h-4 w-4" />}
                          </button>
                        )}
                      </TableCell>
                      <TableCell className="font-semibold text-slate-900 dark:text-slate-100">
                        {item.name}
                        {isService && (
                          <span
                            data-testid="service-badge"
                            className="ml-2 rounded-full border border-violet-200 bg-violet-50 px-2 py-0.5 text-[10px] font-semibold text-violet-700 dark:border-violet-800 dark:bg-violet-950/60 dark:text-violet-300"
                          >
                            Service
                          </span>
                        )}
                      </TableCell>
                      <TableCell className="text-xs text-slate-500">{item.baseUnit}</TableCell>
                      <TableCell align="right" mono className="text-xs">
                        {isService ? (
                          <span data-testid="can-make">
                            {item.canMake != null ? `Can make ${item.canMake}` : '—'}
                          </span>
                        ) : (
                          item.currentStock
                        )}
                      </TableCell>
                      <TableCell className="text-center">
                        {isService ? (
                          <span className="text-slate-400">—</span>
                        ) : (
                          <StockStatusBadge
                            currentStock={item.currentStock}
                            isLowStock={item.isLowStock}
                            lowStockThreshold={item.lowStockThreshold}
                            baseUnit={item.baseUnit}
                          />
                        )}
                      </TableCell>
                      <TableCell align="right">
                        {isService ? (
                          <span className="text-slate-400">—</span>
                        ) : (
                          <LowStockLevelCell
                            itemId={item.itemId}
                            itemName={item.name}
                            value={item.lowStockThreshold}
                            baseUnit={item.baseUnit}
                            onSaved={patchRow}
                          />
                        )}
                      </TableCell>
                      <TableCell align="right" mono className="text-slate-400 text-xs">
                        {item.mrp !== null ? <CurrencyDisplay amount={item.mrp} size="sm" /> : '—'}
                      </TableCell>
                      <TableCell align="right" mono className="font-bold text-slate-900 dark:text-slate-100">
                        <CurrencyDisplay amount={item.defaultSalePrice} size="sm" />
                      </TableCell>
                      <TableCell className="text-center text-xs font-mono">{isService ? '—' : item.bundles.length}</TableCell>
                      <TableCell className="text-center">
                        <div className="flex items-center justify-center gap-1.5">
                          <button
                            type="button"
                            aria-label={`Edit ${item.name}`}
                            onClick={() => {
                              setEditingItem(item);
                              setDrawerOpen(true);
                            }}
                            title={isService ? 'Edit service and components' : 'Edit item and bundles'}
                            className="p-1 rounded text-slate-500 hover:text-blue-600 hover:bg-slate-100 dark:hover:bg-slate-800 transition-colors cursor-pointer"
                          >
                            <Pencil className="h-3.5 w-3.5" />
                          </button>
                          {!isService && (
                            <button
                              type="button"
                              aria-label={`Adjust stock for ${item.name}`}
                              onClick={() => setAdjustItem(item)}
                              title="Adjust Stock / Scrap"
                              className="p-1 rounded text-slate-500 hover:text-amber-600 hover:bg-slate-100 dark:hover:bg-slate-800 transition-colors cursor-pointer"
                            >
                              <SlidersHorizontal className="h-3.5 w-3.5" />
                            </button>
                          )}
                          {item.itemType === 'PHYSICAL' && (
                            <button
                              type="button"
                              aria-label={`Add bundle for ${item.name}`}
                              onClick={() => setBundleItem(item)}
                              title="Add packaging bundle"
                              className="p-1 rounded text-slate-500 hover:text-blue-600 hover:bg-slate-100 dark:hover:bg-slate-800 transition-colors cursor-pointer"
                            >
                              <Boxes className="h-3.5 w-3.5" />
                            </button>
                          )}
                          {item.itemType !== 'SERVICE' && (
                            <button
                              type="button"
                              aria-label={`Set stock for ${item.name}`}
                              onClick={() => setSetStockItem(item)}
                              title="Set stock on hand (unrecorded stock)"
                              className="p-1 rounded text-slate-500 hover:text-emerald-600 hover:bg-slate-100 dark:hover:bg-slate-800 transition-colors cursor-pointer"
                            >
                              <PackagePlus className="h-3.5 w-3.5" />
                            </button>
                          )}
                        </div>
                      </TableCell>
                    </TableRow>
                    {isOpen && !isService && (
                      <TableRow data-testid="lot-expansion">
                        <TableCell colSpan={COLS} className="bg-slate-50/70 dark:bg-slate-800/30 py-2">
                          <LotInspector key={item.currentStock} itemId={item.itemId} baseUnit={item.baseUnit} />
                        </TableCell>
                      </TableRow>
                    )}
                  </React.Fragment>
                );
              })
            )}
          </TableBody>
        </Table>
      </div>

      {/* Pager */}
      <div className="shrink-0 flex items-center justify-between text-xs text-slate-500 px-1">
        <span data-testid="pager-summary">
          {total} item{total === 1 ? '' : 's'} · Page {page} of {pages}
        </span>
        <div className="flex items-center gap-2">
          <Button
            variant="outline"
            size="sm"
            disabled={page <= 1 || isLoading}
            onClick={() => setPage((p) => Math.max(1, p - 1))}
          >
            Previous
          </Button>
          <Button
            variant="outline"
            size="sm"
            disabled={page >= pages || isLoading}
            onClick={() => setPage((p) => p + 1)}
          >
            Next
          </Button>
        </div>
      </div>

      <ItemEditDrawer
        isOpen={drawerOpen}
        onClose={() => setDrawerOpen(false)}
        item={drawerItem}
        onSaved={refreshAll}
      />

      <ItemDetailsDrawer
        item={detailsItem ? (items.find((i) => i.itemId === detailsItem.itemId) ?? detailsItem) : null}
        onClose={() => setDetailsItem(null)}
        onEdit={(item) => {
          setDetailsItem(null);
          setEditingItem(item);
          setDrawerOpen(true);
        }}
      />

      <BundleCreationModal
        isOpen={bundleItem !== null}
        onClose={() => setBundleItem(null)}
        initialItemId={bundleItem?.itemId}
        baseUnit={bundleItem?.baseUnit}
        onBundleCreated={() => refreshAll()}
      />

      <ImportItemsDialog isOpen={importOpen} onClose={() => setImportOpen(false)} onImported={refreshAll} />

      {setStockItem && (
        <SetStockDialog
          isOpen
          item={{
            itemId: setStockItem.itemId,
            name: setStockItem.name,
            currentStock: setStockItem.currentStock,
            baseUnit: setStockItem.baseUnit,
          }}
          onClose={() => setSetStockItem(null)}
          onDone={refreshAll}
        />
      )}

      {adjustItem && (
        <StockAdjustmentModal
          isOpen
          onClose={() => setAdjustItem(null)}
          itemId={adjustItem.itemId}
          itemName={adjustItem.name}
          currentStock={adjustItem.currentStock}
          baseUnit={adjustItem.baseUnit}
          onAdjusted={refreshAll}
        />
      )}
    </div>
  );
};

export default InventoryPage;
