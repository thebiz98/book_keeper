import React, { useEffect, useRef, useState } from 'react';
import { api, errorMessage } from '@/api';
import type { ItemSearchResult } from '@/types';
import { Modal } from '../ui/Modal';
import { TextInput } from '../ui/TextInput';
import { FormActions, enterToSubmit } from '../ui/FormActions';
import { validationField } from './formUtils';
import { FIELD_MAP, validateItemForm, type ItemFormErrors } from './itemForm';

export interface ItemQuickCreateModalProps {
  isOpen: boolean;
  initialName: string;
  /** New Sale: stock on hand and purchase price are required (opening-stock lot). New Purchase: hidden. */
  withStock: boolean;
  onClose: () => void;
  onCreated: (item: ItemSearchResult) => void;
}

const LOW_STOCK_DEFAULT = 5;

/** "New Item" quick create (FR-24, DD-50): visible Save and Cancel, Enter = Save, Esc = Cancel. */
export const ItemQuickCreateModal: React.FC<ItemQuickCreateModalProps> = ({
  isOpen,
  initialName,
  withStock,
  onClose,
  onCreated,
}) => {
  const [name, setName] = useState('');
  const [baseUnit, setBaseUnit] = useState('PCS');
  const [mrp, setMrp] = useState('');
  const [price, setPrice] = useState('');
  const [stock, setStock] = useState('');
  const [cost, setCost] = useState('');
  const [errors, setErrors] = useState<ItemFormErrors>({});
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const submitting = useRef(false);

  useEffect(() => {
    if (!isOpen) return;
    setName(initialName);
    setBaseUnit('PCS');
    setMrp('');
    setPrice('');
    setStock('');
    setCost('');
    setErrors({});
    setError(null);
    submitting.current = false;
  }, [isOpen, initialName, withStock]);

  const save = async () => {
    if (submitting.current) return;
    const { errors: errs, values } = validateItemForm(
      {
        name,
        baseUnit,
        mrp,
        price,
        lowStock: String(LOW_STOCK_DEFAULT),
        initialStock: stock,
        initialCost: cost,
      },
      withStock ? 'quick-stock' : 'quick'
    );
    setErrors(errs);
    if (!values) return;

    submitting.current = true;
    setSaving(true);
    setError(null);
    const itemName = name.trim();
    const unit = baseUnit.trim().toUpperCase();
    try {
      const itemId = await api.createItem({
        name: itemName,
        category: null,
        barcode: null,
        baseUnit: unit,
        mrp: values.mrp,
        defaultSalePrice: values.price,
        lowStockThreshold: LOW_STOCK_DEFAULT,
        initialStock: withStock ? values.stock : null,
        initialCost: withStock ? values.cost : null,
        bundles: null,
      });
      const rows = await api.searchItemsFts(itemName, 8).catch(() => [] as ItemSearchResult[]);
      const found =
        rows.find((r) => r.itemId === itemId) ??
        ({
          itemId,
          name: itemName,
          category: null,
          barcode: null,
          baseUnit: unit,
          itemType: 'PHYSICAL',
          mrp: values.mrp,
          defaultSalePrice: values.price,
          currentStock: withStock ? (values.stock ?? 0) : 0,
          isLowStock: false,
          lowStockThreshold: LOW_STOCK_DEFAULT,
          bundles: [],
          matchedBundleId: null,
        } satisfies ItemSearchResult);
      onCreated(found);
    } catch (e) {
      const f = validationField(e);
      const key = f ? FIELD_MAP[f] : undefined;
      if (key) setErrors({ [key]: errorMessage(e) });
      else setError(errorMessage(e) || 'Failed to create item');
    } finally {
      submitting.current = false;
      setSaving(false);
    }
  };

  return (
    <Modal
      isOpen={isOpen}
      onClose={onClose}
      title="New Item"
      description={withStock ? 'Added to the catalog with opening stock' : 'Added to the catalog'}
      size="md"
      footer={
        <FormActions
          onSave={() => void save()}
          onCancel={onClose}
          isSubmitting={saving}
          canSave={name.trim().length > 0}
          saveLabel="Save"
        />
      }
    >
      <div className="flex flex-col gap-3 py-1" onKeyDown={enterToSubmit(() => void save())}>
        <TextInput
          label="Item Name *"
          data-testid="quick-item-name"
          value={name}
          onChange={(e) => setName(e.target.value)}
          error={errors.name}
          autoFocus
          autoComplete="off"
        />
        <div className="grid grid-cols-3 gap-3">
          <TextInput
            label="Base Unit *"
            data-testid="quick-item-unit"
            value={baseUnit}
            onChange={(e) => setBaseUnit(e.target.value.toUpperCase())}
            error={errors.baseUnit}
          />
          <TextInput
            label="MRP (₹, optional)"
            data-testid="quick-item-mrp"
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
            data-testid="quick-item-price"
            type="number"
            min={0}
            step="any"
            value={price}
            onChange={(e) => setPrice(e.target.value)}
            error={errors.defaultSalePrice}
            className="text-right font-mono"
          />
        </div>
        {withStock && (
          <div className="grid grid-cols-2 gap-3">
            <TextInput
              label="Stock on hand *"
              data-testid="quick-item-stock"
              type="number"
              min={0}
              step="any"
              value={stock}
              onChange={(e) => setStock(e.target.value)}
              error={errors.initialStock}
              className="text-right font-mono"
            />
            <TextInput
              label="Purchase Price / base unit (₹) *"
              data-testid="quick-item-cost"
              type="number"
              min={0}
              step="any"
              value={cost}
              onChange={(e) => setCost(e.target.value)}
              error={errors.initialCost}
              className="text-right font-mono"
            />
          </div>
        )}
        {error && (
          <p role="alert" className="text-sm text-red-600">
            {error}
          </p>
        )}
      </div>
    </Modal>
  );
};

export default ItemQuickCreateModal;
