import { describe, it, expect, beforeEach } from 'vitest';
import { act } from 'react';
import { render } from '../testUtils';
import { ExpensesPage } from '@/pages/ExpensesPage';
import App from '@/App';
import { useAppStore } from '@/store/useAppStore';
import { __resetHotkeys } from '@/hooks/useHotkeys';
import type { Expense, ExpenseType } from '@/types';
import { installApi, waitFor, flush, click, setInput, bodyText } from './helpers';

const INK: ExpenseType = { id: 1, name: 'Printer Ink', isActive: true, createdAt: '2026-11-01T00:00:00Z' };
const TEA: ExpenseType = { id: 2, name: 'Tea', isActive: true, createdAt: '2026-11-01T00:00:00Z' };

const row = (id: number, type: ExpenseType, amount: number, notes: string | null = null): Expense => ({
  id,
  expenseCode: `EXP-${1000 + id}`,
  expenseDate: '2026-11-20',
  expenseTypeId: type.id,
  expenseTypeName: type.name,
  amount,
  notes,
  createdAt: '2026-11-20T09:00:00Z',
  updatedAt: null,
});

const q = <T extends Element = HTMLElement>(sel: string) => document.querySelector<T>(sel);
const tid = <T extends Element = HTMLElement>(id: string) => q<T>(`[data-testid="${id}"]`);

function key(el: Element, k: string, init: KeyboardEventInit = {}) {
  act(() => {
    el.dispatchEvent(new KeyboardEvent('keydown', { key: k, bubbles: true, cancelable: true, ...init }));
  });
}

function makeApi(over: Record<string, unknown> = {}) {
  const rows = [row(1, INK, 450, 'cartridge'), row(2, TEA, 50)];
  return installApi({
    searchExpenseTypes: async (query: string) => [INK, TEA].filter((t) => t.name.toLowerCase().includes(query.toLowerCase())),
    listExpenseTypes: [INK, TEA],
    getNextExpenseCode: 'EXP-1003',
    createExpenseType: async (name: string) => ({ id: 9, name, isActive: true, createdAt: '2026-11-21T00:00:00Z' }),
    createExpense: async (p: { expenseTypeId: number; amount: number }) => ({
      ...row(3, INK, p.amount),
      expenseTypeId: p.expenseTypeId,
    }),
    updateExpense: async (id: number, p: { amount: number }) => ({ ...row(id, INK, p.amount), updatedAt: 'now' }),
    deleteExpense: undefined,
    listExpenses: { items: rows, total: 2, page: 1, limit: 25, periodTotal: 500 },
    getDashboardMetrics: { cashInHand: 0 },
    ...over,
  });
}

async function typeType(text: string) {
  const input = tid<HTMLInputElement>('expense-type-input')!;
  setInput(input, text);
  await waitFor(() => !!q('[role="option"]'));
  return input;
}

describe('ExpensesPage', () => {
  beforeEach(() => {
    document.body.innerHTML = '';
    __resetHotkeys();
  });

  it('focuses the Type field, shows the code preview and the list with the period total', async () => {
    makeApi();
    render(<ExpensesPage />);
    await waitFor(() => !!tid('expense-code-preview') && bodyText().includes('EXP-1001'));
    expect(document.activeElement).toBe(tid('expense-type-input'));
    expect(tid('expense-code-preview')!.textContent).toBe('EXP-1003');
    expect(tid('expense-period-total')!.textContent).toContain('₹500.00');
  });

  it('typing searches live and Enter selects the top hit, then focus moves to Amount', async () => {
    const api = makeApi();
    render(<ExpensesPage />);
    await waitFor(() => !!tid('expense-type-input'));
    const input = await typeType('pri');
    expect(api.searchExpenseTypes).toHaveBeenCalledWith('pri', 8);
    key(input, 'Enter');
    await waitFor(() => (tid('expense-type-selected')?.textContent ?? '').includes('Printer Ink'));
    await flush(10);
    expect(document.activeElement).toBe(tid('expense-amount'));
  });

  it('no match offers Create; Enter opens the inline panel with Save/Cancel; Save creates and selects', async () => {
    const api = makeApi({ searchExpenseTypes: async () => [] });
    render(<ExpensesPage />);
    await waitFor(() => !!tid('expense-type-input'));
    const input = tid<HTMLInputElement>('expense-type-input')!;
    setInput(input, 'Toner');
    await waitFor(() => !!q('[data-create-option]'));
    expect(q('[data-create-option]')!.textContent).toContain('+ Create "Toner"');
    key(input, 'Enter');
    await waitFor(() => !!tid('expense-type-create'));
    const panel = tid('expense-type-create')!;
    expect((tid<HTMLInputElement>('expense-type-name')!).value).toBe('Toner');
    expect(panel.textContent).toContain('Cancel');
    expect(panel.textContent).toContain('Save type');
    const save = Array.from(panel.querySelectorAll('button')).find((b) => b.textContent?.includes('Save type'))!;
    click(save);
    await waitFor(() => (tid('expense-type-selected')?.textContent ?? '').includes('Toner'));
    expect(api.createExpenseType).toHaveBeenCalledWith('Toner');
  });

  it('Cancel in the create panel creates nothing and returns focus to Type', async () => {
    const api = makeApi({ searchExpenseTypes: async () => [] });
    render(<ExpensesPage />);
    await waitFor(() => !!tid('expense-type-input'));
    const input = tid<HTMLInputElement>('expense-type-input')!;
    setInput(input, 'Toner');
    await waitFor(() => !!q('[data-create-option]'));
    click(q('[data-create-option]')!);
    await waitFor(() => !!tid('expense-type-create'));
    const cancel = Array.from(tid('expense-type-create')!.querySelectorAll('button')).find((b) => b.textContent?.includes('Cancel'))!;
    click(cancel);
    await waitFor(() => !tid('expense-type-create'));
    await flush(10);
    expect(api.createExpenseType).not.toHaveBeenCalled();
    expect(document.activeElement).toBe(tid('expense-type-input'));
  });

  it('submits a camelCase payload, resets the form and reloads the list', async () => {
    const api = makeApi();
    render(<ExpensesPage />);
    await waitFor(() => !!tid('expense-type-input'));
    const input = await typeType('pri');
    key(input, 'Enter');
    await waitFor(() => (tid('expense-type-selected')?.textContent ?? '').includes('Printer Ink'));
    setInput(tid('expense-amount')!, '120');
    setInput(tid('expense-notes')!, ' toner refill ');
    const listCalls = api.listExpenses.mock.calls.length;
    click(tid('expense-save')!);
    await waitFor(() => !!tid('expense-saved'));
    expect(api.createExpense).toHaveBeenCalledWith({
      expenseDate: expect.stringMatching(/^\d{4}-\d{2}-\d{2}$/),
      expenseTypeId: 1,
      amount: 120,
      notes: 'toner refill',
    });
    expect(tid('expense-saved')!.textContent).toContain('EXP-1003 saved');
    expect(tid<HTMLInputElement>('expense-amount')!.value).toBe('0');
    await waitFor(() => api.listExpenses.mock.calls.length > listCalls);
  });

  it('shows a backend validation error inline and keeps the form', async () => {
    makeApi({
      createExpense: async () => {
        throw { type: 'ValidationError', details: { field: 'amount', message: 'Amount must be greater than 0' } };
      },
    });
    render(<ExpensesPage />);
    await waitFor(() => !!tid('expense-type-input'));
    const input = await typeType('tea');
    key(input, 'Enter');
    await waitFor(() => (tid('expense-type-selected')?.textContent ?? '').includes('Tea'));
    setInput(tid('expense-amount')!, '5');
    click(tid('expense-save')!);
    await waitFor(() => !!tid('expense-error'));
    expect(tid('expense-error')!.textContent).toContain('Amount must be greater than 0');
  });

  it('client check: saving without a type shows a message and calls nothing', async () => {
    const api = makeApi();
    render(<ExpensesPage />);
    await waitFor(() => !!tid('expense-save'));
    click(tid('expense-save')!);
    await waitFor(() => !!tid('expense-error'));
    expect(tid('expense-error')!.textContent).toContain('Choose or create an expense type');
    expect(api.createExpense).not.toHaveBeenCalled();
  });

  it('list search and type filter pass searchQuery and expenseTypeId', async () => {
    const api = makeApi();
    render(<ExpensesPage />);
    await waitFor(() => bodyText().includes('EXP-1001'));
    setInput(tid('expense-search')!, 'ink');
    await waitFor(() => api.listExpenses.mock.calls.some((c) => c[0].searchQuery === 'ink'));
    setInput(tid('expense-type-filter')!, '2');
    await waitFor(() => api.listExpenses.mock.calls.some((c) => c[0].expenseTypeId === 2));
  });

  it('Edit loads the row and Update calls updateExpense; Delete confirms then deletes', async () => {
    const api = makeApi();
    render(<ExpensesPage />);
    await waitFor(() => !!tid('expense-edit-1'));
    click(tid('expense-edit-1')!);
    await waitFor(() => bodyText().includes('Editing EXP-1001'));
    expect(tid<HTMLInputElement>('expense-amount')!.value).toBe('450');
    setInput(tid('expense-amount')!, '460');
    click(tid('expense-save')!);
    await waitFor(() => api.updateExpense.mock.calls.length === 1);
    expect(api.updateExpense.mock.calls[0][0]).toBe(1);
    expect(api.updateExpense.mock.calls[0][1]).toMatchObject({ amount: 460, expenseTypeId: 1 });

    click(tid('expense-delete-2')!);
    await waitFor(() => bodyText().includes('Delete EXP-1002 (Tea'));
    const confirm = Array.from(document.querySelectorAll('button')).find((b) => (b.textContent ?? '').trim().startsWith('Delete') && !b.getAttribute('data-testid') && b.closest('[data-modal], [role="dialog"], [aria-modal]') !== null);
    expect(confirm).toBeTruthy();
    click(confirm!);
    await waitFor(() => api.deleteExpense.mock.calls.length === 1);
    expect(api.deleteExpense).toHaveBeenCalledWith(2);
  });
});

describe('F7 hotkey', () => {
  beforeEach(() => {
    document.body.innerHTML = '';
    __resetHotkeys();
  });

  it('navigates to Expenses and focuses the Type field', async () => {
    makeApi({ getDashboardMetrics: { cashInHand: 0, todayExpensesAmount: 0 } });
    useAppStore.setState({ activeRoute: 'dashboard' });
    render(<App />);
    await flush(20);
    key(window as unknown as Element, 'F7');
    await waitFor(() => useAppStore.getState().activeRoute === 'expenses');
    await waitFor(() => document.activeElement === tid('expense-type-input'));
  });
});
