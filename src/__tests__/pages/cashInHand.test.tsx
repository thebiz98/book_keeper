import { describe, it, expect, vi } from 'vitest';
import { render } from '../testUtils';
import { Header } from '@/components/layout/Header';
import { SettingsPage } from '@/pages/SettingsPage';
import { useAppStore } from '@/store/useAppStore';
import { installApi, flush, waitFor, click, buttonByText, bodyText, setInput } from './helpers';

describe('Cash in hand (FR-33)', () => {
  it('the header pill is labelled "Cash in hand" and shows the backend value', async () => {
    installApi({
      getShopSettings: { id: 1, shopName: 'Shop', phone: null, address: null, taxId: null, openingCash: 0 },
      getDashboardMetrics: { cashInHand: 12345.5 },
    });
    render(<Header />);
    await waitFor(() => bodyText().includes('₹12,345.50'));
    expect(bodyText()).toContain('Cash in hand');
    expect(useAppStore.getState().cashInHand).toBe(12345.5);
  });

  it('Settings loads the opening cash, saves it with the shop settings and refreshes the metrics', async () => {
    const api = installApi({
      getShopSettings: { id: 1, shopName: 'Shop', phone: null, address: null, taxId: null, openingCash: 500 },
      getAppConfig: { testingMode: false, backupDirectory: 'C:/b', isDefaultDirectory: true, lastAutoBackupAt: null },
      updateShopSettings: async () => undefined,
      getDashboardMetrics: { cashInHand: 750 },
    });
    render(<SettingsPage />);
    const field = () => document.querySelector('[data-testid="opening-cash"]') as HTMLInputElement;
    await waitFor(() => field()?.value === '500');
    expect(bodyText()).toContain('Cash already in the drawer when you started using Book Keeper');
    setInput(field(), '750.5');
    await click(buttonByText('Save Profile'));
    await flush();
    expect(api.updateShopSettings).toHaveBeenCalledWith(expect.objectContaining({ shopName: 'Shop', openingCash: 750.5 }));
    expect(api.getDashboardMetrics).toHaveBeenCalled();
    await waitFor(() => useAppStore.getState().cashInHand === 750);
  });

  it('shows the backend message when the opening cash is rejected', async () => {
    installApi({
      getShopSettings: { id: 1, shopName: 'Shop', phone: null, address: null, taxId: null, openingCash: 0 },
      getAppConfig: { testingMode: false, backupDirectory: 'C:/b', isDefaultDirectory: true, lastAutoBackupAt: null },
      updateShopSettings: vi.fn(async () => {
        throw { type: 'ValidationError', field: 'opening_cash', message: 'Opening cash must be a number, zero or more' };
      }),
    });
    render(<SettingsPage />);
    await waitFor(() => !!document.querySelector('[data-testid="opening-cash"]'));
    setInput(document.querySelector('[data-testid="opening-cash"]')!, '-5');
    await click(buttonByText('Save Profile'));
    await flush();
    expect(bodyText()).toContain('Opening cash must be a number');
  });
});
