import React, { useState, useEffect, useRef, useId } from 'react';
import { Search, Loader2, Plus } from 'lucide-react';
import { cn, HIGHLIGHT_ROW } from '@/lib/utils';
import { useEscapeLayer } from '@/hooks/useHotkeys';
import { useLiveSearch, TYPEAHEAD_DEBOUNCE_MS } from '@/hooks/useLiveSearch';

export interface LiveSearchTypeaheadProps<T> {
  search: (query: string) => Promise<T[]>;
  getKey: (item: T) => string | number;
  renderItem: (item: T, state: { highlighted: boolean }) => React.ReactNode;
  onSelect: (item: T) => void;
  /** Label used for the exact-match check that hides the create row. */
  getLabel?: (item: T) => string;
  /** Enables the "+ Create" row; Enter with no results calls it. Receives the trimmed query. */
  onCreate?: (query: string) => void;
  createLabel?: (query: string) => string;
  pickOnEnter?: (
    items: T[],
    query: string,
    ctx: { highlightedIndex: number; resultsWereShown: boolean }
  ) => T | undefined;
  /** Enter pressed on an empty query (e.g. open the payment dialog). */
  onEmptyEnter?: () => void;
  emptyText?: (query: string) => string;
  ariaLabel: string;
  listAriaLabel?: string;
  testId?: string;
  placeholder?: string;
  debounceMs?: number;
  minChars?: number;
  autoFocus?: boolean;
  disabled?: boolean;
  /** Clear the query after a selection (default true). Ignored when `selectedLabel` is used. */
  clearOnSelect?: boolean;
  /**
   * Controlled "picked" text (FR-48): a string keeps the picked name in the box (no search runs
   * for it); going back to null clears the box, unless the user has already typed something else.
   * Leave undefined for the default clear-on-select behaviour.
   */
  selectedLabel?: string | null;
  /** Every edit of the input text (lets the parent un-pick when the text no longer matches). */
  onQueryChange?: (query: string) => void;
  /** Enter on the unchanged `selectedLabel` (e.g. move on to the quantity). */
  onLabelEnter?: () => void;
  className?: string;
  inputRef?: React.MutableRefObject<HTMLInputElement | null>;
  /**
   * `dropdown` (default): absolute popover under the input. `inline`: the results sit in normal
   * flow below the input inside a scrolling container (the input row never scrolls away).
   */
  resultsMode?: 'dropdown' | 'inline';
  /** Extra classes for the results container. */
  resultsClassName?: string;
}

/**
 * Debounced search box with a keyboard-navigable result list and an optional inline
 * "+ Create" row (expense types, charge types, ...). Item, party and type pickers build on it.
 */
export function LiveSearchTypeahead<T>({
  search,
  getKey,
  renderItem,
  onSelect,
  getLabel,
  onCreate,
  createLabel = (q) => `+ Create "${q}"`,
  pickOnEnter,
  onEmptyEnter,
  emptyText = (q) => `No matches for "${q}"`,
  ariaLabel,
  listAriaLabel,
  testId,
  placeholder,
  debounceMs = TYPEAHEAD_DEBOUNCE_MS,
  minChars = 1,
  autoFocus = false,
  disabled = false,
  clearOnSelect = true,
  selectedLabel,
  onQueryChange,
  onLabelEnter,
  className,
  inputRef: externalInputRef,
  resultsMode = 'dropdown',
  resultsClassName,
}: LiveSearchTypeaheadProps<T>): React.JSX.Element {
  const live = useLiveSearch<T[]>({
    initial: [],
    debounceMs,
    minChars,
    emptyBehavior: 'clear',
    fetcher: (q) =>
      search(q).then(
        (items) => (Array.isArray(items) ? items : []),
        () => [] as T[]
      ),
  });
  const { query, setQuery, results, resultsFor, isLoading, showSpinner, flush, reset, showText } = live;
  const controlled = selectedLabel !== undefined;

  const [isOpen, setIsOpen] = useState(false);
  const [highlightedIndex, setHighlightedIndex] = useState(0);
  const listId = useId();
  const containerRef = useRef<HTMLDivElement>(null);
  const localInputRef = useRef<HTMLInputElement | null>(null);
  const queryRef = useRef('');
  queryRef.current = query;
  const highlightedRef = useRef(0);
  highlightedRef.current = highlightedIndex;
  const cb = useRef({ onSelect, onCreate, onEmptyEnter, pickOnEnter, onLabelEnter });
  cb.current = { onSelect, onCreate, onEmptyEnter, pickOnEnter, onLabelEnter };
  const labelRef = useRef<string | null | undefined>(selectedLabel);
  labelRef.current = selectedLabel;

  // FR-48: show the picked name; clear it when the parent un-picks (but never wipe what the user is typing).
  const prevLabel = useRef<string | null | undefined>(undefined);
  useEffect(() => {
    if (!controlled) return;
    const before = prevLabel.current;
    prevLabel.current = selectedLabel;
    if (selectedLabel) {
      showText(selectedLabel);
      setIsOpen(false);
    } else if (before && queryRef.current === before) {
      reset();
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selectedLabel, controlled]);

  const setInput = (el: HTMLInputElement | null) => {
    localInputRef.current = el;
    if (externalInputRef) externalInputRef.current = el;
  };

  useEffect(() => {
    if (autoFocus) localInputRef.current?.focus();
  }, [autoFocus]);

  useEffect(() => {
    const handleClickOutside = (e: MouseEvent) => {
      if (containerRef.current && !containerRef.current.contains(e.target as Node)) setIsOpen(false);
    };
    document.addEventListener('mousedown', handleClickOutside);
    return () => document.removeEventListener('mousedown', handleClickOutside);
  }, []);

  useEscapeLayer(() => setIsOpen(false), isOpen);

  // New results open the list and highlight the first row; a cleared search closes it.
  useEffect(() => {
    if (resultsFor === null) {
      setIsOpen(false);
      return;
    }
    setIsOpen(true);
    setHighlightedIndex(0);
  }, [resultsFor, results]);

  const trimmed = query.trim();
  const settled = resultsFor === trimmed && trimmed !== '' && !isLoading;
  const exactMatch =
    !!getLabel && results.some((r) => getLabel(r).trim().toLowerCase() === trimmed.toLowerCase());
  const showCreate = !!onCreate && settled && !exactMatch;
  const optionCount = results.length + (showCreate ? 1 : 0);
  const showList = isOpen && optionCount > 0;
  const showEmpty = isOpen && settled && results.length === 0 && !onCreate;
  const activeId = showList ? `${listId}-opt-${highlightedIndex}` : undefined;

  // Keep the highlighted option visible inside the (scrolling) list.
  useEffect(() => {
    if (!showList) return;
    const el = containerRef.current?.querySelector(`[id="${listId}-opt-${highlightedIndex}"]`);
    (el as HTMLElement | null)?.scrollIntoView?.({ block: 'nearest' });
  }, [showList, highlightedIndex, listId]);

  const inline = resultsMode === 'inline';

  const closeAfterSelect = () => {
    // Controlled: drop the results but keep the text; the selectedLabel effect then shows the name.
    if (controlled) showText(queryRef.current);
    else if (clearOnSelect) reset();
    else setIsOpen(false);
  };

  const handleSelect = (item: T) => {
    closeAfterSelect();
    cb.current.onSelect(item);
  };

  const handleCreate = (q: string) => {
    closeAfterSelect();
    cb.current.onCreate?.(q);
  };

  const handleEnter = async () => {
    const q = queryRef.current.trim();
    if (!q) {
      cb.current.onEmptyEnter?.();
      return;
    }
    const label = labelRef.current;
    if (label && q === label.trim()) {
      cb.current.onLabelEnter?.();
      return;
    }
    let items: T[];
    let shown = false;
    if (resultsFor === q && !isLoading) {
      items = results;
      shown = true;
      const hi = highlightedRef.current;
      if (cb.current.onCreate && showCreate && hi === items.length) return handleCreate(q);
    } else {
      items = await flush();
      if (queryRef.current.trim() !== q) return; // user kept typing
    }
    const picked = cb.current.pickOnEnter
      ? cb.current.pickOnEnter(items, q, { highlightedIndex: highlightedRef.current, resultsWereShown: shown })
      : shown
        ? items[highlightedRef.current]
        : items[0];
    if (picked !== undefined) return handleSelect(picked);
    if (items.length === 0 && cb.current.onCreate) handleCreate(q);
  };

  const handleKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === 'ArrowDown') {
      if (!isOpen && optionCount > 0) {
        e.preventDefault();
        setIsOpen(true);
      } else if (optionCount > 0) {
        e.preventDefault();
        setHighlightedIndex((p) => (p + 1) % optionCount);
      }
    } else if (e.key === 'ArrowUp') {
      if (optionCount > 0) {
        e.preventDefault();
        setHighlightedIndex((p) => (p <= 0 ? optionCount - 1 : p - 1));
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

  return (
    <div ref={containerRef} className={cn('relative w-full', inline && 'flex min-h-0 flex-col', className)}>
      <div className={cn('relative flex items-center', inline && 'shrink-0')}>
        <Search className="absolute left-3 top-1/2 -translate-y-1/2 h-4 w-4 text-slate-400" />
        <input
          ref={setInput}
          type="text"
          role="combobox"
          aria-expanded={showList}
          aria-controls={showList ? listId : undefined}
          aria-activedescendant={activeId}
          aria-autocomplete="list"
          aria-label={ariaLabel}
          data-testid={testId}
          autoComplete="off"
          disabled={disabled}
          value={query}
          onChange={(e) => {
            setQuery(e.target.value);
            onQueryChange?.(e.target.value);
          }}
          onKeyDown={handleKeyDown}
          onFocus={(e) => {
            // A picked name is selected on focus, so typing replaces it at once.
            if (selectedLabel && e.currentTarget.value === selectedLabel) e.currentTarget.select();
            if (optionCount > 0) setIsOpen(true);
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
          aria-label={listAriaLabel ?? `${ariaLabel} results`}
          data-results-mode={resultsMode}
          className={cn(
            'mt-1 w-full rounded-lg bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 py-1',
            inline ? 'min-h-0 flex-1 overflow-y-auto' : 'absolute z-50 max-h-72 overflow-auto shadow-xl',
            resultsClassName
          )}
        >
          {showEmpty ? (
            <div className="px-4 py-3 text-xs text-slate-400 text-center">{emptyText(trimmed)}</div>
          ) : (
            <>
              {results.map((item, idx) => {
                const highlighted = idx === highlightedIndex;
                return (
                  <div
                    key={getKey(item)}
                    id={`${listId}-opt-${idx}`}
                    role="option"
                    aria-selected={highlighted}
                    onMouseDown={(e) => e.preventDefault()}
                    onClick={() => handleSelect(item)}
                    onMouseEnter={() => setHighlightedIndex(idx)}
                    className={cn(
                      'cursor-pointer transition-colors border-b border-slate-50 dark:border-slate-800/60 last:border-0 select-none',
                      highlighted
                        ? HIGHLIGHT_ROW
                        : 'hover:bg-slate-50 dark:hover:bg-slate-800/40'
                    )}
                  >
                    {renderItem(item, { highlighted })}
                  </div>
                );
              })}
              {showCreate && (
                <div
                  id={`${listId}-opt-${results.length}`}
                  role="option"
                  aria-selected={highlightedIndex === results.length}
                  data-create-option
                  onMouseDown={(e) => e.preventDefault()}
                  onClick={() => handleCreate(trimmed)}
                  onMouseEnter={() => setHighlightedIndex(results.length)}
                  className={cn(
                    'flex items-center gap-2 px-3 py-2 text-xs font-medium cursor-pointer select-none',
                    highlightedIndex === results.length
                      ? HIGHLIGHT_ROW
                      : 'text-blue-700 dark:text-blue-300 hover:bg-slate-50 dark:hover:bg-slate-800/40'
                  )}
                >
                  <Plus className="h-3.5 w-3.5" />
                  {createLabel(trimmed)}
                </div>
              )}
            </>
          )}
        </div>
      )}
    </div>
  );
}

export default LiveSearchTypeahead;
