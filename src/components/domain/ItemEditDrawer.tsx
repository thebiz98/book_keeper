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
import { LotInspector } from './LotInspector';
import { validationField } from './formUtils';
import { FIELD_MAP, validateItemForm, type ItemFormErrors } from './itemForm';
import { ServiceComponentsEditor, toComponentInputs, type ComponentRow } from './ServiceComponentsEditor';

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
  // FR-36: Physical / Service choice (create only; fixed once the item exists).
  const [kind, setKind] = useState<'PHYSICAL' | 'SERVICE'>('PHYSICAL');
  const [serviceCost, setServiceCost] = useState('');
  const [components, setComponents] = useState<ComponentRow[]>([]);
  const isService = isEdit ? item.itemType === 'SERVICE' : kind === 'SERVICE';
  const [errors, setErrors] = useState<ItemFormErrors>({});
  const [formError, setFormError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const busy = useRef(false);
  const bundleModalOpen = useRef(false);
  const lotModalOpen = useRef(false);
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
    setKind(item?.itemType === 'SERVICE' ? 'SERVICE' : 'PHYSICAL');
    setServiceCost(numText(item?.serviceCost));
    setComponents([]);
    if (item?.itemType === 'SERVICE') {
      let live = true;
      api
        .getServiceComponents(item.itemId)
        .then((rows) => {
          if (live)
            setComponents(
              rows.map((r) => ({ itemId: r.componentItemId, name: r.name, baseUnit: r.baseUnit, qty: String(r.quantityPerUnit) }))
            );
        })
        .catch((err) => live && setFormError(errorMessage(err) || 'Failed to load components'));
      busy.current = false;
      setErrors({});
      setFormError(null);
      return () => {
        live = false;
      };
    }
    setErrors({});
    setFormError(null);
    busy.current = false;
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [isOpen, itemId]);

  const handleSave = async () => {
    if (busy.current) return;
    const { errors: errs, values } = validateItemForm(
      { name, baseUnit, mrp, price, lowStock, initialStock, initialCost, serviceCost },
      isService ? 'service' : isEdit ? 'edit' : 'create'
    );
    setErrors(errs);
    if (!values) return;
    const { mrp: mrpVal, price: priceVal, lowStock: lowVal, stock: stockVal, cost: costVal, serviceCost: svcCost } = values;

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
    const serviceFields = isService ? { serviceCost: svcCost, components: toComponentInputs(components) } : {};
    try {
      if (isEdit) {
        await api.updateItem(item.itemId, { ...base, ...serviceFields });
      } else if (isService) {
        await api.createItem({
          ...base,
          ...serviceFields,
          itemType: 'SERVICE',
          initialStock: null,
          initialCost: null,
          bundles: null,
        });
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
    if (bundleModalOpen.current || stockDialogOpen.current || lotModalOpen.current) return;
    if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) {
      e.preventDefault();
      void handleSave();
    }
  };

  return (
    <Drawer
      isOpen={isOpen}
      onClose={() => {
        if (!bundleModalOpen.current && !stockDialogOpen.current && !lotModalOpen.current) onClose();
      }}
      title={isEdit ? `Edit ${isService ? 'Service' : 'Item'}: ${item.name}` : isService ? 'Add New Service' : 'Add New Item'}
      description={
        isEdit && !isService
          ? 'Stock changes through purchases, sales, returns, adjustments or Set stock.'
          : isService
            ? 'A service has no stock. Selling it uses up its components.'
            : undefined
      }
      width="lg"
      footer={
        <FormActions
          onSave={() => void handleSave()}
          onCancel={onClose}
          isSubmitting={saving}
          saveLabel={isEdit ? 'Save Changes' : isService ? 'Save Service' : 'Save Item'}
          saveHotkey="Ctrl+Enter"
        />
      }
    >
      <div className="flex flex-col gap-3" onKeyDown={onKeyDown}>
        <div className="flex items-center justify-between rounded-md bg-slate-50 dark:bg-slate-800/50 px-3 py-2 text-xs">
          <span className="text-slate-500">Type</span>
          <span
            data-testid="item-type"
            role="radiogroup"
            aria-label="Item type"
            className="inline-flex rounded-md border border-slate-300 dark:border-slate-700 overflow-hidden"
          >
            {(['PHYSICAL', 'SERVICE'] as const).map((k) => (
              <button
                key={k}
                type="button"
                role="radio"
                aria-checked={isService === (k === 'SERVICE')}
                data-testid={k === 'SERVICE' ? 'type-service' : 'type-physical'}
                disabled={isEdit}
                onClick={() => setKind(k)}
                className={
                  'px-3 py-1 font-medium disabled:cursor-not-allowed ' +
                  (isService === (k === 'SERVICE')
                    ? 'bg-blue-600 text-white'
                    : 'bg-white dark:bg-slate-900 text-slate-600 dark:text-slate-300 disabled:opacity-60')
                }
              >
                {k === 'SERVICE' ? 'Service' : 'Physical'}
              </button>
            ))}
          </span>
        </div>

        <TextInput
          label={isService ? 'Service Name *' : 'Item Name *'}
          value={name}
          onChange={(e) => setName(e.target.value)}
          error={errors.name}
          autoFocus
        />

        <div className="grid grid-cols-3 gap-3">
          <TextInput
            label={isService ? 'Unit * (e.g. PRINT)' : 'Base Unit *'}
            value={baseUnit}
            onChange={(e) => setBaseUnit(e.target.value.toUpperCase())}
            error={errors.baseUnit}
          />
          {isService ? (
            <TextInput
              label="Service cost (₹/unit, optional)"
              type="number"
              min={0}
              step="any"
              placeholder="none"
              value={serviceCost}
              onChange={(e) => setServiceCost(e.target.value)}
              error={errors.serviceCost}
              className="text-right font-mono"
            />
          ) : (
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
          )}
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

        {isService && (
          <ServiceComponentsEditor
            rows={components}
            onChange={setComponents}
            serviceCost={serviceCost}
            serviceItemId={item?.itemId}
          />
        )}

        {!isService && (
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
        )}

        {isService ? null : isEdit ? (
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

        {isEdit && !isService && (
          <section
            data-testid="item-edit-lots"
            aria-label="Lots and Purchase Price"
            className="mt-2 pt-3 border-t border-slate-100 dark:border-slate-800"
          >
            <h3 className="text-xs font-semibold text-slate-700 dark:text-slate-200 mb-1">Lots &amp; Purchase Price</h3>
            <p className="text-[10px] text-slate-400 mb-2">
              Fix a wrong Purchase Price here. Past sale costs and the purchase bill are re-costed by the backend.
            </p>
            <LotInspector
              itemId={item.itemId}
              baseUnit={item.baseUnit}
              editable
              onChanged={onSaved}
              onModalToggle={(o) => {
                lotModalOpen.current = o;
              }}
            />
          </section>
        )}

        {isService ? null : isEdit ? (
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
