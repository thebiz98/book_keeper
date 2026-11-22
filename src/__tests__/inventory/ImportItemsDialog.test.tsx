import { describe, it, expect, vi } from 'vitest';
import { render } from '../testUtils';
import { byText, click, flush, installApi, typeInto } from './testHelpers';
import { ImportItemsDialog } from '@/components/domain/ImportItemsDialog';

const report = {
  validCount: 12,
  rowsWithErrors: [{ row: 4, field: 'base_unit', message: 'Base unit is required' }],
  duplicates: [{ row: 7, name: 'Gel Pen', existingItemId: 22 }],
};
const summary = { created: 10, updated: 1, skipped: 1, stockLotsCreated: 3 };

function mount(over: Record<string, unknown> = {}) {
  const api = installApi({
    validateItemsImport: vi.fn().mockResolvedValue(report),
    executeItemsImport: vi.fn().mockResolvedValue(summary),
    ...over,
  });
  const onImported = vi.fn();
  const r = render(<ImportItemsDialog isOpen onClose={vi.fn()} onImported={onImported} />);
  return { api, onImported, ...r };
}

const pathInput = (c: HTMLElement) => c.querySelector('input[placeholder^="Absolute path"]') as HTMLInputElement;

describe('ImportItemsDialog', () => {
  it('validate -> report -> strategy -> confirm -> summary, then refreshes the catalog', async () => {
    const { api, container, onImported } = mount();
    await flush(60);
    expect((byText(container, 'button', 'Confirm import') as HTMLButtonElement).disabled).toBe(true);

    typeInto(pathInput(container), ' /tmp/items.xlsx ');
    click(byText(container, 'button', 'Validate'));
    await flush();
    expect(api.validateItemsImport).toHaveBeenCalledWith('/tmp/items.xlsx');

    const rep = container.querySelector('[data-testid="import-report"]') as HTMLElement;
    expect(rep.textContent).toContain('Valid rows: 12');
    expect(container.querySelector('[data-testid="import-error-row"]')?.textContent).toContain('Base unit is required');
    expect(container.querySelector('[data-testid="import-duplicate-row"]')?.textContent).toContain('Gel Pen');

    const radios = Array.from(container.querySelectorAll<HTMLInputElement>('input[name="import-strategy"]'));
    expect(radios.map((r) => r.value)).toEqual(['SKIP', 'OVERWRITE', 'ADD_STOCK']);
    expect(radios[0].checked).toBe(true);
    click(radios[2]);

    click(byText(container, 'button', 'Confirm import'));
    await flush();
    expect(api.executeItemsImport).toHaveBeenCalledWith('/tmp/items.xlsx', 'ADD_STOCK');
    expect(container.querySelector('[data-testid="import-summary"]')?.textContent).toContain('Created 10, updated 1, skipped 1');
    expect(onImported).toHaveBeenCalledTimes(1);
  });

  it('editing the path invalidates the report so a stale file cannot be imported', async () => {
    const { container, api } = mount();
    await flush(60);
    typeInto(pathInput(container), '/a.xlsx');
    click(byText(container, 'button', 'Validate'));
    await flush();
    typeInto(pathInput(container), '/b.xlsx');
    expect((byText(container, 'button', 'Confirm import') as HTMLButtonElement).disabled).toBe(true);
    expect(api.executeItemsImport).not.toHaveBeenCalled();
  });

  it('shows validation failures from the backend', async () => {
    const { container } = mount({
      validateItemsImport: vi.fn().mockRejectedValue({ type: 'IoError', details: 'File not found' }),
    });
    await flush(60);
    typeInto(pathInput(container), '/nope.xlsx');
    click(byText(container, 'button', 'Validate'));
    await flush();
    expect(container.querySelector('[role="alert"]')?.textContent).toBe('File not found');
  });
});
