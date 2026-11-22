import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render } from '../testUtils';
import { SettingsPage } from '@/pages/SettingsPage';
import type { AppConfigView } from '@/types';
import { installApi, flush, waitFor, click, buttonByText, bodyText } from './helpers';

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
