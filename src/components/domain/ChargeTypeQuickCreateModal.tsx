import React, { useEffect, useRef, useState } from 'react';
import { api, errorMessage } from '@/api';
import type { ChargeAppliesTo, ChargeEffect, ChargeType, TransactionType } from '@/types';
import { Modal } from '../ui/Modal';
import { TextInput } from '../ui/TextInput';
import { Select } from '../ui/Select';
import { FormActions, enterToSubmit } from '../ui/FormActions';

export interface ChargeTypeQuickCreateModalProps {
  isOpen: boolean;
  initialName: string;
  /** The bill side being edited; "Applies to" defaults to Both. */
  side: TransactionType;
  onClose: () => void;
  onCreated: (ct: ChargeType) => void;
}

/** "New Charge / Discount Type" (FR-19, FR-22): visible Save and Cancel, Enter = Save, Esc = Cancel. */
export const ChargeTypeQuickCreateModal: React.FC<ChargeTypeQuickCreateModalProps> = ({
  isOpen,
  initialName,
  side,
  onClose,
  onCreated,
}) => {
  const [name, setName] = useState('');
  const [effect, setEffect] = useState<ChargeEffect>('CHARGE');
  const [appliesTo, setAppliesTo] = useState<ChargeAppliesTo>('BOTH');
  const [defaultAmount, setDefaultAmount] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [fieldErrors, setFieldErrors] = useState<{ name?: string; defaultAmount?: string }>({});
  const [saving, setSaving] = useState(false);
  const submitting = useRef(false);

  useEffect(() => {
    if (!isOpen) return;
    setName(initialName);
    setEffect('CHARGE');
    setAppliesTo('BOTH');
    setDefaultAmount('');
    setError(null);
    setFieldErrors({});
    submitting.current = false;
  }, [isOpen, initialName, side]);

  const save = async () => {
    if (submitting.current) return;
    const errs: { name?: string; defaultAmount?: string } = {};
    const amt = defaultAmount.trim() === '' ? null : Number(defaultAmount);
    if (!name.trim()) errs.name = 'Name is required';
    if (amt !== null && (!Number.isFinite(amt) || amt <= 0)) errs.defaultAmount = 'Must be greater than 0 (or leave empty)';
    setFieldErrors(errs);
    if (Object.keys(errs).length > 0) return;
    submitting.current = true;
    setSaving(true);
    setError(null);
    try {
      const ct = await api.createChargeType({ name: name.trim(), effect, appliesTo, defaultAmount: amt });
      onCreated(ct);
    } catch (e) {
      const msg = errorMessage(e);
      if (/name/i.test(msg) && /exists/i.test(msg)) setFieldErrors({ name: msg });
      else setError(msg);
    } finally {
      submitting.current = false;
      setSaving(false);
    }
  };

  return (
    <Modal
      isOpen={isOpen}
      onClose={onClose}
      title="New Charge / Discount Type"
      description="Reusable on sale and purchase bills"
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
          label="Name *"
          data-testid="charge-type-name"
          value={name}
          onChange={(e) => setName(e.target.value)}
          error={fieldErrors.name}
          autoFocus
          autoComplete="off"
        />
        <fieldset className="flex items-center gap-4 text-sm" data-testid="charge-type-effect">
          <legend className="text-xs font-medium text-slate-600 dark:text-slate-300 mb-1">Effect</legend>
          <label className="flex items-center gap-1.5 cursor-pointer">
            <input type="radio" name="charge-effect" checked={effect === 'CHARGE'} onChange={() => setEffect('CHARGE')} />
            Charge (adds)
          </label>
          <label className="flex items-center gap-1.5 cursor-pointer">
            <input type="radio" name="charge-effect" checked={effect === 'DISCOUNT'} onChange={() => setEffect('DISCOUNT')} />
            Discount (subtracts)
          </label>
        </fieldset>
        <Select<ChargeAppliesTo>
          label="Applies to"
          value={appliesTo}
          onChange={setAppliesTo}
          options={[
            { value: 'BOTH', label: 'Sale and Purchase' },
            { value: 'SALE', label: 'Sale only' },
            { value: 'PURCHASE', label: 'Purchase only' },
          ]}
        />
        <TextInput
          label="Default amount (optional)"
          data-testid="charge-type-default"
          type="number"
          min={0}
          step="any"
          value={defaultAmount}
          onChange={(e) => setDefaultAmount(e.target.value)}
          error={fieldErrors.defaultAmount}
          className="text-right font-mono"
        />
        {error && (
          <p role="alert" className="text-sm text-red-600">
            {error}
          </p>
        )}
      </div>
    </Modal>
  );
};

export default ChargeTypeQuickCreateModal;
