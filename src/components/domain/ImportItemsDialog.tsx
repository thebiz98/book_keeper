import React, { useEffect, useRef, useState } from 'react';
import { api } from '@/api';
import { errorMessage } from '@/api/errors';
import type { ConflictStrategy, ImportSummary, ImportValidationReport } from '@/types';
import { Modal } from '../ui/Modal';
import { Button } from '../ui/Button';
import { TextInput } from '../ui/TextInput';

export interface ImportItemsDialogProps {
  isOpen: boolean;
  onClose: () => void;
  /** Called after a successful import so the catalog can refresh. */
  onImported: () => void;
}

const STRATEGIES: { value: ConflictStrategy; label: string; hint: string }[] = [
  { value: 'SKIP', label: 'Skip', hint: 'Leave existing items untouched' },
  { value: 'OVERWRITE', label: 'Overwrite details', hint: 'Update name, price, MRP etc. of matches' },
  { value: 'ADD_STOCK', label: 'Add stock', hint: 'Keep details, add the file quantity as new stock' },
];

/**
 * Import flow: path -> dry-run report -> conflict strategy -> confirm -> summary.
 * There is no native file picker in this build, so the user types the absolute .xlsx/.csv path.
 */
export const ImportItemsDialog: React.FC<ImportItemsDialogProps> = ({ isOpen, onClose, onImported }) => {
  const [path, setPath] = useState('');
  const [report, setReport] = useState<ImportValidationReport | null>(null);
  const [validatedPath, setValidatedPath] = useState<string | null>(null);
  const [strategy, setStrategy] = useState<ConflictStrategy>('SKIP');
  const [summary, setSummary] = useState<ImportSummary | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState<'validate' | 'import' | null>(null);
  const lock = useRef(false);

  useEffect(() => {
    if (isOpen) {
      setPath('');
      setReport(null);
      setValidatedPath(null);
      setStrategy('SKIP');
      setSummary(null);
      setError(null);
      setBusy(null);
      lock.current = false;
    }
  }, [isOpen]);

  const trimmed = path.trim();
  const reportCurrent = report !== null && validatedPath === trimmed;

  const handleValidate = async () => {
    if (!trimmed || lock.current) return;
    lock.current = true;
    setBusy('validate');
    setError(null);
    setSummary(null);
    setReport(null);
    try {
      const r = await api.validateItemsImport(trimmed);
      setReport(r);
      setValidatedPath(trimmed);
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      lock.current = false;
      setBusy(null);
    }
  };

  const handleImport = async () => {
    if (!reportCurrent || lock.current) return;
    lock.current = true;
    setBusy('import');
    setError(null);
    try {
      const s = await api.executeItemsImport(trimmed, strategy);
      setSummary(s);
      onImported();
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      lock.current = false;
      setBusy(null);
    }
  };

  const canImport =
    reportCurrent && summary === null && (report.validCount > 0 || report.duplicates.length > 0);

  return (
    <Modal
      isOpen={isOpen}
      onClose={onClose}
      title="Import Items"
      description="Validate the file first, then choose how existing items are handled."
      size="xl"
      footer={
        <>
          <Button variant="outline" size="sm" onClick={onClose}>
            {summary ? 'Close' : 'Cancel'}
          </Button>
          {summary === null && (
            <Button
              variant="primary"
              size="sm"
              disabled={!canImport}
              isLoading={busy === 'import'}
              onClick={() => void handleImport()}
            >
              Confirm import
            </Button>
          )}
        </>
      }
    >
      <div className="flex flex-col gap-4 text-xs">
        <div className="flex items-end gap-2">
          <div className="flex-1">
            <TextInput
              label="File path (.xlsx / .csv)"
              placeholder="Absolute path, e.g. /home/user/items.xlsx"
              value={path}
              onChange={(e) => setPath(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === 'Enter') {
                  e.preventDefault();
                  void handleValidate();
                }
              }}
              autoFocus
            />
          </div>
          <Button
            variant="secondary"
            size="md"
            disabled={!trimmed}
            isLoading={busy === 'validate'}
            onClick={() => void handleValidate()}
          >
            Validate
          </Button>
        </div>
        <p className="text-[11px] text-slate-400 -mt-2">
          A native file picker is not available in this build: paste the full path to the file.
        </p>

        {report && (
          <div data-testid="import-report" className="flex flex-col gap-3">
            <div className="flex flex-wrap gap-3">
              <span className="px-2 py-1 rounded bg-emerald-50 text-emerald-700 dark:bg-emerald-950/50 dark:text-emerald-300 font-semibold">
                Valid rows: {report.validCount}
              </span>
              <span className="px-2 py-1 rounded bg-rose-50 text-rose-700 dark:bg-rose-950/50 dark:text-rose-300 font-semibold">
                Rows with errors: {report.rowsWithErrors.length}
              </span>
              <span className="px-2 py-1 rounded bg-amber-50 text-amber-700 dark:bg-amber-950/50 dark:text-amber-300 font-semibold">
                Duplicates: {report.duplicates.length}
              </span>
            </div>

            {report.rowsWithErrors.length > 0 && (
              <div className="max-h-40 overflow-auto rounded border border-slate-200 dark:border-slate-700">
                <table className="w-full">
                  <thead className="sticky top-0 bg-slate-50 dark:bg-slate-800 text-left text-slate-500">
                    <tr>
                      <th className="px-2 py-1 font-medium">Row</th>
                      <th className="px-2 py-1 font-medium">Field</th>
                      <th className="px-2 py-1 font-medium">Problem</th>
                    </tr>
                  </thead>
                  <tbody>
                    {report.rowsWithErrors.map((r, i) => (
                      <tr key={`${r.row}-${r.field}-${i}`} data-testid="import-error-row" className="border-t border-slate-100 dark:border-slate-800">
                        <td className="px-2 py-1 font-mono">{r.row}</td>
                        <td className="px-2 py-1 font-mono">{r.field}</td>
                        <td className="px-2 py-1">{r.message}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            )}

            {report.duplicates.length > 0 && (
              <div className="max-h-32 overflow-auto rounded border border-slate-200 dark:border-slate-700">
                <table className="w-full">
                  <thead className="sticky top-0 bg-slate-50 dark:bg-slate-800 text-left text-slate-500">
                    <tr>
                      <th className="px-2 py-1 font-medium">Row</th>
                      <th className="px-2 py-1 font-medium">Name</th>
                      <th className="px-2 py-1 font-medium">Existing item #</th>
                    </tr>
                  </thead>
                  <tbody>
                    {report.duplicates.map((d) => (
                      <tr key={`${d.row}-${d.existingItemId}`} data-testid="import-duplicate-row" className="border-t border-slate-100 dark:border-slate-800">
                        <td className="px-2 py-1 font-mono">{d.row}</td>
                        <td className="px-2 py-1">{d.name}</td>
                        <td className="px-2 py-1 font-mono">{d.existingItemId}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            )}

            <fieldset className="flex flex-col gap-1.5" disabled={summary !== null}>
              <legend className="font-semibold text-slate-700 dark:text-slate-300 mb-1">
                When an item already exists
              </legend>
              {STRATEGIES.map((s) => (
                <label key={s.value} className="flex items-start gap-2 cursor-pointer">
                  <input
                    type="radio"
                    name="import-strategy"
                    value={s.value}
                    checked={strategy === s.value}
                    onChange={() => setStrategy(s.value)}
                    className="mt-0.5"
                  />
                  <span>
                    <span className="font-medium">{s.label}</span>
                    <span className="text-slate-400"> : {s.hint}</span>
                  </span>
                </label>
              ))}
            </fieldset>
          </div>
        )}

        {summary && (
          <div
            role="status"
            data-testid="import-summary"
            className="rounded-md border border-emerald-200 bg-emerald-50 dark:bg-emerald-950/40 dark:border-emerald-800 px-3 py-2 text-emerald-800 dark:text-emerald-200"
          >
            Import complete. Created {summary.created}, updated {summary.updated}, skipped {summary.skipped}, stock
            lots created {summary.stockLotsCreated}.
          </div>
        )}

        {error && (
          <p role="alert" className="text-rose-600 dark:text-rose-400 font-medium">
            {error}
          </p>
        )}
      </div>
    </Modal>
  );
};

export default ImportItemsDialog;
