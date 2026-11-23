import React, { useRef, useState } from 'react';
import { api, errorMessage } from '@/api';
import type { ExpenseType } from '@/types';
import { useEscapeLayer } from '@/hooks/useHotkeys';
import { LiveSearchTypeahead } from '../common/LiveSearchTypeahead';
import { TextInput } from '../ui/TextInput';
import { FormActions, enterToSubmit } from '../ui/FormActions';

export interface ExpenseTypeFieldProps {
  value: ExpenseType | null;
  onChange: (t: ExpenseType | null) => void;
  /** Called after a type was picked or created (the form moves focus to Amount). */
  onPicked?: () => void;
  autoFocus?: boolean;
  inputRef?: React.MutableRefObject<HTMLInputElement | null>;
}

/**
 * Expense type picker (FR-18): live FTS typeahead over the type master with an inline
 * `[+ Create "..."]` row. Creating is idempotent on the backend (an existing name returns the
 * existing type), and the create panel has visible Save / Cancel buttons (FR-22).
 */
export const ExpenseTypeField: React.FC<ExpenseTypeFieldProps> = ({
  value,
  onChange,
  onPicked,
  autoFocus,
  inputRef,
}) => {
  const [creating, setCreating] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const localRef = useRef<HTMLInputElement | null>(null);
  const typeRef = inputRef ?? localRef;

  const cancelCreate = () => {
    setCreating(null);
    setError(null);
    setTimeout(() => typeRef.current?.focus(), 0);
  };
  useEscapeLayer(cancelCreate, creating !== null);

  const saveCreate = async () => {
    if (creating === null || saving) return;
    setSaving(true);
    setError(null);
    try {
      const t = await api.createExpenseType(creating);
      setCreating(null);
      onChange(t);
      onPicked?.();
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setSaving(false);
    }
  };

  const pick = (t: ExpenseType) => {
    onChange(t);
    onPicked?.();
  };

  return (
    <div className="flex flex-col gap-1.5">
      <label className="text-xs font-medium text-slate-600 dark:text-slate-300">Type</label>
      <LiveSearchTypeahead<ExpenseType>
        search={(q) => api.searchExpenseTypes(q, 8)}
        getKey={(t) => t.id}
        getLabel={(t) => t.name}
        onSelect={pick}
        onCreate={(q) => {
          setError(null);
          setCreating(q);
        }}
        createLabel={(q) => `+ Create "${q}"`}
        renderItem={(t) => (
          <div data-testid="expense-type-option" className="px-3 py-2 text-sm">
            {t.name}
          </div>
        )}
        ariaLabel="Expense type"
        testId="expense-type-input"
        placeholder="Search or create a type..."
        autoFocus={autoFocus}
        inputRef={typeRef}
        disabled={creating !== null}
      />
      {creating !== null ? (
        <div
          data-testid="expense-type-create"
          className="rounded-md border border-blue-200 dark:border-blue-900 bg-blue-50/50 dark:bg-blue-950/20 p-2.5 flex flex-col gap-2"
          onKeyDown={enterToSubmit(() => void saveCreate())}
        >
          <TextInput
            label="New expense type"
            aria-label="New expense type name"
            data-testid="expense-type-name"
            autoFocus
            value={creating}
            onChange={(e) => setCreating(e.target.value)}
            error={error ?? undefined}
          />
          <FormActions
            onSave={() => void saveCreate()}
            onCancel={cancelCreate}
            isSubmitting={saving}
            canSave={creating.trim() !== ''}
            saveLabel="Save type"
          />
        </div>
      ) : (
        <div className="text-xs text-slate-500" data-testid="expense-type-selected">
          {value ? (
            <>
              Selected: <span className="font-semibold text-slate-800 dark:text-slate-100">{value.name}</span>{' '}
              <button type="button" className="underline cursor-pointer" onClick={() => onChange(null)}>
                change
              </button>
            </>
          ) : (
            'No type selected'
          )}
        </div>
      )}
    </div>
  );
};

export default ExpenseTypeField;
