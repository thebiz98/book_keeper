import React, { useEffect, useRef, useState } from 'react';
import { cn } from '@/lib/utils';
import { api } from '@/api';
import { errorMessage } from '@/api/errors';
import type { ItemSearchResult } from '@/types';

export interface LowStockLevelCellProps {
  itemId: number;
  itemName: string;
  value: number;
  baseUnit: string;
  /** Receives the refreshed row returned by the server (flag recomputed there). */
  onSaved: (row: ItemSearchResult) => void;
}

const INVALID = 'Enter 0 or more';

/**
 * Always-an-input cell for the per-item low stock level. Focus selects the text; Enter saves and
 * moves to the next row's cell; blur saves; Esc reverts. Invalid or unchanged input is never sent.
 */
export const LowStockLevelCell: React.FC<LowStockLevelCellProps> = ({
  itemId,
  itemName,
  value,
  baseUnit,
  onSaved,
}) => {
  const [draft, setDraft] = useState(String(value));
  const [error, setError] = useState<string | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const savingRef = useRef(false);
  /** Last value known to be stored on the server (prop, or what we just saved). */
  const knownRef = useRef(value);
  const onSavedRef = useRef(onSaved);
  onSavedRef.current = onSaved;

  useEffect(() => {
    knownRef.current = value;
    if (document.activeElement !== inputRef.current) setDraft(String(value));
  }, [value]);

  const focusNext = () => {
    const all = Array.from(document.querySelectorAll<HTMLInputElement>('[data-low-stock-input]'));
    const idx = inputRef.current ? all.indexOf(inputRef.current) : -1;
    all[idx + 1]?.focus();
  };

  const commit = async (next?: 'next') => {
    if (savingRef.current) return;
    const text = draft.trim();
    const n = Number(text);
    if (text === '' || !Number.isFinite(n) || n < 0) {
      setError(INVALID);
      return;
    }
    if (n === knownRef.current) {
      setError(null);
      setDraft(String(n));
      if (next) focusNext();
      return;
    }
    savingRef.current = true;
    try {
      const row = await api.updateItemLowStockThreshold(itemId, n);
      knownRef.current = row.lowStockThreshold;
      setDraft(String(row.lowStockThreshold));
      setError(null);
      onSavedRef.current(row);
      if (next) focusNext();
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      savingRef.current = false;
    }
  };

  return (
    <div className="flex flex-col items-end gap-0.5">
      <div className="flex items-center justify-end gap-1">
        <input
          ref={inputRef}
          data-low-stock-input
          type="text"
          inputMode="decimal"
          aria-label={`Low stock level for ${itemName}`}
          aria-invalid={error ? true : undefined}
          value={draft}
          onChange={(e) => {
            setDraft(e.target.value);
            if (error) setError(null);
          }}
          onFocus={(e) => e.currentTarget.select()}
          onKeyDown={(e) => {
            if (e.key === 'Enter') {
              e.preventDefault();
              void commit('next');
            } else if (e.key === 'Escape') {
              e.preventDefault();
              e.stopPropagation();
              e.nativeEvent.stopImmediatePropagation();
              setDraft(String(knownRef.current));
              setError(null);
            }
          }}
          onBlur={() => void commit()}
          className={cn(
            'w-16 rounded border bg-white dark:bg-slate-900 px-1.5 py-0.5 text-right text-xs font-mono tabular-nums focus:outline-none focus:ring-2 focus:ring-blue-500',
            error ? 'border-rose-400' : 'border-slate-300 dark:border-slate-700'
          )}
        />
        <span className="text-[10px] text-slate-400">{baseUnit}</span>
      </div>
      {error && (
        <span role="alert" className="text-[10px] text-rose-600">
          {error}
        </span>
      )}
    </div>
  );
};

export default LowStockLevelCell;
