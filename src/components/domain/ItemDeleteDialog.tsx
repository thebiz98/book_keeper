import React, { useEffect, useState } from 'react';
import { api } from '@/api';
import type { DeleteMode, ItemDeletePreview } from '@/types';
import { CurrencyDisplay } from '../common/CurrencyDisplay';
import { DeleteConfirmDialog } from './DeleteConfirmDialog';

export interface ItemDeleteDialogProps {
  /** null = closed. */
  itemId: number | null;
  onClose: () => void;
  /** Called after the backend deleted the item; `mode` says whether it is gone for good. */
  onDeleted: (name: string, mode: DeleteMode) => void;
}

/**
 * FR-45 / A1 / FR-58: confirm text for deleting an item. Everything shown comes from
 * `previewItemDelete`; with stock (`writeOff` non-null) the shop picks "Just delete" (default)
 * or "Book as loss", and the choice is sent as `writeOff`.
 */
export const ItemDeleteDialog: React.FC<ItemDeleteDialogProps> = ({ itemId, onClose, onDeleted }) => {
  const [writeOff, setWriteOff] = useState(false);
  useEffect(() => setWriteOff(false), [itemId]);

  return (
    <DeleteConfirmDialog<ItemDeletePreview>
      isOpen={itemId !== null}
      title="Delete item"
      loadPreview={() => api.previewItemDelete(itemId as number)}
      canDelete={(p) => p.canDelete}
      blockedReason={(p) => p.reason}
      confirmLabel={(p) => (p.mode === 'HARD' ? 'Delete permanently' : 'Delete item')}
      onConfirm={async (p) => {
        const r = await api.deleteItem(p.itemId, Boolean(p.writeOff) && writeOff);
        onDeleted(p.name, r.mode);
      }}
      onClose={onClose}
      renderBody={(p) => (
        <>
          {p.mode === 'HARD' ? (
            <p data-testid="delete-hard-text">
              &apos;{p.name}&apos; was never used. It will be removed permanently
              {p.stock > 0 && (
                <>
                  {' '}
                  ({p.stock} {p.baseUnit} opening stock, <CurrencyDisplay amount={p.stockValue} size="sm" />)
                </>
              )}
              .
            </p>
          ) : (
            <>
              <p data-testid="delete-soft-text">
                &apos;{p.name}&apos; has {p.stock} {p.baseUnit} in stock
                {p.stock > 0 && (
                  <>
                    {' '}
                    worth <CurrencyDisplay amount={p.stockValue} size="sm" />, which will leave Inventory value
                  </>
                )}
                .
              </p>
              {p.canDelete && p.writeOff && (
                <fieldset className="flex flex-col gap-2" data-testid="delete-writeoff-choice">
                  <legend className="sr-only">What happens to the stock</legend>
                  <label className="flex items-start gap-2">
                    <input
                      type="radio"
                      name="item-delete-writeoff"
                      data-testid="delete-just-delete"
                      checked={!writeOff}
                      onChange={() => setWriteOff(false)}
                    />
                    <span>
                      <span className="font-medium">Just delete</span>
                      <span className="block text-xs text-slate-500">
                        No loss is booked; restoring the item brings the {p.writeOff.quantity} {p.writeOff.baseUnit} back. Use this
                        when the item was a mistake and you will re-add it.
                      </span>
                    </span>
                  </label>
                  <label className="flex items-start gap-2">
                    <input
                      type="radio"
                      name="item-delete-writeoff"
                      data-testid="delete-book-loss"
                      checked={writeOff}
                      onChange={() => setWriteOff(true)}
                    />
                    <span>
                      <span className="font-medium">
                        Book as loss (<CurrencyDisplay amount={p.writeOff.loss} size="sm" />)
                      </span>
                      <span className="block text-xs text-slate-500">
                        Booked as Inventory Losses today, then the item is deleted.
                      </span>
                    </span>
                  </label>
                </fieldset>
              )}
              {p.canDelete && (
                <p className="text-xs text-slate-500">Used in past records, so it is moved to Deleted items. You can restore it later.</p>
              )}
              {p.blockingServices.length > 0 && (
                <ul className="list-disc pl-5 text-xs" data-testid="delete-blocking">
                  {p.blockingServices.map((s) => (
                    <li key={s}>{s}</li>
                  ))}
                </ul>
              )}
            </>
          )}
        </>
      )}
    />
  );
};

export default ItemDeleteDialog;
