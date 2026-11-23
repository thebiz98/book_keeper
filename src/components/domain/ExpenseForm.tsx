import React, { useEffect, useRef, useState } from 'react';
import { api, errorMessage } from '@/api';
import { todayLocal } from '@/lib/dates';
import type { Expense, ExpenseType } from '@/types';
import { useBusyLock, useHotkeyAction } from '@/hooks/useHotkeys';
import { Button } from '../ui/Button';
import { NumericInput } from '../ui/NumericInput';
import { TextInput } from '../ui/TextInput';
import { ExpenseTypeField } from './ExpenseTypeField';
import { refreshAppMetrics } from './navParams';

export interface ExpenseFormProps {
  /** Expense being edited, or null for a new one. */
  editing: Expense | null;
  /** Bumped by the page after a list change so the code preview refreshes. */
  codeKey: number;
  onSaved: (e: Expense, wasEdit: boolean) => void;
  onCancelEdit: () => void;
  /** Receives the Type input so the page can focus it (F7). */
  typeInputRef: React.MutableRefObject<HTMLInputElement | null>;
}

/** New / edit expense form (FR-18). Enter moves Type -> Amount -> Notes -> save; Ctrl+Enter saves anywhere. */
export const ExpenseForm: React.FC<ExpenseFormProps> = ({ editing, codeKey, onSaved, onCancelEdit, typeInputRef }) => {
  const [date, setDate] = useState(todayLocal());
  const [type, setType] = useState<ExpenseType | null>(null);
  const [amount, setAmount] = useState(0);
  const [notes, setNotes] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [saved, setSaved] = useState<string | null>(null);
  const [code, setCode] = useState('');
  const [resetN, setResetN] = useState(0);
  const amountRef = useRef<HTMLInputElement | null>(null);
  const notesRef = useRef<HTMLInputElement | null>(null);

  useBusyLock(busy);

  useEffect(() => {
    if (editing) {
      setDate(editing.expenseDate);
      setType({ id: editing.expenseTypeId, name: editing.expenseTypeName, isActive: true, createdAt: editing.createdAt });
      setAmount(editing.amount);
      setNotes(editing.notes ?? '');
      setError(null);
      setSaved(null);
    }
  }, [editing]);

  useEffect(() => {
    if (editing) return;
    let live = true;
    api
      .getNextExpenseCode()
      .then((c) => live && setCode(c))
      .catch(() => live && setCode(''));
    return () => {
      live = false;
    };
  }, [editing, codeKey]);

  const reset = () => {
    // Move focus first: a focused NumericInput re-applies its text on blur, which must not undo the reset.
    typeInputRef.current?.focus();
    setType(null);
    setAmount(0);
    setResetN((n) => n + 1); // remount Amount so its displayed text (and blur re-format) is dropped
    setNotes('');
    setError(null);
  };

  const submit = async () => {
    if (busy) return;
    if (!type) {
      setError('Choose or create an expense type.');
      typeInputRef.current?.focus();
      return;
    }
    if (!(amount > 0)) {
      setError('Amount must be greater than 0.');
      amountRef.current?.focus();
      return;
    }
    setBusy(true);
    setError(null);
    const payload = { expenseDate: date, expenseTypeId: type.id, amount, notes: notes.trim() ? notes.trim() : null };
    try {
      const wasEdit = editing !== null;
      const e = editing ? await api.updateExpense(editing.id, payload) : await api.createExpense(payload);
      setSaved(`${e.expenseCode} ${wasEdit ? 'updated' : 'saved'}`);
      refreshAppMetrics();
      reset();
      onSaved(e, wasEdit);
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  };

  const clear = () => {
    if (editing) onCancelEdit();
    reset();
    setSaved(null);
  };

  useHotkeyAction('completeBill', () => void submit(), { scope: 'page' });
  useHotkeyAction('back', clear, { scope: 'page', allowInInputs: true });

  return (
    <form
      data-testid="expense-form"
      className="flex flex-col gap-3 bg-white dark:bg-slate-900 p-4 rounded-lg border border-slate-200 dark:border-slate-800 shadow-xs"
      onSubmit={(e) => {
        e.preventDefault();
        void submit();
      }}
    >
      <div className="flex items-center justify-between">
        <h2 className="text-sm font-bold text-slate-800 dark:text-slate-100">
          {editing ? `Editing ${editing.expenseCode}` : 'New Expense'}
        </h2>
        {!editing && code && (
          <span data-testid="expense-code-preview" className="font-mono text-xs text-slate-500">
            {code}
          </span>
        )}
      </div>

      <TextInput
        label="Date"
        type="date"
        data-testid="expense-date"
        value={date}
        onChange={(e) => setDate(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === 'Enter') {
            e.preventDefault();
            typeInputRef.current?.focus();
          }
        }}
      />

      <ExpenseTypeField
        value={type}
        onChange={setType}
        inputRef={typeInputRef}
        onPicked={() => setTimeout(() => amountRef.current?.focus(), 0)}
      />

      <NumericInput
        key={resetN}
        ref={amountRef}
        label="Amount (₹)"
        data-testid="expense-amount"
        value={amount}
        min={0}
        onChange={setAmount}
        onKeyDown={(e) => {
          if (e.key === 'Enter' && !e.ctrlKey) {
            e.preventDefault();
            notesRef.current?.focus();
          }
        }}
      />

      <TextInput
        ref={notesRef}
        label="Notes"
        data-testid="expense-notes"
        value={notes}
        onChange={(e) => setNotes(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === 'Enter' && !e.ctrlKey) {
            e.preventDefault();
            void submit();
          }
        }}
      />

      {error && (
        <div role="alert" data-testid="expense-error" className="text-xs text-rose-600 dark:text-rose-400">
          {error}
        </div>
      )}
      {saved && !error && (
        <div role="status" data-testid="expense-saved" className="text-xs text-emerald-600 dark:text-emerald-400">
          {saved}
        </div>
      )}

      <div className="flex items-center justify-end gap-2">
        <Button type="button" variant="outline" size="sm" onClick={clear} disabled={busy} hotkey="Esc">
          {editing ? 'Cancel edit' : 'Clear'}
        </Button>
        <Button type="submit" size="sm" isLoading={busy} hotkey="Enter" data-testid="expense-save">
          {editing ? 'Update' : 'Save Expense'}
        </Button>
      </div>
    </form>
  );
};

export default ExpenseForm;
