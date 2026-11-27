import React, { useState } from 'react';
import { api, errorMessage } from '@/api';
import type { UndoableWriteoff } from '@/types';
import { Button } from '../ui/Button';
import { CurrencyDisplay } from '../common/CurrencyDisplay';

export interface UndoWriteoffHintProps {
  /** The backend's list (DD-92): earlier write-offs G7 says can be undone. Nothing shows when empty. */
  writeoffs: UndoableWriteoff[] | undefined;
  baseUnit: string;
  /** Called after an undo succeeded, so the caller re-runs its preview. */
  onUndone: () => void;
  /** Lead text; the default suits an increase ("adding stock back"). */
  lead?: string;
}

/**
 * DD-92 "Undo it instead": found stock that was written off earlier is put back by undoing the
 * write-off (G7), which also removes the loss, rather than added again as new stock.
 */
export const UndoWriteoffHint: React.FC<UndoWriteoffHintProps> = ({ writeoffs, baseUnit, onUndone, lead }) => {
  const [busy, setBusy] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);
  if (!writeoffs || writeoffs.length === 0) return null;

  const undo = async (id: number) => {
    setBusy(id);
    setError(null);
    try {
      await api.undoStockAdjustment(id);
      onUndone();
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(null);
    }
  };

  return (
    <div
      data-testid="undo-writeoff-hint"
      className="rounded-md border border-amber-300 dark:border-amber-800 bg-amber-50 dark:bg-amber-950/40 px-3 py-2 text-xs flex flex-col gap-1.5"
    >
      <p className="text-amber-800 dark:text-amber-300">
        {lead ?? 'Found stock that was written off earlier? Undo the write-off instead: the stock comes back and the loss is removed.'}
      </p>
      <ul className="flex flex-col gap-1">
        {writeoffs.map((w) => (
          <li key={w.adjustmentId} className="flex items-center justify-between gap-2" data-testid="undoable-writeoff">
            <span>
              #{w.adjustmentId} · {w.adjustmentDate} · −{w.quantity} {baseUnit} · loss <CurrencyDisplay amount={w.loss} />
            </span>
            <Button
              size="xs"
              variant="outline"
              aria-label={`Undo write-off ${w.adjustmentId}`}
              isLoading={busy === w.adjustmentId}
              disabled={busy !== null}
              onClick={() => void undo(w.adjustmentId)}
            >
              Undo
            </Button>
          </li>
        ))}
      </ul>
      {error && (
        <p role="alert" className="text-rose-600 dark:text-rose-400 font-medium">
          {error}
        </p>
      )}
    </div>
  );
};

export default UndoWriteoffHint;
