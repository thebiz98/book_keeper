import React from 'react';
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

/** FR-45 / A1: confirm text for deleting an item. Everything shown comes from `previewItemDelete`. */
export const ItemDeleteDialog: React.FC<ItemDeleteDialogProps> = ({ itemId, onClose, onDeleted }) => (
  <DeleteConfirmDialog<ItemDeletePreview>
    isOpen={itemId !== null}
    title="Delete item"
    loadPreview={() => api.previewItemDelete(itemId as number)}
    canDelete={(p) => p.canDelete}
    blockedReason={(p) => p.reason}
    confirmLabel={(p) => (p.mode === 'HARD' ? 'Delete permanently' : 'Delete item')}
    onConfirm={async (p) => {
      const r = await api.deleteItem(p.itemId);
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

export default ItemDeleteDialog;
