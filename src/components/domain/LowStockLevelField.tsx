import React, { useEffect, useRef, useState } from 'react';
import { Pencil } from 'lucide-react';
import { cn } from '@/lib/utils';
import { api } from '@/api';
import { errorMessage } from '@/api/errors';
import type { ItemSearchResult } from '@/types';
import { Button } from '../ui/Button';

export interface LowStockLevelFieldProps {
  itemId: number;
  itemName: string;
  value: number;
  baseUnit: string;
  /** false = read-only (services, deleted items). */
  editable: boolean;
  /** Receives the refreshed row returned by the server (flag recomputed there). */
  onSaved: (row: ItemSearchResult) => void;
}

const INVALID = 'Enter 0 or more';

/**
 * FR-50 Low Stock Level in the Item details drawer. Read-only until Edit is pressed; then an input
 * with Save / Cancel (Enter saves, Esc cancels). Invalid input is never sent; unchanged just closes.
 */
export const LowStockLevelField: React.FC<LowStockLevelFieldProps> = ({
  itemId,
  itemName,
  value,
  baseUnit,
  editable,
  onSaved,
}) => {
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState(String(value));
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    setEditing(false);
    setError(null);
  }, [itemId]);

  useEffect(() => {
    if (editing) inputRef.current?.select();
  }, [editing]);

  const start = () => {
    setDraft(String(value));
    setError(null);
    setEditing(true);
  };

  const cancel = () => {
    setEditing(false);
    setError(null);
  };

  const save = async () => {
    if (saving) return;
    const text = draft.trim();
    const n = Number(text);
    if (text === '' || !Number.isFinite(n) || n < 0) {
      setError(INVALID);
      return;
    }
    if (n === value) {
      cancel();
      return;
    }
    setSaving(true);
    try {
      const row = await api.updateItemLowStockThreshold(itemId, n);
      onSaved(row);
      setEditing(false);
      setError(null);
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setSaving(false);
    }
  };

  if (!editing) {
    return (
      <div className="flex items-center gap-1.5">
        <span className="font-mono" data-testid="low-stock-level">
          {value}
        </span>
        <span className="text-[10px] font-normal text-slate-400">{baseUnit}</span>
        {editable && (
          <button
            type="button"
            aria-label={`Edit low stock level for ${itemName}`}
            title="Edit low stock level"
            onClick={start}
            className="p-0.5 rounded text-slate-400 hover:text-blue-600 hover:bg-slate-100 dark:hover:bg-slate-800 cursor-pointer"
          >
            <Pencil className="h-3 w-3" />
          </button>
        )}
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-1">
      <div className="flex items-center gap-1.5">
        <input
          ref={inputRef}
          type="text"
          inputMode="decimal"
          aria-label={`Low stock level for ${itemName}`}
          aria-invalid={error ? true : undefined}
          value={draft}
          disabled={saving}
          onChange={(e) => {
            setDraft(e.target.value);
            if (error) setError(null);
          }}
          onKeyDown={(e) => {
            if (e.key === 'Enter') {
              e.preventDefault();
              void save();
            } else if (e.key === 'Escape') {
              // Cancel the edit only; do not close the drawer.
              e.preventDefault();
              e.stopPropagation();
              e.nativeEvent.stopImmediatePropagation();
              cancel();
            }
          }}
          className={cn(
            'w-20 rounded border bg-white dark:bg-slate-900 px-1.5 py-0.5 text-right text-xs font-mono font-normal tabular-nums focus:outline-none focus:ring-2 focus:ring-blue-500',
            error ? 'border-rose-400' : 'border-slate-300 dark:border-slate-700'
          )}
        />
        <span className="text-[10px] font-normal text-slate-400">{baseUnit}</span>
      </div>
      <div className="flex items-center gap-1">
        <Button size="xs" onClick={() => void save()} disabled={saving}>
          Save
        </Button>
        <Button size="xs" variant="ghost" onClick={cancel} disabled={saving}>
          Cancel
        </Button>
      </div>
      {error && (
        <span role="alert" className="text-[10px] font-normal text-rose-600">
          {error}
        </span>
      )}
    </div>
  );
};

export default LowStockLevelField;
