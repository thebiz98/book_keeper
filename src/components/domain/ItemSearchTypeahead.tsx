import React from 'react';
import { api } from '@/api';
import type { ItemSearchResult } from '@/types';
import { LiveSearchTypeahead } from '../common/LiveSearchTypeahead';
import { StockStatusBadge } from '../common/StockStatusBadge';
import { CurrencyDisplay } from '../common/CurrencyDisplay';
import { TYPEAHEAD_DEBOUNCE_MS } from '@/hooks/useLiveSearch';

export const SEARCH_DEBOUNCE_MS = TYPEAHEAD_DEBOUNCE_MS;
export const SEARCH_LIMIT = 8;

export interface ItemSelectInfo {
  /** Set when the query matched a bundle barcode; callers pre-select that bundle. */
  matchedBundleId: number | null;
}

export interface ItemSearchTypeaheadProps {
  onSelectItem: (item: ItemSearchResult) => void;
  /** Same selection, with the matched bundle exposed. */
  onSelect?: (item: ItemSearchResult, info: ItemSelectInfo) => void;
  /** Enter pressed on an empty query (e.g. open the payment dialog). */
  onEmptyEnter?: () => void;
  placeholder?: string;
  autoFocus?: boolean;
  className?: string;
  /** Receives the underlying input, for focus management by the caller. */
  inputRef?: React.MutableRefObject<HTMLInputElement | null>;
  /** Clear the query after a selection (default true). */
  clearOnSelect?: boolean;
  /** FR-48: keep the picked item's name in the box (see LiveSearchTypeahead). */
  selectedLabel?: string | null;
  onQueryChange?: (query: string) => void;
  onLabelEnter?: () => void;
  disabled?: boolean;
  /** Enables the "+ Create" row and Enter-on-no-match; receives the trimmed query. */
  onCreate?: (query: string) => void;
  createLabel?: (query: string) => string;
  /** `inline` renders results in flow inside a scroll container (item lookup dialog). */
  resultsMode?: 'dropdown' | 'inline';
  resultsClassName?: string;
  /** Hide services (purchases, returns, adjustments and service components take stocked items only, FR-36). */
  physicalOnly?: boolean;
}

/** Best match for Enter: results shown -> the highlighted row; else exact (bundle) barcode hit, else the top row. */
function pickOnEnter(
  items: ItemSearchResult[],
  trimmed: string,
  ctx: { highlightedIndex: number; resultsWereShown: boolean }
): ItemSearchResult | undefined {
  if (ctx.resultsWereShown) {
    const highlighted = items[ctx.highlightedIndex];
    if (highlighted) return highlighted;
  }
  return (
    items.find((i) => i.matchedBundleId != null || (i.barcode != null && i.barcode === trimmed)) ?? items[0]
  );
}

const searchItems = (q: string) => api.searchItemsFts(q, SEARCH_LIMIT);
const searchPhysical = async (q: string) =>
  (await api.searchItemsFts(q, SEARCH_LIMIT * 2)).filter((i) => i.itemType !== 'SERVICE').slice(0, SEARCH_LIMIT);

/** Item typeahead used by billing, purchases, adjustments and returns (thin wrapper over LiveSearchTypeahead). */
export const ItemSearchTypeahead: React.FC<ItemSearchTypeaheadProps> = ({
  onSelectItem,
  onSelect,
  onEmptyEnter,
  placeholder = 'Search item by name or barcode (F4)...',
  autoFocus = false,
  className,
  inputRef,
  clearOnSelect = true,
  selectedLabel,
  onQueryChange,
  onLabelEnter,
  disabled = false,
  onCreate,
  createLabel,
  resultsMode,
  resultsClassName,
  physicalOnly = false,
}) => (
  <LiveSearchTypeahead<ItemSearchResult>
    resultsMode={resultsMode}
    resultsClassName={resultsClassName}
    search={physicalOnly ? searchPhysical : searchItems}
    onCreate={onCreate}
    createLabel={createLabel}
    getKey={(i) => i.itemId}
    getLabel={(i) => i.name}
    pickOnEnter={pickOnEnter}
    onEmptyEnter={onEmptyEnter}
    emptyText={(q) => `No matching items found for "${q}"`}
    ariaLabel="Search items"
    listAriaLabel="Item results"
    testId="item-search-input"
    placeholder={placeholder}
    debounceMs={SEARCH_DEBOUNCE_MS}
    autoFocus={autoFocus}
    className={className}
    inputRef={inputRef}
    clearOnSelect={clearOnSelect}
    selectedLabel={selectedLabel}
    onQueryChange={onQueryChange}
    onLabelEnter={onLabelEnter}
    disabled={disabled}
    onSelect={(item) => {
      onSelect?.(item, { matchedBundleId: item.matchedBundleId ?? null });
      onSelectItem(item);
    }}
    renderItem={(item) => (
      <div className="flex items-center justify-between px-3 py-2">
        <div className="flex flex-col">
          <span className="text-xs font-semibold text-slate-900 dark:text-slate-100">{item.name}</span>
          <div className="flex items-center gap-2 text-[10px] text-slate-400 font-mono">
            {item.category && <span>{item.category}</span>}
            {item.barcode && <span>• Barcode: {item.barcode}</span>}
          </div>
        </div>
        <div className="flex items-center gap-3">
          {item.itemType === 'SERVICE' ? (
            <span data-testid="service-result-tag" className="text-[10px] font-semibold text-violet-700 dark:text-violet-300">
              Service{item.canMake != null ? ` · can make ${item.canMake}` : ''}
            </span>
          ) : (
            <StockStatusBadge
              currentStock={item.currentStock}
              isLowStock={item.isLowStock}
              lowStockThreshold={item.lowStockThreshold}
              baseUnit={item.baseUnit}
            />
          )}
          <div className="text-right font-mono font-semibold text-xs">
            <CurrencyDisplay amount={item.defaultSalePrice} size="sm" />
          </div>
        </div>
      </div>
    )}
  />
);

export default ItemSearchTypeahead;
