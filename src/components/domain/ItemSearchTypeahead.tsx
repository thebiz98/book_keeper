import React, { useState, useEffect, useRef } from 'react';
import { Search, Loader2 } from 'lucide-react';
import { cn } from '@/lib/utils';
import { api } from '@/api';
import type { ItemSearchResult } from '@/types';
import { StockStatusBadge } from '../common/StockStatusBadge';
import { CurrencyDisplay } from '../common/CurrencyDisplay';

export interface ItemSearchTypeaheadProps {
  onSelectItem: (item: ItemSearchResult) => void;
  placeholder?: string;
  autoFocus?: boolean;
  className?: string;
}

export const ItemSearchTypeahead: React.FC<ItemSearchTypeaheadProps> = ({
  onSelectItem,
  placeholder = 'Search item by name, barcode, or category (F4)...',
  autoFocus = false,
  className,
}) => {
  const [query, setQuery] = useState('');
  const [results, setResults] = useState<ItemSearchResult[]>([]);
  const [isOpen, setIsOpen] = useState(false);
  const [isLoading, setIsLoading] = useState(false);
  const [highlightedIndex, setHighlightedIndex] = useState(0);

  const containerRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (autoFocus) {
      inputRef.current?.focus();
    }
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

  useEffect(() => {
    const trimmed = query.trim();
    if (trimmed.length < 2) {
      setResults([]);
      setIsOpen(false);
      return;
    }

    setIsLoading(true);
    const timer = setTimeout(() => {
      api
        .searchItemsFts(trimmed, 10)
        .then((items) => {
          setResults(Array.isArray(items) ? items : []);
          setIsOpen(true);
          setHighlightedIndex(0);
        })
        .catch(() => {
          setResults([]);
        })
        .finally(() => {
          setIsLoading(false);
        });
    }, 150);

    return () => clearTimeout(timer);
  }, [query]);

  const handleSelect = (item: ItemSearchResult) => {
    onSelectItem(item);
    setQuery('');
    setIsOpen(false);
    setResults([]);
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (!isOpen || results.length === 0) return;

    if (e.key === 'ArrowDown') {
      e.preventDefault();
      setHighlightedIndex((prev) => (prev + 1) % results.length);
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      setHighlightedIndex((prev) => (prev <= 0 ? results.length - 1 : prev - 1));
    } else if (e.key === 'Enter') {
      e.preventDefault();
      const selected = results[highlightedIndex];
      if (selected) {
        handleSelect(selected);
      }
    } else if (e.key === 'Escape') {
      e.preventDefault();
      setIsOpen(false);
    }
  };

  return (
    <div ref={containerRef} className={cn('relative w-full', className)}>
      <div className="relative flex items-center">
        <Search className="absolute left-3 top-1/2 -translate-y-1/2 h-4 w-4 text-slate-400" />
        <input
          ref={inputRef}
          type="text"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          onKeyDown={handleKeyDown}
          onFocus={() => {
            if (results.length > 0) setIsOpen(true);
          }}
          placeholder={placeholder}
          className="w-full rounded-md border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-900 text-slate-900 dark:text-slate-100 text-sm pl-9 pr-8 py-2 placeholder-slate-400 focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-blue-500 transition-colors"
        />
        {isLoading && (
          <Loader2 className="absolute right-3 top-1/2 -translate-y-1/2 h-4 w-4 animate-spin text-slate-400" />
        )}
      </div>

      {isOpen && (
        <div className="absolute z-50 mt-1 max-h-72 w-full overflow-auto rounded-lg bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 shadow-xl py-1">
          {results.length === 0 ? (
            <div className="px-4 py-3 text-xs text-slate-400 text-center">
              No matching items found for &quot;{query}&quot;
            </div>
          ) : (
            results.map((item, idx) => {
              const isHighlighted = idx === highlightedIndex;
              const stock = item.currentStock;
              const unit = item.baseUnit;
              const price = item.defaultSalePrice;

              return (
                <div
                  key={item.itemId}
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
                    <StockStatusBadge currentStock={stock} baseUnit={unit} />
                    <div className="text-right font-mono font-semibold text-xs">
                      <CurrencyDisplay amount={price} size="sm" />
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
