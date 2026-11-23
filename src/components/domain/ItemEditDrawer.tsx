import React, { useEffect, useRef, useState } from 'react';
import { api } from '@/api';
import { errorMessage } from '@/api/errors';
import type { ItemSearchResult } from '@/types';
import { Drawer } from '../ui/Drawer';
import { Button } from '../ui/Button';
import { FormActions } from '../ui/FormActions';
import { SetStockDialog } from './SetStockDialog';
import { TextInput } from '../ui/TextInput';
import { BundleTable } from './BundleTable';
import { validationField } from './formUtils';
import { FIELD_MAP, validateItemForm, type ItemFormErrors } from './itemForm';

export interface ItemEditDrawerProps {
  isOpen: boolean;
  onClose: () => void;
  /** null = create a new item. */
  item: ItemSearchResult | null;
  /** Called after a successful save or any bundle change. */
  onSaved: () => void;
}

const numText = (n: number | null | undefined) => (n === null || n === undefined ? '' : String(n));

/** Item master create / edit drawer. Stock is changed only through the Set stock dialog (DD-07, DD-40). */
export const ItemEditDrawer: React.FC<ItemEditDrawerProps> = ({
  isOpen,
  onClose,
  item,
  onSaved,
}) => {
  const isEdit = item !== null;
  const itemId = item?.itemId;

  const [name, setName] = useState('');
  const [baseUnit, setBaseUnit] = useState('PCS');
  const [mrp, setMrp] = useState('');
  const [price, setPrice] = useState('');
  const [initialStock, setInitialStock] = useState('');
  const [initialCost, setInitialCost] = useState('');
  const [lowStock, setLowStock] = useState('5');
  const [errors, setErrors] = useState<ItemFormErrors>({});
  const [formError, setFormError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const busy = useRef(false);
  const bundleModalOpen = useRef(false);
  const [stockOpen, setStockOpen] = useState(false);
  const stockDialogOpen = useRef(false);
  stockDialogOpen.current = stockOpen;

  // Re-seed the form only when the drawer opens or a different item is chosen, so a bundle
  // refresh (new `item` object, same id) does not wipe unsaved edits.
  useEffect(() => {
    if (!isOpen) return;
    setName(item?.name ?? '');
    setBaseUnit(item?.baseUnit ?? 'PCS');
    setMrp(numText(item?.mrp));
    setPrice(item ? String(item.defaultSalePrice) : '');
    setInitialStock('');
    setInitialCost('');
    setLowStock(item ? String(item.lowStockThreshold) : '5');
    setErrors({});
    setFormError(null);
    busy.current = false;
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [isOpen, itemId]);

  const handleSave = async () => {
    if (busy.current) return;
    const { errors: errs, values } = validateItemForm(
      { name, baseUnit, mrp, price, lowStock, initialStock, initialCost },
      isEdit ? 'edit' : 'create'
    );
    setErrors(errs);
    if (!values) return;
    const { mrp: mrpVal, price: priceVal, lowStock: lowVal, stock: stockVal, cost: costVal } = values;

    busy.current = true;
    setSaving(true);
    setFormError(null);
    const base = {
      name: name.trim(),
      // Category/barcode are not editable in the UI (D5): edit re-sends the stored values, create sends null.
      category: item?.category ?? null,
      barcode: item?.barcode ?? null,
      baseUnit: baseUnit.trim().toUpperCase(),
      mrp: mrpVal, // empty => null, never 0
      defaultSalePrice: priceVal as number,
      lowStockThreshold: lowVal as number,
    };
    try {
      if (isEdit) {
        await api.updateItem(item.itemId, base);
      } else {
        await api.createItem({
          ...base,
          initialStock: stockVal && stockVal > 0 ? stockVal : null,
          initialCost: stockVal && stockVal > 0 ? costVal : null,
          bundles: null,
        });
      }
      onSaved();
      onClose();
    } catch (err) {
      const f = validationField(err);
      const key = f ? FIELD_MAP[f] : undefined;
      if (key) setErrors({ [key]: errorMessage(err) });
      else setFormError(errorMessage(err) || 'Failed to save item');
    } finally {
      busy.current = false;
      setSaving(false);
    }
  };

  const onKeyDown = (e: React.KeyboardEvent) => {
    if (bundleModalOpen.current || stockDialogOpen.current) return;
    if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) {
      e.preventDefault();
      void handleSave();
    }
  };

  return (
    <Drawer
      isOpen={isOpen}
      onClose={() => {
        if (!bundleModalOpen.current && !stockDialogOpen.current) onClose();
      }}
      title={isEdit ? `Edit Item: ${item.name}` : 'Add New Item'}
      description={
        isEdit ? 'Stock changes through purchases, sales, returns, adjustments or Set stock.' : undefined
      }
      width="lg"
      footer={
        <FormActions
          onSave={() => void handleSave()}
          onCancel={onClose}
          isSubmitting={saving}
          saveLabel={isEdit ? 'Save Changes' : 'Save Item'}
          saveHotkey="Ctrl+Enter"
        />
      }
    >
      <div className="flex flex-col gap-3" onKeyDown={onKeyDown}>
        <TextInput
          label="Item Name *"
          value={name}
          onChange={(e) => setName(e.target.value)}
          error={errors.name}
          autoFocus
        />

        <div className="grid grid-cols-3 gap-3">
          <TextInput
            label="Base Unit *"
            value={baseUnit}
            onChange={(e) => setBaseUnit(e.target.value.toUpperCase())}
            error={errors.baseUnit}
          />
          <TextInput
            label="MRP (₹, optional)"
            type="number"
            min={0}
            step="any"
            placeholder="none"
            value={mrp}
            onChange={(e) => setMrp(e.target.value)}
            error={errors.mrp}
            className="text-right font-mono"
          />
          <TextInput
            label="Sale Price (₹) *"
            type="number"
            min={0}
            step="any"
            value={price}
            onChange={(e) => setPrice(e.target.value)}
            error={errors.defaultSalePrice}
            className="text-right font-mono"
          />
        </div>

        <div className="flex flex-col gap-1">
          <TextInput
            label="Low Stock Level (base units)"
            type="number"
            min={0}
            step="any"
            value={lowStock}
            onChange={(e) => setLowStock(e.target.value)}
            error={errors.lowStockThreshold}
            className="text-right font-mono"
          />
          <p className="text-[10px] text-slate-400">Marked Low Stock when stock falls below this. 0 = never</p>
        </div>

        <div className="flex items-center justify-between rounded-md bg-slate-50 dark:bg-slate-800/50 px-3 py-2 text-xs">
          <span className="text-slate-500">Type</span>
          <span data-testid="item-type" className="font-medium">
            {item?.itemType === 'SERVICE' ? 'Service' : 'Physical (stocked item)'}
          </span>
        </div>

        {isEdit ? (
          <div className="flex items-center justify-between rounded-md bg-slate-50 dark:bg-slate-800/50 px-3 py-2 text-xs">
            <span className="text-slate-500">Stock on hand</span>
            <span className="flex items-center gap-2">
              <span className="font-mono font-semibold" data-testid="readonly-stock">
                {item.currentStock} {item.baseUnit}
              </span>
              {item.itemType !== 'SERVICE' && (
                <Button
                  type="button"
                  size="xs"
                  variant="outline"
                  data-testid="set-stock-button"
                  onClick={() => setStockOpen(true)}
                >
                  Set stock…
                </Button>
              )}
            </span>
          </div>
        ) : (
          <div className="grid grid-cols-2 gap-3">
            <TextInput
              label="Opening Stock (optional)"
              type="number"
              min={0}
              step="any"
              value={initialStock}
              onChange={(e) => setInitialStock(e.target.value)}
              error={errors.initialStock}
              className="text-right font-mono"
            />
            <TextInput
              label="Purchase Price / base unit (₹)"
              type="number"
              min={0}
              step="any"
              value={initialCost}
              onChange={(e) => setInitialCost(e.target.value)}
              error={errors.initialCost}
              className="text-right font-mono"
            />
          </div>
        )}

        {formError && (
          <p role="alert" className="text-xs text-rose-600 font-medium">
            {formError}
          </p>
        )}

        {isEdit ? (
          <div className="mt-2 pt-3 border-t border-slate-100 dark:border-slate-800">
            <BundleTable
              itemId={item.itemId}
              baseUnit={item.baseUnit}
              bundles={item.bundles}
              onChanged={onSaved}
              onModalToggle={(o) => {
                bundleModalOpen.current = o;
              }}
            />
          </div>
        ) : (
          <p className="text-xs text-slate-400">Save the item first to add packaging bundles.</p>
        )}
      </div>
      {isEdit && (
        <SetStockDialog
          isOpen={stockOpen}
          item={{ itemId: item.itemId, name: item.name, currentStock: item.currentStock, baseUnit: item.baseUnit }}
          onClose={() => setStockOpen(false)}
          onDone={onSaved}
        />
      )}
    </Drawer>
  );
};

export default ItemEditDrawer;
