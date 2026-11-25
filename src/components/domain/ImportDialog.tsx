import React, { useCallback, useEffect, useRef, useState } from 'react';
import { api } from '@/api';
import { errorMessage } from '@/api/errors';
import {
  documentsPath,
  hasNativeFiles,
  onFileDrop,
  pickFile,
  pickSavePath,
  revealInFolder,
  SPREADSHEET_FILTERS,
  VEDBILL_FILTERS,
} from '@/api/fileDialog';
import type { ConflictStrategy, ImportKind, ImportSummary, ImportValidationReport, TemplateFormat } from '@/types';
import { Modal } from '../ui/Modal';
import { Button } from '../ui/Button';
import { TextInput } from '../ui/TextInput';

/** Kinds the dialog opens for. Service components are a source inside the items dialog. */
export type ImportDialogKind = 'ITEMS' | 'PARTIES' | 'EXPENSES';

export interface ImportDialogProps {
  isOpen: boolean;
  onClose: () => void;
  kind: ImportDialogKind;
  /** Called after a successful import so the page can refresh. */
  onImported: () => void;
}

export type ImportSource = 'SPREADSHEET' | 'VEDBILL' | 'COMPONENTS';

interface Strategy {
  value: ConflictStrategy;
  label: string;
  hint: string;
}

const ITEM_STRATEGIES: Strategy[] = [
  { value: 'SKIP', label: 'Skip', hint: 'Leave existing items untouched' },
  { value: 'OVERWRITE', label: 'Overwrite details', hint: 'Update price, MRP, category etc. of matches (stock is not changed)' },
  { value: 'ADD_STOCK', label: 'Add stock', hint: 'Keep details, add the imported quantity as new stock' },
];

const PARTY_STRATEGIES: Strategy[] = [
  { value: 'SKIP', label: 'Skip', hint: 'Leave existing parties untouched' },
  { value: 'OVERWRITE', label: 'Update', hint: 'Update phone, notes and opening balance of matches' },
];

interface KindConfig {
  title: string;
  /** Singular, lower case: "one item per row". */
  noun: string;
  /** Column heading for the name in the report tables. */
  nameLabel: string;
  existsLabel: string;
  templateBasename: string;
}

const CONFIG: Record<ImportDialogKind, KindConfig> = {
  ITEMS: {
    title: 'Import Items',
    noun: 'item',
    nameLabel: 'Item',
    existsLabel: 'Already in Book Keeper',
    templateBasename: 'book_keeper_items_template',
  },
  PARTIES: {
    title: 'Import Parties',
    noun: 'party',
    nameLabel: 'Party',
    existsLabel: 'Already in Book Keeper',
    templateBasename: 'book_keeper_parties_template',
  },
  EXPENSES: {
    title: 'Import Expenses',
    noun: 'expense',
    nameLabel: 'Expense type',
    existsLabel: 'Already in Book Keeper',
    templateBasename: 'book_keeper_expenses_template',
  },
};

const SPREADSHEET_EXT = /\.(xlsx|xls|csv)$/i;
const VEDBILL_EXT = /\.vb$/i;
const VEDBILL_FOLDER = 'VEDBIL';
const COMPONENTS_BASENAME = 'book_keeper_service_components_template';

/** Drag-and-drop and pasted paths may arrive quoted (Explorer "Copy as path"). */
function cleanPath(p: string): string {
  return p.trim().replace(/^"(.*)"$/, '$1').trim();
}

/**
 * Import flow shared by Items, Parties and Expenses (FR-41): pick / drop / paste a file ->
 * automatic dry-run report -> conflict strategy (where it applies) -> confirm -> summary.
 * Items also import a VedBill backup (.vb) and a service components CSV.
 */
export const ImportDialog: React.FC<ImportDialogProps> = ({ isOpen, onClose, kind, onImported }) => {
  const cfg = CONFIG[kind];
  const isItems = kind === 'ITEMS';
  const strategies = kind === 'PARTIES' ? PARTY_STRATEGIES : ITEM_STRATEGIES;

  const sourceForPath = useCallback(
    (p: string, fallback: ImportSource): ImportSource | null => {
      if (isItems && VEDBILL_EXT.test(p)) return 'VEDBILL';
      if (SPREADSHEET_EXT.test(p)) return fallback === 'COMPONENTS' ? 'COMPONENTS' : 'SPREADSHEET';
      return null;
    },
    [isItems]
  );

  const [source, setSource] = useState<ImportSource>('SPREADSHEET');
  const [path, setPath] = useState('');
  const [report, setReport] = useState<ImportValidationReport | null>(null);
  const [validated, setValidated] = useState<{ path: string; source: ImportSource } | null>(null);
  const [strategy, setStrategy] = useState<ConflictStrategy>('SKIP');
  const [summary, setSummary] = useState<ImportSummary | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState<'validate' | 'import' | 'template' | null>(null);
  const [dragOver, setDragOver] = useState(false);
  const [templateSaved, setTemplateSaved] = useState<string | null>(null);
  const importLock = useRef(false);
  const requestId = useRef(0);
  const debounce = useRef<ReturnType<typeof setTimeout> | null>(null);
  const native = hasNativeFiles();

  useEffect(() => {
    if (isOpen) {
      setSource('SPREADSHEET');
      setPath('');
      setReport(null);
      setValidated(null);
      setStrategy('SKIP');
      setSummary(null);
      setError(null);
      setBusy(null);
      setDragOver(false);
      setTemplateSaved(null);
      importLock.current = false;
    }
  }, [isOpen]);

  useEffect(() => () => {
    if (debounce.current) clearTimeout(debounce.current);
  }, []);

  const trimmed = cleanPath(path);
  const reportCurrent = report !== null && validated?.path === trimmed && validated.source === source;
  const usesStrategy = source !== 'COMPONENTS' && kind !== 'EXPENSES';

  const validate = useCallback(
    async (p: string, src: ImportSource, strat: ConflictStrategy) => {
      const target = cleanPath(p);
      if (!target) return;
      if (debounce.current) clearTimeout(debounce.current);
      const id = ++requestId.current;
      setBusy('validate');
      setError(null);
      setSummary(null);
      setReport(null);
      setValidated(null);
      try {
        let r: ImportValidationReport;
        if (kind === 'PARTIES') r = await api.validatePartiesImport(target, strat);
        else if (kind === 'EXPENSES') r = await api.validateExpensesImport(target);
        else if (src === 'VEDBILL') r = await api.validateVedbillImport(target);
        else if (src === 'COMPONENTS') r = await api.validateServiceComponentsImport(target);
        else r = await api.validateItemsImport(target);
        if (id !== requestId.current) return;
        setReport(r);
        setValidated({ path: target, source: src });
      } catch (e) {
        if (id !== requestId.current) return;
        setError(errorMessage(e));
      } finally {
        if (id === requestId.current) setBusy(null);
      }
    },
    [kind]
  );

  /** A path chosen by picker, drop or paste: switch source by extension and validate at once. */
  const acceptFile = useCallback(
    (p: string, fallback: ImportSource) => {
      const target = cleanPath(p);
      const src = sourceForPath(target, fallback) ?? fallback;
      setSource(src);
      setPath(target);
      void validate(target, src, strategy);
    },
    [validate, sourceForPath, strategy]
  );

  // OS file drop (Tauri gives real paths).
  useEffect(() => {
    if (!isOpen || !native) return;
    let unlisten: (() => void) | null = null;
    let cancelled = false;
    void onFileDrop((s) => {
      if (s === 'over') setDragOver(true);
      else if (s === 'leave') setDragOver(false);
      else {
        setDragOver(false);
        const first = s.dropped[0];
        if (!first) return;
        if (!sourceForPath(first, 'SPREADSHEET')) {
          setError(
            isItems
              ? 'That file type is not supported. Drop a .xlsx or .csv spreadsheet, or a VedBill .vb backup.'
              : 'That file type is not supported. Drop a .xlsx or .csv spreadsheet.'
          );
          return;
        }
        acceptFile(first, source === 'COMPONENTS' ? 'COMPONENTS' : 'SPREADSHEET');
      }
    }).then((u) => {
      if (cancelled) u();
      else unlisten = u;
    });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [isOpen, native, acceptFile, sourceForPath, isItems, source]);

  const browse = async (src: ImportSource) => {
    try {
      const picked =
        src === 'VEDBILL'
          ? await pickFile(VEDBILL_FILTERS, await documentsPath(VEDBILL_FOLDER), 'Choose a VedBill backup (.vb)')
          : await pickFile(
              SPREADSHEET_FILTERS,
              await documentsPath(),
              src === 'COMPONENTS' ? 'Choose the service components file' : `Choose the ${cfg.noun}s spreadsheet`
            );
      if (picked) acceptFile(picked, src);
    } catch (e) {
      setError(errorMessage(e));
    }
  };

  const chooseSource = (src: ImportSource) => {
    if (src === source && report) return;
    requestId.current++;
    setSource(src);
    setPath('');
    setReport(null);
    setValidated(null);
    setSummary(null);
    setError(null);
    setBusy(null);
    // VedBill: open the file picker straight away.
    if (src === 'VEDBILL' && native) void browse('VEDBILL');
  };

  const onPathChange = (value: string) => {
    setPath(value);
    if (debounce.current) clearTimeout(debounce.current);
    const target = cleanPath(value);
    // Typed/pasted full path with a known extension: validate after a short pause.
    if (sourceForPath(target, source)) {
      debounce.current = setTimeout(() => acceptFile(target, source), 600);
    }
  };

  /** The conflict strategy changes what the dry run says for parties (below-settled checks). */
  const onStrategyChange = (value: ConflictStrategy) => {
    setStrategy(value);
    if (kind === 'PARTIES' && validated && validated.path === trimmed) {
      void validate(validated.path, validated.source, value);
    }
  };

  const templateKind: ImportKind = source === 'COMPONENTS' ? 'SERVICE_COMPONENTS' : kind;

  const downloadTemplate = async (format: TemplateFormat) => {
    const ext = format === 'XLSX' ? 'xlsx' : 'csv';
    setError(null);
    setTemplateSaved(null);
    try {
      const base = source === 'COMPONENTS' ? COMPONENTS_BASENAME : cfg.templateBasename;
      const fileName = `${base}.${ext}`;
      const suggested = (await documentsPath(fileName)) ?? fileName;
      const dest = native
        ? await pickSavePath(
            [format === 'XLSX' ? { name: 'Excel workbook', extensions: ['xlsx'] } : { name: 'CSV file', extensions: ['csv'] }],
            suggested,
            `Save the ${source === 'COMPONENTS' ? 'service components' : `${cfg.noun}s`} import template`
          )
        : suggested;
      if (!dest) return;
      setBusy('template');
      const saved =
        templateKind === 'ITEMS'
          ? await api.writeItemsImportTemplate(dest, format)
          : await api.writeImportTemplate(templateKind, dest, format);
      setTemplateSaved(saved);
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy((b) => (b === 'template' ? null : b));
    }
  };

  const handleImport = async () => {
    if (!reportCurrent || importLock.current) return;
    importLock.current = true;
    setBusy('import');
    setError(null);
    try {
      let s: ImportSummary;
      if (kind === 'PARTIES') s = await api.executePartiesImport(trimmed, strategy);
      else if (kind === 'EXPENSES') s = await api.executeExpensesImport(trimmed);
      else if (source === 'VEDBILL') s = await api.executeVedbillImport(trimmed, strategy);
      else if (source === 'COMPONENTS') s = await api.executeServiceComponentsImport(trimmed);
      else s = await api.executeItemsImport(trimmed, strategy);
      setSummary(s);
      onImported();
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      importLock.current = false;
      setBusy(null);
    }
  };

  const headerIssues = report?.headerIssues ?? [];
  const warnings = report?.warnings ?? [];
  const info = report?.info ?? [];
  const hasBlockingErrors = headerIssues.length > 0 || (report?.rowsWithErrors.length ?? 0) > 0;
  const canImport =
    reportCurrent &&
    summary === null &&
    !hasBlockingErrors &&
    (report.validCount > 0 || report.duplicates.length > 0);
  const showsStock = isItems && source !== 'COMPONENTS';
  const nameLabel = source === 'COMPONENTS' ? 'Service' : cfg.nameLabel;
  const sourceOptions: [ImportSource, string][] = [
    ['SPREADSHEET', 'Spreadsheet (.xlsx / .csv)'],
    ['COMPONENTS', 'Service components (CSV)'],
    ['VEDBILL', 'VedBill backup (.vb)'],
  ];

  const templateButtons = (
    <span className="inline-flex gap-1.5">
      <Button variant="outline" size="sm" isLoading={busy === 'template'} onClick={() => void downloadTemplate('XLSX')}>
        Excel template
      </Button>
      <Button variant="outline" size="sm" disabled={busy === 'template'} onClick={() => void downloadTemplate('CSV')}>
        CSV template
      </Button>
    </span>
  );

  return (
    <Modal
      isOpen={isOpen}
      onClose={onClose}
      title={cfg.title}
      description="Pick a file; it is checked automatically. Nothing is saved until you confirm."
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
        {isItems && (
          <div role="radiogroup" aria-label="Import from" className="inline-flex self-start rounded-md border border-slate-200 dark:border-slate-700 p-0.5">
            {sourceOptions.map(([value, label]) => (
              <button
                key={value}
                type="button"
                role="radio"
                aria-checked={source === value}
                data-testid={`import-source-${value}`}
                onClick={() => chooseSource(value)}
                className={`px-3 py-1.5 rounded font-medium transition-colors ${
                  source === value
                    ? 'bg-indigo-600 text-white'
                    : 'text-slate-600 hover:bg-slate-100 dark:text-slate-300 dark:hover:bg-slate-800'
                }`}
              >
                {label}
              </button>
            ))}
          </div>
        )}

        {source === 'VEDBILL' ? (
          <p className="text-slate-500 dark:text-slate-400 -mt-1">
            Brings over the item list and current stock (each VedBill stock lot with its own cost and date). Services
            come over as service items. Sales, purchases and parties are not imported. Backups are usually in
            Documents\VEDBIL.
          </p>
        ) : (
          <div className="flex flex-wrap items-center justify-between gap-2 -mt-1">
            <p className="text-slate-500 dark:text-slate-400">
              {source === 'COMPONENTS'
                ? 'One line per component of a service. The services must already exist; a service in the file gets exactly the components listed.'
                : `Download a template, fill one ${cfg.noun} per row, then drop the file here.`}
            </p>
            {templateButtons}
          </div>
        )}

        {templateSaved && (
          <p role="status" data-testid="template-saved" className="text-emerald-700 dark:text-emerald-300 -mt-2">
            Template saved to <span className="font-mono break-all">{templateSaved}</span>.{' '}
            {native && (
              <button type="button" className="underline font-medium" onClick={() => void revealInFolder(templateSaved)}>
                Open folder
              </button>
            )}
          </p>
        )}

        <div
          data-testid="import-dropzone"
          className={`rounded-lg border-2 border-dashed px-4 py-5 text-center transition-colors ${
            dragOver
              ? 'border-indigo-500 bg-indigo-50 dark:bg-indigo-950/40'
              : 'border-slate-300 dark:border-slate-700'
          }`}
        >
          <p className="font-medium text-slate-700 dark:text-slate-200">
            {dragOver
              ? 'Drop to check this file'
              : native
                ? source === 'VEDBILL'
                  ? 'Drop the .vb backup here, or'
                  : 'Drop the .xlsx / .csv file here, or'
                : 'Paste the full path to the file below'}
          </p>
          {native && (
            <Button variant="secondary" size="sm" className="mt-2" onClick={() => void browse(source)}>
              {source === 'VEDBILL' ? 'Choose backup file…' : 'Choose file…'}
            </Button>
          )}
        </div>

        <div className="flex items-end gap-2">
          <div className="flex-1">
            <TextInput
              label={source === 'VEDBILL' ? 'Backup file (.vb)' : 'File (.xlsx / .csv)'}
              placeholder={source === 'VEDBILL' ? 'Path to the .vb backup' : 'Path to the .xlsx or .csv file'}
              value={path}
              onChange={(e) => onPathChange(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === 'Enter') {
                  e.preventDefault();
                  void validate(path, source, strategy);
                }
              }}
            />
          </div>
          <Button
            variant="secondary"
            size="md"
            disabled={!trimmed}
            isLoading={busy === 'validate'}
            onClick={() => void validate(path, source, strategy)}
          >
            {report ? 'Check again' : 'Check file'}
          </Button>
        </div>

        {busy === 'validate' && (
          <p role="status" className="text-slate-500 -mt-2">
            Checking the file…
          </p>
        )}

        {report && headerIssues.length > 0 && (
          <div
            data-testid="import-header-issues"
            className="flex flex-col gap-2 rounded-md border border-rose-200 bg-rose-50 dark:bg-rose-950/30 dark:border-rose-900 px-3 py-2"
          >
            <p className="font-semibold text-rose-700 dark:text-rose-300">
              Fix the column headers in row 1 of your file, save it, then press “Check again”.
            </p>
            <ul className="list-disc pl-5 flex flex-col gap-1 text-rose-800 dark:text-rose-200">
              {headerIssues.map((h, i) => (
                <li key={`${h.kind}-${h.column}-${i}`} data-testid="import-header-issue">
                  {h.message}
                </li>
              ))}
            </ul>
            {report.expectedColumns.length > 0 && (
              <div className="flex flex-col gap-1">
                <span className="text-slate-600 dark:text-slate-300">
                  Expected columns (<span className="font-semibold">*</span> required; order does not matter):
                </span>
                <div className="flex flex-wrap gap-1">
                  {report.expectedColumns.map((c) => (
                    <span
                      key={c.name}
                      title={c.rule}
                      data-testid="expected-column"
                      className={`px-1.5 py-0.5 rounded font-mono ${
                        c.required
                          ? 'bg-amber-100 text-amber-900 dark:bg-amber-900/40 dark:text-amber-200 font-semibold'
                          : 'bg-slate-100 text-slate-700 dark:bg-slate-800 dark:text-slate-300'
                      }`}
                    >
                      {c.name}
                      {c.required ? '*' : ''}
                    </span>
                  ))}
                </div>
              </div>
            )}
            <div className="flex flex-wrap items-center gap-2 text-slate-600 dark:text-slate-300">
              Easiest fix: copy your data into a fresh template. {templateButtons}
            </div>
          </div>
        )}

        {report && headerIssues.length === 0 && (
          <div data-testid="import-report" className="flex flex-col gap-3">
            <div className="flex flex-wrap gap-2">
              <span className="px-2 py-1 rounded bg-emerald-50 text-emerald-700 dark:bg-emerald-950/50 dark:text-emerald-300 font-semibold">
                Ready to import: {report.validCount}
              </span>
              {showsStock && (
                <span className="px-2 py-1 rounded bg-sky-50 text-sky-700 dark:bg-sky-950/50 dark:text-sky-300 font-semibold">
                  Stock lots: {report.lotsToCreate}
                </span>
              )}
              <span className="px-2 py-1 rounded bg-rose-50 text-rose-700 dark:bg-rose-950/50 dark:text-rose-300 font-semibold">
                Errors: {report.rowsWithErrors.length}
              </span>
              {(showsStock || kind === 'PARTIES') && (
                <span className="px-2 py-1 rounded bg-amber-50 text-amber-700 dark:bg-amber-950/50 dark:text-amber-300 font-semibold">
                  Already exist: {report.duplicates.length}
                </span>
              )}
              {warnings.length > 0 && (
                <span className="px-2 py-1 rounded bg-slate-100 text-slate-700 dark:bg-slate-800 dark:text-slate-300 font-semibold">
                  Notes: {warnings.length}
                </span>
              )}
            </div>

            {info.length > 0 && (
              <ul data-testid="import-info" className="list-disc pl-5 text-slate-600 dark:text-slate-300">
                {info.map((line) => (
                  <li key={line}>{line}</li>
                ))}
              </ul>
            )}

            {report.rowsWithErrors.length > 0 && (
              <div className="flex flex-col gap-1">
                <p className="font-semibold text-rose-700 dark:text-rose-300">
                  Fix these rows in the file, save it, then press “Check again”. Nothing is imported while errors remain.
                </p>
                <div className="max-h-44 overflow-auto rounded border border-slate-200 dark:border-slate-700">
                  <table className="w-full">
                    <thead className="sticky top-0 bg-slate-50 dark:bg-slate-800 text-left text-slate-500">
                      <tr>
                        <th className="px-2 py-1 font-medium">Row</th>
                        <th className="px-2 py-1 font-medium">{nameLabel}</th>
                        <th className="px-2 py-1 font-medium">Column</th>
                        <th className="px-2 py-1 font-medium">Problem and fix</th>
                      </tr>
                    </thead>
                    <tbody>
                      {report.rowsWithErrors.map((r, i) => (
                        <tr key={`${r.row}-${r.field}-${i}`} data-testid="import-error-row" className="border-t border-slate-100 dark:border-slate-800">
                          <td className="px-2 py-1 font-mono">{r.row}</td>
                          <td className="px-2 py-1">{r.name ?? '—'}</td>
                          <td className="px-2 py-1 font-mono">{r.field}</td>
                          <td className="px-2 py-1">{r.message}</td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
              </div>
            )}

            {warnings.length > 0 && (
              <details open={warnings.length <= 8} className="rounded border border-slate-200 dark:border-slate-700">
                <summary className="cursor-pointer px-2 py-1 font-semibold text-slate-700 dark:text-slate-300">
                  Notes: how some rows will be imported ({warnings.length})
                </summary>
                <div className="max-h-44 overflow-auto">
                  <table className="w-full">
                    <thead className="sticky top-0 bg-slate-50 dark:bg-slate-800 text-left text-slate-500">
                      <tr>
                        <th className="px-2 py-1 font-medium">Row</th>
                        <th className="px-2 py-1 font-medium">{nameLabel}</th>
                        <th className="px-2 py-1 font-medium">Note</th>
                      </tr>
                    </thead>
                    <tbody>
                      {warnings.map((w, i) => (
                        <tr key={`${w.row}-${i}`} data-testid="import-warning-row" className="border-t border-slate-100 dark:border-slate-800">
                          <td className="px-2 py-1 font-mono">{w.row || '—'}</td>
                          <td className="px-2 py-1">{w.name || '—'}</td>
                          <td className="px-2 py-1">{w.message}</td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
              </details>
            )}

            {report.duplicates.length > 0 && (
              <div className="max-h-32 overflow-auto rounded border border-slate-200 dark:border-slate-700">
                <table className="w-full">
                  <thead className="sticky top-0 bg-slate-50 dark:bg-slate-800 text-left text-slate-500">
                    <tr>
                      <th className="px-2 py-1 font-medium">Row</th>
                      <th className="px-2 py-1 font-medium">{cfg.existsLabel}</th>
                      <th className="px-2 py-1 font-medium">Existing #</th>
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

            {usesStrategy && (!hasBlockingErrors || kind === 'PARTIES') && (
              <fieldset className="flex flex-col gap-1.5" disabled={summary !== null}>
                <legend className="font-semibold text-slate-700 dark:text-slate-300 mb-1">
                  When {kind === 'PARTIES' ? 'a party' : 'an item'} already exists
                </legend>
                {strategies.map((s) => (
                  <label key={s.value} className="flex items-start gap-2 cursor-pointer">
                    <input
                      type="radio"
                      name="import-strategy"
                      value={s.value}
                      checked={strategy === s.value}
                      onChange={() => onStrategyChange(s.value)}
                      className="mt-0.5"
                    />
                    <span>
                      <span className="font-medium">{s.label}</span>
                      <span className="text-slate-400"> : {s.hint}</span>
                    </span>
                  </label>
                ))}
              </fieldset>
            )}
          </div>
        )}

        {summary && (
          <div
            role="status"
            data-testid="import-summary"
            className="rounded-md border border-emerald-200 bg-emerald-50 dark:bg-emerald-950/40 dark:border-emerald-800 px-3 py-2 text-emerald-800 dark:text-emerald-200"
          >
            Import complete. Created {summary.created}, updated {summary.updated}, skipped {summary.skipped}
            {showsStock ? `, stock lots created ${summary.stockLotsCreated}` : ''}.
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

export default ImportDialog;
