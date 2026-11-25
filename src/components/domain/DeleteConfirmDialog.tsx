import React, { useEffect, useRef, useState } from 'react';
import { errorMessage } from '@/api/errors';
import { Modal } from '../ui/Modal';
import { Button } from '../ui/Button';

export interface DeleteConfirmDialogProps<P> {
  isOpen: boolean;
  title: string;
  /** Backend dry run (DD-56). Nothing is computed here: the body renders what it returns. */
  loadPreview: () => Promise<P>;
  renderBody: (preview: P) => React.ReactNode;
  canDelete: (preview: P) => boolean;
  /** Backend sentence shown (and the button disabled) when `canDelete` is false. */
  blockedReason?: (preview: P) => string | null;
  confirmLabel: string | ((preview: P) => string);
  /** Runs the delete. A rejection is shown in the dialog; success is the caller's to close. */
  onConfirm: (preview: P) => Promise<void>;
  onClose: () => void;
}

/**
 * Shared confirm dialog for deleting a record (items in E2, parties in E3). It loads a backend
 * preview when opened, shows it, and enables the danger button only when the backend says the
 * delete is allowed.
 */
export function DeleteConfirmDialog<P>({
  isOpen,
  title,
  loadPreview,
  renderBody,
  canDelete,
  blockedReason,
  confirmLabel,
  onConfirm,
  onClose,
}: DeleteConfirmDialogProps<P>): React.ReactElement | null {
  const [preview, setPreview] = useState<P | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const loadRef = useRef(loadPreview);
  loadRef.current = loadPreview;

  useEffect(() => {
    if (!isOpen) return;
    let live = true;
    setPreview(null);
    setLoadError(null);
    setError(null);
    loadRef
      .current()
      .then((p) => live && setPreview(p))
      .catch((e) => live && setLoadError(errorMessage(e)));
    return () => {
      live = false;
    };
  }, [isOpen]);

  if (!isOpen) return null;

  const allowed = preview !== null && canDelete(preview);
  const reason = preview !== null && !allowed ? (blockedReason?.(preview) ?? null) : null;
  const label = preview === null ? (typeof confirmLabel === 'string' ? confirmLabel : 'Delete') : typeof confirmLabel === 'string' ? confirmLabel : confirmLabel(preview);

  const confirm = async () => {
    if (preview === null || !allowed || busy) return;
    setBusy(true);
    setError(null);
    try {
      await onConfirm(preview);
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <Modal
      isOpen
      onClose={onClose}
      title={title}
      size="md"
      footer={
        <>
          <Button variant="outline" size="sm" onClick={onClose} data-testid="delete-confirm-cancel">
            Cancel
          </Button>
          <Button
            variant="danger"
            size="sm"
            data-testid="delete-confirm-submit"
            disabled={!allowed}
            isLoading={busy}
            onClick={() => void confirm()}
          >
            {label}
          </Button>
        </>
      }
    >
      <div data-testid="delete-confirm" className="flex flex-col gap-2 text-sm text-slate-700 dark:text-slate-200">
        {loadError ? (
          <p role="alert" className="text-xs text-rose-600">
            {loadError}
          </p>
        ) : preview === null ? (
          <p className="text-xs text-slate-400">Checking...</p>
        ) : (
          renderBody(preview)
        )}
        {reason && (
          <p role="alert" data-testid="delete-confirm-blocked" className="text-xs font-medium text-rose-600">
            {reason}
          </p>
        )}
        {error && (
          <p role="alert" className="text-xs font-medium text-rose-600">
            {error}
          </p>
        )}
      </div>
    </Modal>
  );
}

export default DeleteConfirmDialog;
