import React, { useEffect, useRef, useState } from 'react';
import { api } from '@/api';
import { errorMessage } from '@/api/errors';
import type { ItemSearchResult } from '@/types';
import { Drawer } from '../ui/Drawer';
import { FormActions } from '../ui/FormActions';
import { TextInput } from '../ui/TextInput';
import { BundleTable } from './BundleTable';
import { parseOptional, validationField } from './formUtils';

export interface ItemEditDrawerProps {
  isOpen: boolean;
  onClose: () => void;
  /** null = create a new item. */
  item: ItemSearchResult | null;
  categories: string[];
  /** Called after a successful save or any bundle change. */
  onSaved: () => void;
}

type Field =
  | 'name'
  | 'category'
  | 'barcode'
  | 'baseUnit'
  | 'mrp'
  | 'defaultSalePrice'
  | 'initialStock'
  | 'initialCost';

const FIELD_MAP: Record<string, Field> = {
  name: 'name',
  category: 'category',
  barcode: 'barcode',
  base_unit: 'baseUnit',
  mrp: 'mrp',
  default_sale_price: 'defaultSalePrice',
  initial_stock: 'initialStock',
  initial_cost: 'initialCost',
};

const numText = (n: number | null | undefined) => (n === null || n === undefined ? '' : String(n));

/** Item master create / edit drawer. Stock is read-only on edit (DD-07). */
export const ItemEditDrawer: React.FC<ItemEditDrawerProps> = ({
  isOpen,
  onClose,
  item,
  categories,
  onSaved,
}) => {
  const isEdit = item !== null;
  const itemId = item?.itemId;

  const [name, setName] = useState('');
  const [category, setCategory] = useState('');
  const [barcode, setBarcode] = useState('');
  const [baseUnit, setBaseUnit] = useState('PCS');
  const [mrp, setMrp] = useState('');
  const [price, setPrice] = useState('');
  const [initialStock, setInitialStock] = useState('');
  const [initialCost, setInitialCost] = useState('');
  const [errors, setErrors] = useState<Partial<Record<Field, string>>>({});
  const [formError, setFormError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const busy = useRef(false);
  const bundleModalOpen = useRef(false);

  // Re-seed the form only when the drawer opens or a different item is chosen, so a bundle
  // refresh (new `item` object, same id) does not wipe unsaved edits.
  useEffect(() => {
    if (!isOpen) return;
    setName(item?.name ?? '');
    setCategory(item?.category ?? '');
    setBarcode(item?.barcode ?? '');
    setBaseUnit(item?.baseUnit ?? 'PCS');
    setMrp(numText(item?.mrp));
    setPrice(item ? String(item.defaultSalePrice) : '');
    setInitialStock('');
    setInitialCost('');
    setErrors({});
    setFormError(null);
    busy.current = false;
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [isOpen, itemId]);

  const handleSave = async () => {
    if (busy.current) return;
    const errs: Partial<Record<Field, string>> = {};
    const mrpVal = parseOptional(mrp);
    const priceVal = parseOptional(price);
    const stockVal = parseOptional(initialStock);
    const costVal = parseOptional(initialCost);

    if (!name.trim()) errs.name = 'Item name is required';
    if (!baseUnit.trim()) errs.baseUnit = 'Base unit is required';
    if (priceVal === null || !Number.isFinite(priceVal) || priceVal < 0) {
      errs.defaultSalePrice = 'Enter a sale price (0 or more)';
    }
    if (mrpVal !== null && (!Number.isFinite(mrpVal) || mrpVal <= 0)) {
      errs.mrp = 'MRP must be greater than 0 (leave empty for none)';
    }
    if (!isEdit) {
      if (stockVal !== null && (!Number.isFinite(stockVal) || stockVal < 0)) {
        errs.initialStock = 'Initial stock cannot be negative';
      }
      if (costVal !== null && (!Number.isFinite(costVal) || costVal < 0)) {
        errs.initialCost = 'Cost cannot be negative';
      }
      if (stockVal !== null && stockVal > 0 && costVal === null) {
        errs.initialCost = 'Enter the cost of the opening stock';
      }
    }
    setErrors(errs);
    if (Object.keys(errs).length > 0) return;

    busy.current = true;
    setSaving(true);
    setFormError(null);
    const base = {
      name: name.trim(),
      category: category.trim() || null,
      barcode: barcode.trim() || null,
      baseUnit: baseUnit.trim().toUpperCase(),
      mrp: mrpVal, // empty => null, never 0
      defaultSalePrice: priceVal as number,
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
    if (bundleModalOpen.current) return;
    if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) {
      e.preventDefault();
      void handleSave();
    }
  };

  return (
    <Drawer
      isOpen={isOpen}
      onClose={() => {
        if (!bundleModalOpen.current) onClose();
      }}
      title={isEdit ? `Edit Item: ${item.name}` : 'Add New Item'}
      description={isEdit ? 'Stock is read-only here. Change it via purchases, sales, returns or adjustments.' : undefined}
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

        <div className="grid grid-cols-2 gap-3">
          <div>
            <TextInput
              label="Category"
              list="inventory-categories"
              value={category}
              onChange={(e) => setCategory(e.target.value)}
              error={errors.category}
            />
            <datalist id="inventory-categories">
              {categories.map((c) => (
                <option key={c} value={c} />
              ))}
            </datalist>
          </div>
          <TextInput
            label="Barcode / SKU"
            value={barcode}
            onChange={(e) => setBarcode(e.target.value)}
            error={errors.barcode}
          />
        </div>

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
            label="Default Sale Price (₹) *"
            type="number"
            min={0}
            step="any"
            value={price}
            onChange={(e) => setPrice(e.target.value)}
            error={errors.defaultSalePrice}
            className="text-right font-mono"
          />
        </div>

        <div className="flex items-center justify-between rounded-md bg-slate-50 dark:bg-slate-800/50 px-3 py-2 text-xs">
          <span className="text-slate-500">Type</span>
          <span data-testid="item-type" className="font-medium">
            {item?.itemType === 'SERVICE' ? 'Service' : 'Physical (stocked item)'}
          </span>
        </div>

        {isEdit ? (
          <div className="flex items-center justify-between rounded-md bg-slate-50 dark:bg-slate-800/50 px-3 py-2 text-xs">
            <span className="text-slate-500">Current stock (read-only)</span>
            <span className="font-mono font-semibold" data-testid="readonly-stock">
              {item.currentStock} {item.baseUnit}
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
              label="Opening Cost / base unit (₹)"
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
    </Drawer>
  );
};

export default ItemEditDrawer;
