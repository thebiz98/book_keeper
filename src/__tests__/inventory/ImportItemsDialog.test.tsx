import { describe, it, expect, vi } from 'vitest';
import { act } from 'react';
import { render } from '../testUtils';
import { byText, click, flush, installApi, typeInto } from './testHelpers';
import { ImportItemsDialog } from '@/components/domain/ImportItemsDialog';
import type { DropState } from '@/api/fileDialog';

const files = vi.hoisted(() => ({
  pickFile: vi.fn(),
  pickSavePath: vi.fn(),
  drop: null as null | ((s: DropState) => void),
}));
vi.mock('@/api/fileDialog', async (orig) => ({
  ...(await orig<typeof import('@/api/fileDialog')>()),
  hasNativeFiles: () => true,
  documentsPath: async (sub?: string) => (sub ? `C:/Docs/${sub}` : 'C:/Docs'),
  pickFile: files.pickFile,
  pickSavePath: files.pickSavePath,
  revealInFolder: vi.fn(),
  onFileDrop: async (h: (s: DropState) => void) => {
    files.drop = h;
    return () => {
      files.drop = null;
    };
  },
}));

const base = { headerIssues: [], warnings: [], lotsToCreate: 9, expectedColumns: [] };
const report = {
  ...base,
  validCount: 12,
  rowsWithErrors: [],
  duplicates: [{ row: 7, name: 'Gel Pen', existingItemId: 22 }],
};
const reportWithErrors = {
  ...report,
  rowsWithErrors: [{ row: 4, name: 'Ink', field: 'Base Unit', message: 'Base Unit is empty. Enter a unit such as PCS.' }],
};
const headerReport = {
  ...base,
  validCount: 0,
  rowsWithErrors: [],
  duplicates: [],
  lotsToCreate: 0,
  headerIssues: [
    {
      kind: 'UNKNOWN',
      column: 'Sale Rate',
      position: 3,
      columnLetter: 'C',
      message: 'Column "Sale Rate" (column C) is not a template column. Rename it to "Sale Price".',
      suggestion: 'Sale Price',
    },
    {
      kind: 'MISSING',
      column: 'Purchase Cost',
      position: null,
      columnLetter: null,
      message: 'Required column "Purchase Cost" is missing. Add a column with exactly this header in row 1.',
      suggestion: 'Purchase Cost',
    },
  ],
  expectedColumns: [
    { name: 'Item Name', required: true, rule: '', example: '' },
    { name: 'Category', required: false, rule: '', example: '' },
  ],
};
const summary = { created: 10, updated: 1, skipped: 1, stockLotsCreated: 3 };

function mount(over: Record<string, unknown> = {}) {
  files.pickFile.mockReset().mockResolvedValue(null);
  files.pickSavePath.mockReset().mockImplementation(async (_f: unknown, p: string) => p);
  const api = installApi({
    validateItemsImport: vi.fn().mockResolvedValue(report),
    executeItemsImport: vi.fn().mockResolvedValue(summary),
    validateVedbillImport: vi.fn().mockResolvedValue({
      ...report,
      duplicates: [],
      warnings: [{ row: 91, name: 'WHITE GLUE 20', message: 'VedBill shows stock 29 but its lots add up to 32.' }],
    }),
    executeVedbillImport: vi.fn().mockResolvedValue(summary),
    writeItemsImportTemplate: vi.fn().mockImplementation(async (p: string) => p),
    ...over,
  });
  const onImported = vi.fn();
  const r = render(<ImportItemsDialog isOpen onClose={vi.fn()} onImported={onImported} />);
  return { api, onImported, ...r };
}

const pathInput = (c: HTMLElement) => c.querySelector('input[placeholder^="Path to the"]') as HTMLInputElement;

describe('ImportItemsDialog', () => {
  it('a pasted path is checked automatically -> report -> strategy -> confirm -> summary', async () => {
    const { api, container, onImported } = mount();
    await flush(60);
    expect((byText(container, 'button', 'Confirm import') as HTMLButtonElement).disabled).toBe(true);

    typeInto(pathInput(container), ' "C:\\tmp\\items.xlsx" ');
    await flush(700);
    expect(api.validateItemsImport).toHaveBeenCalledWith('C:\\tmp\\items.xlsx');

    const rep = container.querySelector('[data-testid="import-report"]') as HTMLElement;
    expect(rep.textContent).toContain('Ready to import: 12');
    expect(rep.textContent).toContain('Stock lots: 9');
    expect(container.querySelector('[data-testid="import-duplicate-row"]')?.textContent).toContain('Gel Pen');

    const radios = Array.from(container.querySelectorAll<HTMLInputElement>('input[name="import-strategy"]'));
    expect(radios.map((r) => r.value)).toEqual(['SKIP', 'OVERWRITE', 'ADD_STOCK']);
    expect(radios[0].checked).toBe(true);
    click(radios[2]);

    click(byText(container, 'button', 'Confirm import'));
    await flush();
    expect(api.executeItemsImport).toHaveBeenCalledWith('C:\\tmp\\items.xlsx', 'ADD_STOCK');
    expect(container.querySelector('[data-testid="import-summary"]')?.textContent).toContain('Created 10, updated 1, skipped 1');
    expect(onImported).toHaveBeenCalledTimes(1);
  });

  it('row errors are listed with the item name and block the import', async () => {
    const { container, api } = mount({ validateItemsImport: vi.fn().mockResolvedValue(reportWithErrors) });
    await flush(60);
    typeInto(pathInput(container), '/a.csv');
    click(byText(container, 'button', 'Check file'));
    await flush();
    const row = container.querySelector('[data-testid="import-error-row"]')?.textContent ?? '';
    expect(row).toContain('Ink');
    expect(row).toContain('Base Unit is empty');
    expect((byText(container, 'button', 'Confirm import') as HTMLButtonElement).disabled).toBe(true);
    expect(container.querySelector('input[name="import-strategy"]')).toBeNull();
    expect(api.executeItemsImport).not.toHaveBeenCalled();
  });

  it('header problems show every fix, the expected columns and template downloads', async () => {
    const { container, api } = mount({ validateItemsImport: vi.fn().mockResolvedValue(headerReport) });
    await flush(60);
    typeInto(pathInput(container), '/bad.csv');
    click(byText(container, 'button', 'Check file'));
    await flush();
    const panel = container.querySelector('[data-testid="import-header-issues"]') as HTMLElement;
    const issues = Array.from(panel.querySelectorAll('[data-testid="import-header-issue"]')).map((e) => e.textContent);
    expect(issues[0]).toContain('Rename it to "Sale Price"');
    expect(issues[1]).toContain('"Purchase Cost" is missing');
    const cols = Array.from(panel.querySelectorAll('[data-testid="expected-column"]')).map((e) => e.textContent);
    expect(cols).toEqual(['Item Name*', 'Category']);
    expect(container.querySelector('[data-testid="import-report"]')).toBeNull();
    expect((byText(container, 'button', 'Confirm import') as HTMLButtonElement).disabled).toBe(true);

    click(byText(panel, 'button', 'Excel template'));
    await flush();
    expect(api.writeItemsImportTemplate).toHaveBeenCalledWith('C:/Docs/book_keeper_items_template.xlsx', 'XLSX');
    expect(container.querySelector('[data-testid="template-saved"]')?.textContent).toContain('book_keeper_items_template.xlsx');
  });

  it('CSV template download', async () => {
    const { container, api } = mount();
    await flush(60);
    click(byText(container, 'button', 'CSV template'));
    await flush();
    expect(api.writeItemsImportTemplate).toHaveBeenCalledWith('C:/Docs/book_keeper_items_template.csv', 'CSV');
  });

  it('VedBill source validates and imports through the VedBill commands, showing notes', async () => {
    const { container, api } = mount();
    await flush(60);
    click(container.querySelector('[data-testid="import-source-VEDBILL"]') as HTMLElement);
    typeInto(pathInput(container), 'C:\\Users\\me\\Documents\\VEDBIL\\shop-backup-2026-11-22-1405.vb');
    await flush(700);
    expect(api.validateVedbillImport).toHaveBeenCalledWith('C:\\Users\\me\\Documents\\VEDBIL\\shop-backup-2026-11-22-1405.vb');
    expect(api.validateItemsImport).not.toHaveBeenCalled();
    expect(container.querySelector('[data-testid="import-warning-row"]')?.textContent).toContain('lots add up to 32');
    click(byText(container, 'button', 'Confirm import'));
    await flush();
    expect(api.executeVedbillImport).toHaveBeenCalledWith(
      'C:\\Users\\me\\Documents\\VEDBIL\\shop-backup-2026-11-22-1405.vb',
      'SKIP'
    );
  });

  it('a .vb path typed on the spreadsheet tab switches to the VedBill source', async () => {
    const { container, api } = mount();
    await flush(60);
    typeInto(pathInput(container), 'D:\\b.vb');
    await flush(700);
    expect(api.validateVedbillImport).toHaveBeenCalledWith('D:\\b.vb');
    expect(container.querySelector('[data-testid="import-source-VEDBILL"]')?.getAttribute('aria-checked')).toBe('true');
  });

  it('choosing VedBill opens the file picker in Documents/VEDBIL and checks the chosen backup', async () => {
    const { container, api } = mount();
    files.pickFile.mockResolvedValue('C:/Docs/VEDBIL/x.vb');
    await flush(60);
    click(container.querySelector('[data-testid="import-source-VEDBILL"]') as HTMLElement);
    await flush();
    expect(files.pickFile).toHaveBeenCalledTimes(1);
    const [filters, defaultPath] = files.pickFile.mock.calls[0];
    expect(filters[0].extensions).toEqual(['vb']);
    expect(defaultPath).toBe('C:/Docs/VEDBIL');
    expect(api.validateVedbillImport).toHaveBeenCalledWith('C:/Docs/VEDBIL/x.vb');
    expect(pathInput(container).value).toBe('C:/Docs/VEDBIL/x.vb');
  });

  it('a dropped spreadsheet is checked at once; unsupported files get a clear message', async () => {
    const { container, api } = mount();
    await flush(60);
    expect(files.drop).not.toBeNull();
    await act(async () => files.drop!('over'));
    expect(container.querySelector('[data-testid="import-dropzone"]')?.textContent).toContain('Drop to check this file');
    await act(async () => files.drop!({ dropped: ['C:/x/notes.txt'] }));
    expect(container.querySelector('[role="alert"]')?.textContent).toContain('not supported');
    await act(async () => files.drop!({ dropped: ['C:/x/items.csv'] }));
    await flush();
    expect(api.validateItemsImport).toHaveBeenCalledWith('C:/x/items.csv');
  });

  it('editing the path invalidates the report so a stale file cannot be imported', async () => {
    const { container, api } = mount();
    await flush(60);
    typeInto(pathInput(container), '/a.xlsx');
    await flush(700);
    typeInto(pathInput(container), '/b.xls');
    expect((byText(container, 'button', 'Confirm import') as HTMLButtonElement).disabled).toBe(true);
    expect(api.executeItemsImport).not.toHaveBeenCalled();
  });

  it('shows validation failures from the backend', async () => {
    const { container } = mount({
      validateItemsImport: vi.fn().mockRejectedValue({ type: 'IoError', details: 'File not found' }),
    });
    await flush(60);
    typeInto(pathInput(container), '/nope.xlsx');
    await flush(700);
    expect(container.querySelector('[role="alert"]')?.textContent).toBe('File not found');
  });
});
