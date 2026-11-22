import React, { useState, useEffect, useRef, useCallback, useId } from 'react';
import { Search, Loader2 } from 'lucide-react';
import { cn } from '@/lib/utils';
import { api } from '@/api';
import type { ItemSearchResult } from '@/types';
import { useEscapeLayer } from '@/hooks/useHotkeys';
import { StockStatusBadge } from '../common/StockStatusBadge';
import { CurrencyDisplay } from '../common/CurrencyDisplay';

export const SEARCH_DEBOUNCE_MS = 50;
export const SEARCH_LIMIT = 8;
/** A spinner only appears when a search takes longer than this, so quick searches never flash. */
const SPINNER_DELAY_MS = 150;

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
  disabled?: boolean;
}

export const ItemSearchTypeahead: React.FC<ItemSearchTypeaheadProps> = ({
  onSelectItem,
  onSelect,
  onEmptyEnter,
  placeholder = 'Search item by name or barcode (F4)...',
  autoFocus = false,
  className,
  inputRef: externalInputRef,
  clearOnSelect = true,
  disabled = false,
}) => {
  const [query, setQuery] = useState('');
  const [results, setResults] = useState<ItemSearchResult[]>([]);
  /** The trimmed query `results` belong to; null while no search has completed. */
  const [resultsFor, setResultsFor] = useState<string | null>(null);
  const [isOpen, setIsOpen] = useState(false);
  const [isLoading, setIsLoading] = useState(false);
  const [showSpinner, setShowSpinner] = useState(false);
  const [highlightedIndex, setHighlightedIndex] = useState(0);

  const listId = useId();
  const containerRef = useRef<HTMLDivElement>(null);
  const localInputRef = useRef<HTMLInputElement | null>(null);
  const requestId = useRef(0);
  const queryRef = useRef('');
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const inflight = useRef<{ query: string; promise: Promise<ItemSearchResult[]> } | null>(null);
  const latestResults = useRef<{ query: string | null; items: ItemSearchResult[] }>({
    query: null,
    items: [],
  });

  const callbacks = useRef({ onSelectItem, onSelect, onEmptyEnter });
  callbacks.current = { onSelectItem, onSelect, onEmptyEnter };

  const setInput = (el: HTMLInputElement | null) => {
    localInputRef.current = el;
    if (externalInputRef) externalInputRef.current = el;
  };

  useEffect(() => {
    if (autoFocus) localInputRef.current?.focus();
  }, [autoFocus]);

  useEffect(() => {
    const handleClickOutside = (e: MouseEvent) => {
      if (containerRef.current && !containerRef.current.contains(e.target as Node)) {
        setIsOpen(false);
      }
    };
    document.addEventListener('mousedown', handleClickOutside);
    return () => document.removeEventListener('mousedown', handleClickOutside);
  }, []);

  useEffect(() => () => {
    if (timerRef.current) clearTimeout(timerRef.current);
    requestId.current++;
  }, []);

  useEscapeLayer(() => setIsOpen(false), isOpen);

  // Show the spinner only for slow searches.
  useEffect(() => {
    if (!isLoading) {
      setShowSpinner(false);
      return;
    }
    const t = setTimeout(() => setShowSpinner(true), SPINNER_DELAY_MS);
    return () => clearTimeout(t);
  }, [isLoading]);

  /** Run a search; only the latest request may apply its results (stale-response guard). */
  const runSearch = useCallback((trimmed: string): Promise<ItemSearchResult[]> => {
    if (inflight.current && inflight.current.query === trimmed) return inflight.current.promise;
    const id = ++requestId.current;
    setIsLoading(true);
    const promise = api
      .searchItemsFts(trimmed, SEARCH_LIMIT)
      .then((items) => (Array.isArray(items) ? items : []))
      .catch(() => [] as ItemSearchResult[])
      .then((items) => {
        if (id === requestId.current) {
          latestResults.current = { query: trimmed, items };
          setResults(items);
          setResultsFor(trimmed);
          setIsOpen(true);
          setHighlightedIndex(0);
          setIsLoading(false);
          inflight.current = null;
        }
        return items;
      });
    inflight.current = { query: trimmed, promise };
    return promise;
  }, []);

  // 50 ms debounce; a single character is enough (short barcodes / SKUs).
  useEffect(() => {
    const trimmed = query.trim();
    if (trimmed.length < 1) {
      requestId.current++;
      inflight.current = null;
      latestResults.current = { query: null, items: [] };
      setResults([]);
      setResultsFor(null);
      setIsOpen(false);
      setIsLoading(false);
      return;
    }
    timerRef.current = setTimeout(() => {
      timerRef.current = null;
      void runSearch(trimmed);
    }, SEARCH_DEBOUNCE_MS);
    return () => {
      if (timerRef.current) {
        clearTimeout(timerRef.current);
        timerRef.current = null;
      }
    };
  }, [query, runSearch]);

  const reset = () => {
    queryRef.current = '';
    requestId.current++;
    inflight.current = null;
    setQuery('');
    setIsOpen(false);
    setResults([]);
    setResultsFor(null);
    setIsLoading(false);
  };

  const handleSelect = (item: ItemSearchResult) => {
    const info = { matchedBundleId: item.matchedBundleId ?? null };
    if (clearOnSelect) reset();
    else setIsOpen(false);
    callbacks.current.onSelect?.(item, info);
    callbacks.current.onSelectItem(item);
  };

  /** Best match for Enter: exact barcode / bundle-barcode hit first, else the top row. */
  const pickTop = (items: ItemSearchResult[], trimmed: string): ItemSearchResult | undefined =>
    items.find(
      (i) => i.matchedBundleId != null || (i.barcode != null && i.barcode === trimmed)
    ) ?? items[0];

  const handleEnter = async () => {
    const trimmed = queryRef.current.trim();
    if (!trimmed) {
      callbacks.current.onEmptyEnter?.();
      return;
    }
    // Flush the pending debounce, then await the search for exactly this query.
    if (timerRef.current) {
      clearTimeout(timerRef.current);
      timerRef.current = null;
    }
    let items: ItemSearchResult[];
    if (latestResults.current.query === trimmed) {
      // Results for the current query are already in hand: honour the highlighted row.
      items = latestResults.current.items;
      const highlighted = items[highlightedIndexRef.current];
      if (highlighted) return handleSelect(highlighted);
    } else {
      items = await runSearch(trimmed);
      if (queryRef.current.trim() !== trimmed) return; // user kept typing
    }
    const top = pickTop(items, trimmed);
    if (top) handleSelect(top);
  };

  const highlightedIndexRef = useRef(0);
  highlightedIndexRef.current = highlightedIndex;

  const handleKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === 'ArrowDown') {
      if (!isOpen && results.length > 0) {
        e.preventDefault();
        setIsOpen(true);
      } else if (results.length > 0) {
        e.preventDefault();
        setHighlightedIndex((prev) => (prev + 1) % results.length);
      }
    } else if (e.key === 'ArrowUp') {
      if (results.length > 0) {
        e.preventDefault();
        setHighlightedIndex((prev) => (prev <= 0 ? results.length - 1 : prev - 1));
      }
    } else if (e.key === 'Enter') {
      if (e.repeat) {
        e.preventDefault();
        return;
      }
      if (e.ctrlKey || e.metaKey) return; // Ctrl+Enter belongs to the hotkey registry
      e.preventDefault();
      void handleEnter();
    }
  };

  const trimmedQuery = query.trim();
  const showEmpty =
    isOpen && !isLoading && resultsFor === trimmedQuery && trimmedQuery !== '' && results.length === 0;
  const showList = isOpen && results.length > 0;
  const activeId = showList ? `${listId}-opt-${highlightedIndex}` : undefined;

  return (
    <div ref={containerRef} className={cn('relative w-full', className)}>
      <div className="relative flex items-center">
        <Search className="absolute left-3 top-1/2 -translate-y-1/2 h-4 w-4 text-slate-400" />
        <input
          ref={setInput}
          type="text"
          role="combobox"
          aria-expanded={showList}
          aria-controls={showList ? listId : undefined}
          aria-activedescendant={activeId}
          aria-autocomplete="list"
          aria-label="Search items"
          data-testid="item-search-input"
          autoComplete="off"
          disabled={disabled}
          value={query}
          onChange={(e) => {
            queryRef.current = e.target.value;
            setQuery(e.target.value);
          }}
          onKeyDown={handleKeyDown}
          onFocus={() => {
            if (results.length > 0) setIsOpen(true);
          }}
          placeholder={placeholder}
          className="w-full rounded-md border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-900 text-slate-900 dark:text-slate-100 text-sm pl-9 pr-8 py-2 placeholder-slate-400 focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-blue-500 transition-colors disabled:opacity-50"
        />
        {showSpinner && (
          <Loader2
            aria-label="Searching"
            className="absolute right-3 top-1/2 -translate-y-1/2 h-4 w-4 animate-spin text-slate-400"
          />
        )}
      </div>

      {(showList || showEmpty) && (
        <div
          id={listId}
          role="listbox"
          aria-label="Item results"
          className="absolute z-50 mt-1 max-h-72 w-full overflow-auto rounded-lg bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 shadow-xl py-1"
        >
          {showEmpty ? (
            <div className="px-4 py-3 text-xs text-slate-400 text-center">
              No matching items found for &quot;{trimmedQuery}&quot;
            </div>
          ) : (
            results.map((item, idx) => {
              const isHighlighted = idx === highlightedIndex;
              return (
                <div
                  key={item.itemId}
                  id={`${listId}-opt-${idx}`}
                  role="option"
                  aria-selected={isHighlighted}
                  onMouseDown={(e) => e.preventDefault()}
                  onClick={() => handleSelect(item)}
                  onMouseEnter={() => setHighlightedIndex(idx)}
                  className={cn(
                    'flex items-center justify-between px-3 py-2 cursor-pointer transition-colors border-b border-slate-50 dark:border-slate-800/60 last:border-0 select-none',
                    isHighlighted
                      ? 'bg-blue-50/80 dark:bg-blue-950/40 text-blue-900 dark:text-blue-100'
                      : 'hover:bg-slate-50 dark:hover:bg-slate-800/40'
                  )}
                >
                  <div className="flex flex-col">
                    <span className="text-xs font-semibold text-slate-900 dark:text-slate-100">
                      {item.name}
                    </span>
                    <div className="flex items-center gap-2 text-[10px] text-slate-400 font-mono">
                      {item.category && <span>{item.category}</span>}
                      {item.barcode && <span>• Barcode: {item.barcode}</span>}
                    </div>
                  </div>

                  <div className="flex items-center gap-3">
                    <StockStatusBadge currentStock={item.currentStock} baseUnit={item.baseUnit} />
                    <div className="text-right font-mono font-semibold text-xs">
                      <CurrencyDisplay amount={item.defaultSalePrice} size="sm" />
                    </div>
                  </div>
                </div>
              );
            })
          )}
        </div>
      )}
    </div>
  );
};

export default ItemSearchTypeahead;
