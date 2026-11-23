import React, { useState, useEffect, useCallback } from 'react';
import {
  Search,
  Plus,
  Filter,
  RefreshCw,
  SlidersHorizontal,
  Pencil,
  ChevronRight,
  ChevronDown,
  Upload,
} from 'lucide-react';
import { api } from '@/api';
import { errorMessage } from '@/api/errors';
import { totalPages } from '@/lib/pagination';
import type { ItemSearchResult } from '@/types';
import type { Paginated } from '@/types/api';
import { useLiveSearch, LIST_SEARCH_DEBOUNCE_MS } from '@/hooks/useLiveSearch';
import { LowStockLevelCell } from '@/components/domain/LowStockLevelCell';
import { StockAdjustmentModal } from '@/components/domain/StockAdjustmentModal';
import { ItemEditDrawer } from '@/components/domain/ItemEditDrawer';
import { LotInspector } from '@/components/domain/LotInspector';
import { ImportItemsDialog } from '@/components/domain/ImportItemsDialog';
import { StockStatusBadge } from '@/components/common/StockStatusBadge';
import { CurrencyDisplay } from '@/components/common/CurrencyDisplay';
import { Button } from '@/components/ui/Button';
import { Table, TableHeader, TableBody, TableRow, TableHead, TableCell } from '@/components/ui/Table';

const PAGE_SIZE = 50;
const COLS = 11;

const EMPTY_PAGE: Paginated<ItemSearchResult> = { items: [], total: 0, page: 1, limit: PAGE_SIZE };

export const InventoryPage: React.FC = () => {
  const [page, setPage] = useState(1);
  const [category, setCategory] = useState<string>('ALL');
  const [categories, setCategories] = useState<string[]>([]);
  const [lowStockOnly, setLowStockOnly] = useState(false);
  const [expanded, setExpanded] = useState<Set<number>>(new Set());
  /** Rows saved inline since the last fetch (server response wins over the stale list row). */
  const [patched, setPatched] = useState<Map<number, ItemSearchResult>>(new Map());

  const [drawerOpen, setDrawerOpen] = useState(false);
  const [editingItem, setEditingItem] = useState<ItemSearchResult | null>(null);
  const [adjustItem, setAdjustItem] = useState<ItemSearchResult | null>(null);
  const [importOpen, setImportOpen] = useState(false);

  const live = useLiveSearch<Paginated<ItemSearchResult>>({
    initial: EMPTY_PAGE,
    emptyBehavior: 'fetch',
    debounceMs: LIST_SEARCH_DEBOUNCE_MS,
    deps: [category, lowStockOnly, page],
    onQueryCommit: () => setPage(1),
    fetcher: (q) =>
      api.getItemsCatalog({
        category: category === 'ALL' ? null : category,
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

  const fetchCategories = useCallback(() => {
    api
      .listCategories()
      .then((c) => setCategories(Array.isArray(c) ? c : []))
      .catch(() => {});
  }, []);

  useEffect(() => {
    fetchCategories();
  }, [fetchCategories]);

  const refresh = live.refresh;
  const refreshAll = useCallback(() => {
    void refresh();
    fetchCategories();
  }, [refresh, fetchCategories]);

  const toggleExpanded = (itemId: number) =>
    setExpanded((prev) => {
      const next = new Set(prev);
      if (next.has(itemId)) next.delete(itemId);
      else next.add(itemId);
      return next;
    });

  const pages = totalPages(total, PAGE_SIZE);

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
            placeholder="Search items by name, barcode, category..."
            value={live.query}
            onChange={(e) => live.setQuery(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === 'Enter') {
                e.preventDefault();
                void live.flush();
              }
            }}
            className="w-full text-xs pl-8 pr-3 py-1.5 rounded-md border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-900 focus:outline-none focus:ring-2 focus:ring-blue-500"
          />
        </div>

        <div className="flex flex-wrap items-center gap-2.5">
          <div className="flex items-center gap-1.5 text-xs text-slate-500">
            <Filter className="h-3.5 w-3.5" />
            <select
              aria-label="Category filter"
              value={category}
              onChange={(e) => {
                setPage(1);
                setCategory(e.target.value);
              }}
              className="px-2 py-1 rounded border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-900 text-xs text-slate-800 dark:text-slate-200 focus:outline-none"
            >
              <option value="ALL">All Categories</option>
              {categories.map((c) => (
                <option key={c} value={c}>
                  {c}
                </option>
              ))}
            </select>
          </div>

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
              <TableHead>Category</TableHead>
              <TableHead>Base Unit</TableHead>
              <TableHead align="right">Current Stock</TableHead>
              <TableHead className="text-center">Status</TableHead>
              <TableHead align="right">Low Stock Level</TableHead>
              <TableHead align="right">MRP</TableHead>
              <TableHead align="right">Sale Price</TableHead>
              <TableHead className="text-center">Bundles</TableHead>
              <TableHead className="text-center w-24">Actions</TableHead>
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
                return (
                  <React.Fragment key={item.itemId}>
                    <TableRow data-testid="item-row">
                      <TableCell className="text-center">
                        <button
                          type="button"
                          aria-label={`${isOpen ? 'Hide' : 'Show'} lots for ${item.name}`}
                          aria-expanded={isOpen}
                          onClick={() => toggleExpanded(item.itemId)}
                          className="p-0.5 text-slate-400 hover:text-slate-700 cursor-pointer"
                        >
                          {isOpen ? <ChevronDown className="h-4 w-4" /> : <ChevronRight className="h-4 w-4" />}
                        </button>
                      </TableCell>
                      <TableCell className="font-semibold text-slate-900 dark:text-slate-100">
                        {item.name}
                      </TableCell>
                      <TableCell className="text-slate-500 text-xs">{item.category || '—'}</TableCell>
                      <TableCell className="text-xs text-slate-500">{item.baseUnit}</TableCell>
                      <TableCell align="right" mono className="text-xs">
                        {item.currentStock}
                      </TableCell>
                      <TableCell className="text-center">
                        <StockStatusBadge
                          currentStock={item.currentStock}
                          isLowStock={item.isLowStock}
                          lowStockThreshold={item.lowStockThreshold}
                          baseUnit={item.baseUnit}
                        />
                      </TableCell>
                      <TableCell align="right">
                        <LowStockLevelCell
                          itemId={item.itemId}
                          itemName={item.name}
                          value={item.lowStockThreshold}
                          baseUnit={item.baseUnit}
                          onSaved={patchRow}
                        />
                      </TableCell>
                      <TableCell align="right" mono className="text-slate-400 text-xs">
                        {item.mrp !== null ? <CurrencyDisplay amount={item.mrp} size="sm" /> : '—'}
                      </TableCell>
                      <TableCell align="right" mono className="font-bold text-slate-900 dark:text-slate-100">
                        <CurrencyDisplay amount={item.defaultSalePrice} size="sm" />
                      </TableCell>
                      <TableCell className="text-center text-xs font-mono">{item.bundles.length}</TableCell>
                      <TableCell className="text-center">
                        <div className="flex items-center justify-center gap-1.5">
                          <button
                            type="button"
                            aria-label={`Edit ${item.name}`}
                            onClick={() => {
                              setEditingItem(item);
                              setDrawerOpen(true);
                            }}
                            title="Edit item and bundles"
                            className="p-1 rounded text-slate-500 hover:text-blue-600 hover:bg-slate-100 dark:hover:bg-slate-800 transition-colors cursor-pointer"
                          >
                            <Pencil className="h-3.5 w-3.5" />
                          </button>
                          <button
                            type="button"
                            aria-label={`Adjust stock for ${item.name}`}
                            onClick={() => setAdjustItem(item)}
                            title="Adjust Stock / Scrap"
                            className="p-1 rounded text-slate-500 hover:text-amber-600 hover:bg-slate-100 dark:hover:bg-slate-800 transition-colors cursor-pointer"
                          >
                            <SlidersHorizontal className="h-3.5 w-3.5" />
                          </button>
                        </div>
                      </TableCell>
                    </TableRow>
                    {isOpen && (
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
        categories={categories}
        onSaved={refreshAll}
      />

      <ImportItemsDialog isOpen={importOpen} onClose={() => setImportOpen(false)} onImported={refreshAll} />

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
