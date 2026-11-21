import React, { useState } from 'react';
import { Database, FolderSearch, FileSearch, Play, FileSpreadsheet } from 'lucide-react';
import { api } from '@/api';
import type { BackupFileInfo, BackupInspection, ImportSummary, MigrationReport } from '@/types';
import { Card, CardHeader, CardTitle, CardContent } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import { TextInput } from '@/components/ui/TextInput';
import { ConfirmDialog } from '@/components/common/ConfirmDialog';

const countLine = (label: string, v?: number) => (
  <span key={label} className="text-xs font-mono">
    {label}: <b>{v ?? 0}</b>
  </span>
);

const ReportView: React.FC<{ report: MigrationReport }> = ({ report }) => (
  <div className="flex flex-col gap-1 rounded-md border border-slate-200 dark:border-slate-800 p-3">
    <span
      className={`text-xs font-semibold ${
        !report.success ? 'text-rose-600' : 'text-emerald-600'
      }`}
    >
      {!report.success ? 'Completed with errors' : 'Completed successfully'}
    </span>
    <div className="flex flex-wrap gap-4">
      {countLine('Items', report.itemsImported)}
      {countLine('Parties', report.partiesImported)}
      {countLine('Sales', report.salesImported)}
      {countLine('Purchases', report.purchasesImported)}
    </div>
    {report.errors.map((m, i) => (
      <span key={i} className="text-xs text-amber-600 dark:text-amber-400">
        {m}
      </span>
    ))}
  </div>
);

export const MigrationPage: React.FC = () => {
  const [dir, setDir] = useState('');
  const [files, setFiles] = useState<BackupFileInfo[]>([]);
  const [selected, setSelected] = useState('');
  const [inspection, setInspection] = useState<BackupInspection | null>(null);
  const [report, setReport] = useState<MigrationReport | null>(null);
  const [confirmOpen, setConfirmOpen] = useState(false);

  const [excelPath, setExcelPath] = useState('');
  const [excelMsg, setExcelMsg] = useState<string | null>(null);

  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const run = async (name: string, fn: () => Promise<void>) => {
    setBusy(name);
    setError(null);
    try {
      await fn();
    } catch (err) {
      setError(err instanceof Error ? err.message : `${name} failed`);
    } finally {
      setBusy(null);
    }
  };

  const handleScan = () =>
    run('scan', async () => {
      const found = await api.scanBackupDirectory(dir.trim());
      setFiles(found);
      setInspection(null);
      setReport(null);
    });

  const handleInspect = () =>
    run('inspect', async () => {
      setInspection(await api.inspectBackupFile(selected));
      setReport(null);
    });

  const handleExecute = () => {
    setConfirmOpen(false);
    return run('migrate', async () => {
      setReport(await api.executeMigration(selected));
    });
  };

  const excelAction = (label: string, fn: (path: string) => Promise<string | ImportSummary>) =>
    run(label, async () => {
      if (!excelPath.trim()) throw new Error('Enter a file path first');
      const res = await fn(excelPath.trim());
      setExcelMsg(
        typeof res === 'string'
          ? `${label} complete: ${res}`
          : `${label} complete: ${res.importedCount} of ${res.totalRows} rows imported, ${res.skippedCount} skipped${res.errors.length ? ` (${res.errors.join('; ')})` : ''}`
      );
    });

  return (
    <div className="flex flex-col gap-4 max-w-3xl">
      <div className="flex items-center gap-2 px-1">
        <Database className="h-5 w-5 text-blue-600" />
        <h2 className="text-sm font-bold">Data Migration</h2>
      </div>

      {error && (
        <p role="alert" className="text-xs text-rose-600 dark:text-rose-400 font-medium px-1">
          {error}
        </p>
      )}

      <Card>
        <CardHeader>
          <CardTitle>VedBill Backup</CardTitle>
        </CardHeader>
        <CardContent className="flex flex-col gap-3">
          <div className="flex items-end gap-2">
            <div className="flex-1">
              <TextInput
                label="Backup Directory"
                placeholder="/storage/emulated/0/VedBill"
                value={dir}
                onChange={(e) => setDir(e.target.value)}
              />
            </div>
            <Button
              variant="outline"
              isLoading={busy === 'scan'}
              leftIcon={<FolderSearch className="h-4 w-4" />}
              onClick={handleScan}
            >
              Scan
            </Button>
          </div>

          {files.length > 0 && (
            <ul className="flex flex-col gap-1">
              {files.map((f) => (
                <li key={f.filePath}>
                  <label className="flex items-center gap-2 text-xs cursor-pointer">
                    <input
                      type="radio"
                      name="backup-file"
                      checked={selected === f.filePath}
                      onChange={() => setSelected(f.filePath)}
                    />
                    <span className="font-mono">{f.fileName}</span>
                    <span className="text-slate-400">
                      {Math.round(f.fileSizeBytes / 1024)} KB
                    </span>
                  </label>
                </li>
              ))}
            </ul>
          )}

          <div className="flex gap-2">
            <Button
              variant="outline"
              disabled={!selected}
              isLoading={busy === 'inspect'}
              leftIcon={<FileSearch className="h-4 w-4" />}
              onClick={handleInspect}
            >
              Inspect
            </Button>
            <Button
              variant="primary"
              disabled={!selected || !inspection}
              isLoading={busy === 'migrate'}
              leftIcon={<Play className="h-4 w-4" />}
              onClick={() => setConfirmOpen(true)}
            >
              Run Migration
            </Button>
          </div>

          {inspection && (
            <div className="flex flex-wrap gap-4 rounded-md border border-slate-200 dark:border-slate-800 p-3">
              {countLine('Items', inspection.tableStats.itemsCount)}
              {countLine('Parties', inspection.tableStats.partiesCount)}
              {countLine('Sales', inspection.tableStats.salesCount)}
              {countLine('Purchases', inspection.tableStats.purchasesCount)}
            </div>
          )}
          {report && <ReportView report={report} />}
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>Excel Import / Export</CardTitle>
        </CardHeader>
        <CardContent className="flex flex-col gap-3">
          <TextInput
            label="File Path (.xlsx)"
            placeholder="/storage/emulated/0/Documents/items.xlsx"
            value={excelPath}
            onChange={(e) => setExcelPath(e.target.value)}
          />
          <div className="flex flex-wrap gap-2">
            <Button
              variant="outline"
              isLoading={busy === 'Export items'}
              leftIcon={<FileSpreadsheet className="h-4 w-4" />}
              onClick={() => excelAction('Export items', (p) => api.exportItemsToExcel(p))}
            >
              Export Items
            </Button>
            <Button
              variant="outline"
              isLoading={busy === 'Import items'}
              leftIcon={<FileSpreadsheet className="h-4 w-4" />}
              onClick={() => excelAction('Import items', (p) => api.importItemsFromExcel(p))}
            >
              Import Items
            </Button>
            <Button
              variant="outline"
              isLoading={busy === 'Export transactions'}
              leftIcon={<FileSpreadsheet className="h-4 w-4" />}
              onClick={() =>
                excelAction('Export transactions', (p) => api.exportTransactionsToExcel(p))
              }
            >
              Export Transactions
            </Button>
          </div>
          {excelMsg && <p className="text-xs text-emerald-600 dark:text-emerald-400">{excelMsg}</p>}
        </CardContent>
      </Card>

      <ConfirmDialog
        isOpen={confirmOpen}
        title="Run Migration?"
        message="This imports data from the selected backup into the current database. Create a backup first."
        variant="warning"
        confirmText="Migrate"
        onConfirm={handleExecute}
        onCancel={() => setConfirmOpen(false)}
      />
    </div>
  );
};

export default MigrationPage;
