import React, { useState } from 'react';
import { api, errorMessage } from '@/api';
import type { DeletedEntity } from '@/api/errors';
import { Button } from '../ui/Button';

export interface DeletedReferenceAlertProps {
  /** The deleted item or party the backend named (`DeletedReference`, DD-90). */
  deleted: { entity: DeletedEntity; id: number; name: string };
  /** The backend's message ("'X' is deleted. Restore it first."). */
  message: string;
  /** Called after the restore succeeded: the caller re-runs its preview and keeps its draft. */
  onRestored: () => void;
}

/**
 * DD-90: a write was refused because it uses a deleted item or party. Shows the backend message
 * and a Restore button; restoring calls `restoreItem` / `restoreParty` and hands back to the form.
 */
export const DeletedReferenceAlert: React.FC<DeletedReferenceAlertProps> = ({ deleted, message, onRestored }) => {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const restore = async () => {
    if (busy) return;
    setBusy(true);
    setError(null);
    try {
      if (deleted.entity === 'ITEM') await api.restoreItem(deleted.id);
      else await api.restoreParty(deleted.id);
      onRestored();
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div
      role="alert"
      data-testid="deleted-ref-alert"
      className="flex flex-wrap items-center justify-between gap-2 rounded-md border border-amber-200 dark:border-amber-800 bg-amber-50 dark:bg-amber-950/40 px-3 py-2 text-xs text-amber-800 dark:text-amber-300"
    >
      <span>{message}</span>
      <Button variant="outline" size="sm" data-testid="deleted-ref-restore" isLoading={busy} onClick={() => void restore()}>
        Restore '{deleted.name}'
      </Button>
      {error && <span className="w-full text-rose-600 dark:text-rose-400">{error}</span>}
    </div>
  );
};

export default DeletedReferenceAlert;
