import React, { useEffect, useRef, useState } from 'react';
import { Upload, Wallet } from 'lucide-react';
import type { Expense } from '@/types';
import { useHotkeyAction } from '@/hooks/useHotkeys';
import { ExpenseForm } from '@/components/domain/ExpenseForm';
import { ExpenseList } from '@/components/domain/ExpenseList';
import { ImportDialog } from '@/components/domain/ImportDialog';
import { Button } from '@/components/ui/Button';
import { useAppStore } from '@/store/useAppStore';

/** Store Expenses (Page 13, F7): entry form on the left, list on the right. */
export const ExpensesPage: React.FC = () => {
  const [editing, setEditing] = useState<Expense | null>(null);
  const [reloadKey, setReloadKey] = useState(0);
  const [importOpen, setImportOpen] = useState(false);
  const refreshMetrics = useAppStore((s) => s.refreshMetrics);
  const typeInputRef = useRef<HTMLInputElement | null>(null);

  useEffect(() => {
    typeInputRef.current?.focus();
  }, []);

  // F7 while already here: back to the Type field (outranks the global navigation handler).
  useHotkeyAction('expenses', () => typeInputRef.current?.focus(), { scope: 'page' });

  const bump = () => setReloadKey((k) => k + 1);

  return (
    <div className="flex flex-col gap-4 h-full min-h-0">
      <div className="flex items-center justify-between gap-3">
        <h1 className="text-xl font-bold flex items-center gap-2">
          <Wallet className="text-blue-600 dark:text-blue-400" size={24} />
          Store Expenses
        </h1>
        <Button variant="outline" size="sm" leftIcon={<Upload className="h-3.5 w-3.5" />} onClick={() => setImportOpen(true)}>
          Import
        </Button>
      </div>
      <ImportDialog
        kind="EXPENSES"
        isOpen={importOpen}
        onClose={() => setImportOpen(false)}
        onImported={() => {
          bump();
          void refreshMetrics();
        }}
      />
      <div className="flex flex-col lg:flex-row gap-4 flex-1 min-h-0">
        <div className="lg:w-[360px] shrink-0">
          <ExpenseForm
            editing={editing}
            codeKey={reloadKey}
            typeInputRef={typeInputRef}
            onSaved={() => {
              setEditing(null);
              bump();
            }}
            onCancelEdit={() => setEditing(null)}
          />
        </div>
        <div className="flex-1 min-w-0 flex flex-col min-h-0">
          <ExpenseList
            reloadKey={reloadKey}
            onEdit={(e) => setEditing(e)}
            onDeleted={(e) => {
              if (editing?.id === e.id) setEditing(null);
              bump();
            }}
          />
        </div>
      </div>
    </div>
  );
};

export default ExpensesPage;
