import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render } from '../testUtils';
import { SettingsPage } from '@/pages/SettingsPage';
import type { AppConfigView } from '@/types';
import { installApi, flush, waitFor, click, buttonByText, bodyText, setInput } from './helpers';

const pick = vi.hoisted(() => vi.fn());
vi.mock('@/api/pickFolder', () => ({ pickFolder: pick }));

const view = (over: Partial<AppConfigView> = {}): AppConfigView => ({
  testingMode: false,
  backupDirectory: 'C:/data/backups',
  isDefaultDirectory: true,
  lastAutoBackupAt: null,
  ...over,
});

function setup(initial = view()) {
  return installApi({
    getShopSettings: { id: 1, shopName: 'Shop', phone: null, address: null, taxId: null },
    getAppConfig: initial,
    setBackupDirectory: async (p: string | null) =>
      p ? view({ backupDirectory: p, isDefaultDirectory: false }) : view(),
    backupDatabase: async (d: string) => `${d}/x.db`,
  });
}

describe('SettingsPage backup card', () => {
  beforeEach(() => pick.mockReset());

  it('shows the configured folder read-only (no path input)', async () => {
    setup();
    const { container } = render(<SettingsPage />);
    await waitFor(() => bodyText().includes('C:/data/backups'));
    expect(bodyText()).toContain('(default)');
    expect(bodyText()).toContain('Never');
    const inputs = Array.from(container.querySelectorAll('input')).map((i) => i.value);
    expect(inputs).not.toContain('C:/data/backups');
  });

  it('Choose folder calls setBackupDirectory with the picked path', async () => {
    const api = setup();
    pick.mockResolvedValue('D:/mine');
    render(<SettingsPage />);
    await waitFor(() => bodyText().includes('C:/data/backups'));
    await click(buttonByText('Choose folder…'));
    await waitFor(() => bodyText().includes('D:/mine'));
    expect(api.setBackupDirectory).toHaveBeenCalledWith('D:/mine');
    expect(bodyText()).not.toContain('(default)');
  });

  it('Use default resets with null', async () => {
    const api = setup(view({ backupDirectory: 'D:/mine', isDefaultDirectory: false }));
    render(<SettingsPage />);
    await waitFor(() => bodyText().includes('D:/mine'));
    await click(buttonByText('Use default'));
    await flush();
    expect(api.setBackupDirectory).toHaveBeenCalledWith(null);
  });

  it('Backup Now backs up into the current folder', async () => {
    const api = setup();
    render(<SettingsPage />);
    await waitFor(() => bodyText().includes('C:/data/backups'));
    await click(buttonByText('Backup Now'));
    await flush();
    expect(api.backupDatabase).toHaveBeenCalledWith('C:/data/backups');
  });
});

describe('SettingsPage testing tools', () => {
  const summary = (scope: 'Transactions' | 'All') => ({
    scope,
    deleted: [
      { table: 'sales', rows: 3 },
      { table: 'payments', rows: 2 },
    ],
    backupPath: 'C:/data/backups/book_keeper_prereset_1.db',
  });
  const dialogInput = () => document.querySelector('[role="dialog"] input') as HTMLInputElement;
  const confirmBtn = () => buttonByText('Confirm');

  function setupTesting(over: Record<string, unknown> = {}) {
    return installApi({
      getShopSettings: { id: 1, shopName: 'Shop', phone: null, address: null, taxId: null },
      getAppConfig: view({ testingMode: true }),
      resetData: async (scope: 'Transactions' | 'All') => summary(scope),
      getDashboardMetrics: { liveCashInDrawer: 0 },
      ...over,
    });
  }

  it('is hidden when testing mode is off', async () => {
    setup();
    render(<SettingsPage />);
    await waitFor(() => bodyText().includes('C:/data/backups'));
    expect(bodyText()).not.toContain('Testing tools');
  });

  it('is visible when testing mode is on', async () => {
    setupTesting();
    render(<SettingsPage />);
    await waitFor(() => bodyText().includes('Testing tools'));
    expect(bodyText()).toContain('Clear transactions');
    expect(bodyText()).toContain('Clear all data');
  });

  it('keeps Confirm disabled until the exact phrase is typed', async () => {
    const api = setupTesting();
    render(<SettingsPage />);
    await waitFor(() => bodyText().includes('Testing tools'));
    await click(buttonByText('Clear transactions'));
    await flush();
    expect(confirmBtn().disabled).toBe(true);
    setInput(dialogInput(), 'reset');
    expect(confirmBtn().disabled).toBe(true);
    setInput(dialogInput(), 'WRONG');
    expect(confirmBtn().disabled).toBe(true);
    setInput(dialogInput(), 'RESET');
    expect(confirmBtn().disabled).toBe(false);
    expect(api.resetData).not.toHaveBeenCalled();
  });

  it('RESET clears transactions and shows the backup path and row total', async () => {
    const api = setupTesting();
    render(<SettingsPage />);
    await waitFor(() => bodyText().includes('Testing tools'));
    await click(buttonByText('Clear transactions'));
    await flush();
    setInput(dialogInput(), 'RESET');
    await click(confirmBtn());
    await waitFor(() => bodyText().includes('Reset complete'));
    expect(api.resetData).toHaveBeenCalledWith('Transactions');
    expect(bodyText()).toContain('5 rows deleted');
    expect(bodyText()).toContain('book_keeper_prereset_1.db');
    expect(document.querySelector('[role="dialog"]')).toBeNull();
  });

  it('DELETE ALL clears everything', async () => {
    const api = setupTesting();
    render(<SettingsPage />);
    await waitFor(() => bodyText().includes('Testing tools'));
    await click(buttonByText('Clear all data'));
    await flush();
    setInput(dialogInput(), 'RESET');
    expect(confirmBtn().disabled).toBe(true);
    setInput(dialogInput(), 'DELETE ALL');
    await click(confirmBtn());
    await waitFor(() => bodyText().includes('Reset complete'));
    expect(api.resetData).toHaveBeenCalledWith('All');
  });

  it('shows an OperationNotAllowed error and keeps the dialog open', async () => {
    setupTesting({
      resetData: async () => {
        throw { type: 'OperationNotAllowed', details: 'Reset is disabled outside testing mode' };
      },
    });
    render(<SettingsPage />);
    await waitFor(() => bodyText().includes('Testing tools'));
    await click(buttonByText('Clear transactions'));
    await flush();
    setInput(dialogInput(), 'RESET');
    await click(confirmBtn());
    await waitFor(() => bodyText().includes('Reset is disabled outside testing mode'));
    expect(document.querySelector('[role="dialog"]')).not.toBeNull();
  });
});
