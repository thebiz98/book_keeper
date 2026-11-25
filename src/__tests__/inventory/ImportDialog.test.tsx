import { describe, it, expect, vi } from 'vitest';
import { render } from '../testUtils';
import { byText, click, flush, installApi, typeInto } from './testHelpers';
import { ImportDialog, type ImportDialogKind } from '@/components/domain/ImportDialog';

const files = vi.hoisted(() => ({
  pickSavePath: vi.fn(),
}));
vi.mock('@/api/fileDialog', async (orig) => ({
  ...(await orig<typeof import('@/api/fileDialog')>()),
  hasNativeFiles: () => true,
  documentsPath: async (sub?: string) => (sub ? `C:/Docs/${sub}` : 'C:/Docs'),
  pickFile: vi.fn().mockResolvedValue(null),
  pickSavePath: files.pickSavePath,
  revealInFolder: vi.fn(),
  onFileDrop: async () => () => undefined,
}));

const base = {
  headerIssues: [],
  warnings: [],
  lotsToCreate: 0,
  expectedColumns: [],
  duplicates: [],
  rowsWithErrors: [],
};
const summary = { created: 3, updated: 1, skipped: 2, stockLotsCreated: 0 };

function mount(kind: ImportDialogKind, over: Record<string, unknown> = {}) {
  files.pickSavePath.mockReset().mockImplementation(async (_f: unknown, p: string) => p);
  const api = installApi({
    validatePartiesImport: vi.fn().mockResolvedValue({
      ...base,
      validCount: 3,
      duplicates: [{ row: 4, name: 'Ravi', existingItemId: 9 }],
      info: ['1 opening balance bill(s) totalling 5000 will be created or updated.'],
    }),
    executePartiesImport: vi.fn().mockResolvedValue(summary),
    validateExpensesImport: vi.fn().mockResolvedValue({
      ...base,
      validCount: 4,
      warnings: [{ row: 3, name: 'Tea', message: 'Identical to row 2 (same date, type, amount and notes).' }],
      info: ['1 new expense type(s) will be created: Tea.'],
    }),
    executeExpensesImport: vi.fn().mockResolvedValue({ ...summary, created: 4 }),
    validateServiceComponentsImport: vi.fn().mockResolvedValue({ ...base, validCount: 2, info: ['1 service(s) will get a new component list.'] }),
    executeServiceComponentsImport: vi.fn().mockResolvedValue({ ...summary, created: 0 }),
    writeImportTemplate: vi.fn().mockImplementation(async (_k: string, p: string) => p),
    writeItemsImportTemplate: vi.fn().mockImplementation(async (p: string) => p),
    validateItemsImport: vi.fn().mockResolvedValue({ ...base, validCount: 1 }),
    ...over,
  });
  const onImported = vi.fn();
  const r = render(<ImportDialog kind={kind} isOpen onClose={vi.fn()} onImported={onImported} />);
  return { api, onImported, ...r };
}

const pathInput = (c: HTMLElement) => c.querySelector('input[placeholder^="Path to the"]') as HTMLInputElement;

describe('ImportDialog (FR-41)', () => {
  it('parties: report with info and existing parties, strategy Skip/Update re-checks, confirm imports with the strategy', async () => {
    const { api, container, onImported } = mount('PARTIES');
    await flush(60);
    expect(container.textContent).toContain('Import Parties');
    expect(container.querySelector('[data-testid="import-source-VEDBILL"]')).toBeNull();

    typeInto(pathInput(container), 'C:\\tmp\\parties.csv');
    await flush(700);
    expect(api.validatePartiesImport).toHaveBeenCalledWith('C:\\tmp\\parties.csv', 'SKIP');
    const rep = container.querySelector('[data-testid="import-report"]') as HTMLElement;
    expect(rep.textContent).toContain('Ready to import: 3');
    expect(rep.textContent).toContain('Already exist: 1');
    expect(rep.textContent).not.toContain('Stock lots');
    expect(container.querySelector('[data-testid="import-info"]')?.textContent).toContain('totalling 5000');

    const radios = Array.from(container.querySelectorAll<HTMLInputElement>('input[name="import-strategy"]'));
    expect(radios.map((r) => r.value)).toEqual(['SKIP', 'OVERWRITE']);
    click(radios[1]);
    await flush();
    expect(api.validatePartiesImport).toHaveBeenLastCalledWith('C:\\tmp\\parties.csv', 'OVERWRITE');

    click(byText(container, 'button', 'Confirm import'));
    await flush();
    expect(api.executePartiesImport).toHaveBeenCalledWith('C:\\tmp\\parties.csv', 'OVERWRITE');
    const text = container.querySelector('[data-testid="import-summary"]')?.textContent ?? '';
    expect(text).toContain('Created 3, updated 1, skipped 2');
    expect(text).not.toContain('stock lots');
    expect(onImported).toHaveBeenCalledTimes(1);
  });

  it('parties: row errors block the import but the strategy stays changeable', async () => {
    const { container, api } = mount('PARTIES', {
      validatePartiesImport: vi.fn().mockResolvedValue({
        ...base,
        validCount: 1,
        rowsWithErrors: [{ row: 3, name: 'Ravi', field: 'Opening Balance', message: '₹600 is already settled against the opening balance' }],
      }),
    });
    await flush(60);
    typeInto(pathInput(container), '/p.csv');
    click(byText(container, 'button', 'Check file'));
    await flush();
    expect(container.querySelector('[data-testid="import-error-row"]')?.textContent).toContain('already settled');
    expect(container.querySelector('input[name="import-strategy"]')).not.toBeNull();
    expect((byText(container, 'button', 'Confirm import') as HTMLButtonElement).disabled).toBe(true);
    expect(api.executePartiesImport).not.toHaveBeenCalled();
  });

  it('parties: template download uses the PARTIES kind', async () => {
    const { container, api } = mount('PARTIES');
    await flush(60);
    click(byText(container, 'button', 'CSV template'));
    await flush();
    expect(api.writeImportTemplate).toHaveBeenCalledWith('PARTIES', 'C:/Docs/book_keeper_parties_template.csv', 'CSV');
  });

  it('expenses: no strategy, warnings listed, confirm imports', async () => {
    const { container, api, onImported } = mount('EXPENSES');
    await flush(60);
    expect(container.textContent).toContain('Import Expenses');
    typeInto(pathInput(container), '/e.xlsx');
    await flush(700);
    expect(api.validateExpensesImport).toHaveBeenCalledWith('/e.xlsx');
    expect(container.querySelector('input[name="import-strategy"]')).toBeNull();
    expect(container.querySelector('[data-testid="import-warning-row"]')?.textContent).toContain('Identical to row 2');
    expect(container.querySelector('[data-testid="import-info"]')?.textContent).toContain('new expense type');
    click(byText(container, 'button', 'Confirm import'));
    await flush();
    expect(api.executeExpensesImport).toHaveBeenCalledWith('/e.xlsx');
    expect(container.querySelector('[data-testid="import-summary"]')?.textContent).toContain('Created 4');
    expect(onImported).toHaveBeenCalledTimes(1);
    click(byText(container, 'button', 'Close'));
  });

  it('expenses: template download uses the EXPENSES kind', async () => {
    const { container, api } = mount('EXPENSES');
    await flush(60);
    click(byText(container, 'button', 'Excel template'));
    await flush();
    expect(api.writeImportTemplate).toHaveBeenCalledWith('EXPENSES', 'C:/Docs/book_keeper_expenses_template.xlsx', 'XLSX');
  });

  it('items: the Service components source uses its own commands and template, with no strategy', async () => {
    const { container, api } = mount('ITEMS');
    await flush(60);
    click(container.querySelector('[data-testid="import-source-COMPONENTS"]') as HTMLElement);
    typeInto(pathInput(container), '/c.csv');
    await flush(700);
    expect(api.validateServiceComponentsImport).toHaveBeenCalledWith('/c.csv');
    expect(api.validateItemsImport).not.toHaveBeenCalled();
    expect(container.querySelector('input[name="import-strategy"]')).toBeNull();
    expect(container.querySelector('[data-testid="import-report"]')?.textContent).not.toContain('Stock lots');
    click(byText(container, 'button', 'CSV template'));
    await flush();
    expect(api.writeImportTemplate).toHaveBeenCalledWith('SERVICE_COMPONENTS', 'C:/Docs/book_keeper_service_components_template.csv', 'CSV');
    click(byText(container, 'button', 'Confirm import'));
    await flush();
    expect(api.executeServiceComponentsImport).toHaveBeenCalledWith('/c.csv');
  });
});
