import { useMemo, useRef, useState } from 'react';
import { deletedReference, errorMessage, isAppError } from '@/api/errors';
import { formatINR } from '@/lib/money';
import type { PreviewError, SettlementsToMove } from '@/types';
import type { DeletedEntity } from '@/api/errors';

/** DD-88: the backend refuses a party change that would move payments until the user confirms. */
export const SETTLEMENTS_ON_BILL = 'SETTLEMENTS_ON_BILL';

export interface DeletedNotice {
  deleted: { entity: DeletedEntity; id: number; name: string };
  message: string;
}

export interface UseBillEditSaveOptions<P> {
  /** The preview's errors (backend dry run of the same edit). */
  errors: PreviewError[];
  /** From the bill's edit guard: what the confirm dialog says. */
  settlementsToMove: SettlementsToMove;
  /** Name of the party picked in the form (for the dialog text). */
  newPartyName: string | null;
  /** Load ready, form complete, preview settled. */
  ready: boolean;
  buildPayload: () => P | null;
  send: (payload: P) => Promise<unknown>;
  onSaved: () => void;
}

const isMoveError = (e: unknown) => isAppError(e) && e.type === 'BillLocked' && e.details.code === SETTLEMENTS_ON_BILL;

/**
 * Save flow of the bill edit forms (FR-20, FR-54). No rule lives here (DD-56): the backend's
 * `SETTLEMENTS_ON_BILL` turns into a confirm dialog that re-sends with `confirmMoveSettlements`,
 * and a `DeletedReference` (preview or save) turns into the Restore alert.
 */
export function useBillEditSave<P extends { confirmMoveSettlements?: boolean }>(opts: UseBillEditSaveOptions<P>) {
  const { errors, settlementsToMove, newPartyName, ready, buildPayload, send, onSaved } = opts;
  const [saving, setSaving] = useState(false);
  const savingRef = useRef(false);
  const [saveError, setSaveError] = useState<string | null>(null);
  const [saveDeleted, setSaveDeleted] = useState<DeletedNotice | null>(null);
  const [confirmOpen, setConfirmOpen] = useState(false);

  const moveError = errors.find((e) => e.field === SETTLEMENTS_ON_BILL) ?? null;
  // Everything but the move notice blocks Save; the move notice asks on Save instead.
  const blocking = useMemo(() => errors.filter((e) => e.field !== SETTLEMENTS_ON_BILL), [errors]);
  const previewDeleted = blocking.find((e) => e.deleted);
  const deleted: DeletedNotice | null = previewDeleted?.deleted
    ? { deleted: previewDeleted.deleted, message: previewDeleted.message }
    : saveDeleted;
  const canSave = ready && blocking.length === 0 && !saving;

  const n = settlementsToMove.count;
  const moveInfo = moveError ? `Saving will ask to move ${n === 1 ? '1 payment' : `${n} payments`}` : null;
  const confirmMessage = `Move ${n === 1 ? '1 earlier payment' : `${n} earlier payments`} (${formatINR(
    settlementsToMove.amount
  )}) to '${newPartyName ?? 'the new party'}'?`;

  const run = async (confirmMove: boolean) => {
    const payload = buildPayload();
    if (!payload) return;
    savingRef.current = true;
    setSaving(true);
    setSaveError(null);
    setSaveDeleted(null);
    try {
      await send(confirmMove ? { ...payload, confirmMoveSettlements: true } : payload);
      setConfirmOpen(false);
      onSaved();
    } catch (e) {
      setConfirmOpen(false);
      const ref = deletedReference(e);
      if (ref) setSaveDeleted({ deleted: { entity: ref.entity, id: ref.id, name: ref.name }, message: ref.message });
      else if (!confirmMove && isMoveError(e)) setConfirmOpen(true);
      else setSaveError(errorMessage(e));
    } finally {
      savingRef.current = false;
      setSaving(false);
    }
  };

  const save = async () => {
    if (savingRef.current || !canSave) return;
    if (moveError) {
      setConfirmOpen(true);
      return;
    }
    await run(false);
  };

  return {
    saving,
    saveError,
    blocking,
    deleted,
    moveInfo,
    canSave,
    save,
    /** Clears a save-time deleted notice after a restore. */
    clearDeleted: () => setSaveDeleted(null),
    confirm: {
      isOpen: confirmOpen,
      message: confirmMessage,
      onConfirm: () => run(true),
      onCancel: () => setConfirmOpen(false),
    },
  };
}
