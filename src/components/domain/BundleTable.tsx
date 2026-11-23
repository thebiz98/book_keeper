import React, { useEffect, useState } from 'react';
import { Pencil, Plus, Trash2 } from 'lucide-react';
import { api } from '@/api';
import { errorMessage } from '@/api/errors';
import type { ItemBundle, ItemBundleSearchResult } from '@/types';
import { Button } from '../ui/Button';
import { CurrencyDisplay } from '../common/CurrencyDisplay';
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '../ui/Table';
import { BundleCreationModal } from './BundleCreationModal';

export interface BundleTableProps {
  itemId: number;
  baseUnit: string;
  bundles: ItemBundleSearchResult[];
  /** Called after any create / update / delete so the caller can reload the catalog. */
  onChanged: () => void;
  /** Lets a host drawer ignore Esc while the bundle modal is open. */
  onModalToggle?: (open: boolean) => void;
}

/** Packaging bundles sub-table of the item drawer: list, add, edit, delete. */
export const BundleTable: React.FC<BundleTableProps> = ({ itemId, baseUnit, bundles, onChanged, onModalToggle }) => {
  const [modalOpen, setModalOpen] = useState(false);
  const [editing, setEditing] = useState<ItemBundle | null>(null);
  const [confirmId, setConfirmId] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    onModalToggle?.(modalOpen);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [modalOpen]);

  const openAdd = () => {
    setEditing(null);
    setModalOpen(true);
  };

  const openEdit = (b: ItemBundleSearchResult) => {
    setEditing({
      id: b.bundleId,
      itemId,
      bundleName: b.bundleName,
      unitsPerBundle: b.unitsPerBundle,
      // Catalog rows omit bundle MRP, so it is unknown here (see mrpKnown below).
      mrp: null,
      defaultBundlePrice: b.bundlePrice,
      barcode: b.barcode,
    });
    setModalOpen(true);
  };

  const handleDelete = async (bundleId: number) => {
    setError(null);
    try {
      await api.deleteBundle(bundleId);
      setConfirmId(null);
      onChanged();
    } catch (err) {
      setError(errorMessage(err));
    }
  };

  return (
    <div className="flex flex-col gap-2" data-testid="bundle-table">
      <div className="flex items-center justify-between">
        <h3 className="text-xs font-semibold uppercase tracking-wide text-slate-500">
          Packaging Bundles
        </h3>
        <Button
          type="button"
          variant="outline"
          size="xs"
          onClick={openAdd}
          leftIcon={<Plus className="h-3 w-3" />}
        >
          Add Bundle
        </Button>
      </div>

      {bundles.length === 0 ? (
        <p className="text-xs text-slate-400">No bundles configured. Items sell in {baseUnit} only.</p>
      ) : (
        <Table dense scroll="page">
          <TableHeader>
            <TableRow className="text-left text-slate-400">
              <TableHead className="py-1 font-medium">Name</TableHead>
              <TableHead className="py-1 font-medium text-right">Multiplier</TableHead>
              <TableHead className="py-1 font-medium text-right">Price</TableHead>
              <TableHead className="py-1 font-medium">Barcode</TableHead>
              <TableHead className="py-1" />
            </TableRow>
          </TableHeader>
          <TableBody>
            {bundles.map((b) => (
              <TableRow key={b.bundleId} data-testid="bundle-row" >
                <TableCell className="py-1 font-medium">{b.bundleName}</TableCell>
                <TableCell className="py-1 text-right font-mono">
                  × {b.unitsPerBundle} {baseUnit}
                </TableCell>
                <TableCell className="py-1 text-right">
                  {b.bundlePrice != null ? <CurrencyDisplay amount={b.bundlePrice} size="sm" /> : '—'}
                </TableCell>
                <TableCell className="py-1 font-mono text-slate-500">{b.barcode || '—'}</TableCell>
                <TableCell className="py-1 text-right whitespace-nowrap">
                  {confirmId === b.bundleId ? (
                    <>
                      <button
                        type="button"
                        onClick={() => void handleDelete(b.bundleId)}
                        className="px-1.5 py-0.5 rounded bg-rose-600 text-white text-[10px] font-semibold cursor-pointer"
                      >
                        Confirm delete
                      </button>
                      <button
                        type="button"
                        onClick={() => setConfirmId(null)}
                        className="ml-1 px-1.5 py-0.5 text-[10px] text-slate-500 cursor-pointer"
                      >
                        Cancel
                      </button>
                    </>
                  ) : (
                    <>
                      <button
                        type="button"
                        aria-label={`Edit bundle ${b.bundleName}`}
                        onClick={() => openEdit(b)}
                        className="p-1 text-slate-500 hover:text-blue-600 cursor-pointer"
                      >
                        <Pencil className="h-3.5 w-3.5" />
                      </button>
                      <button
                        type="button"
                        aria-label={`Delete bundle ${b.bundleName}`}
                        onClick={() => setConfirmId(b.bundleId)}
                        className="p-1 text-slate-500 hover:text-rose-600 cursor-pointer"
                      >
                        <Trash2 className="h-3.5 w-3.5" />
                      </button>
                    </>
                  )}
                </TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      )}

      {error && (
        <p role="alert" className="text-xs text-rose-600 dark:text-rose-400 font-medium">
          {error}
        </p>
      )}

      <BundleCreationModal
        isOpen={modalOpen}
        onClose={() => setModalOpen(false)}
        initialItemId={itemId}
        baseUnit={baseUnit}
        bundle={editing}
        mrpKnown={false}
        onBundleCreated={() => onChanged()}
      />
    </div>
  );
};

export default BundleTable;
