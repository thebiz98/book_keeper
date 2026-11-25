import { describe, it, expect, beforeEach, vi } from 'vitest';
import { useAppStore } from '@/store/useAppStore';
import { installApi } from '../components/saleFixtures';

const reset = () =>
  useAppStore.setState({
    activeRoute: 'dashboard',
    shopProfile: { shopName: '' },
    shopLoaded: false,
    cashInHand: 0,
    lookupOpen: false,
    backupRequested: false,
    isDbConnected: true,
    dbLabel: 'shop.db',
  });

describe('useAppStore (shell)', () => {
  beforeEach(reset);

  it('has no fake shop data by default and routes', () => {
    expect(useAppStore.getState().shopProfile.shopName).toBe('');
    useAppStore.getState().setActiveRoute('sale');
    expect(useAppStore.getState().activeRoute).toBe('sale');
  });

  it('loadShopSettings fills the profile from the backend', async () => {
    installApi();
    await useAppStore.getState().loadShopSettings();
    const st = useAppStore.getState();
    expect(st.shopProfile.shopName).toBe('Test Shop');
    expect(st.shopLoaded).toBe(true);
  });

  it('loadShopSettings failure keeps the empty profile', async () => {
    installApi({ getShopSettings: vi.fn(async () => { throw new Error('x'); }) });
    await useAppStore.getState().loadShopSettings();
    expect(useAppStore.getState().shopProfile.shopName).toBe('');
    expect(useAppStore.getState().shopLoaded).toBe(true);
  });

  it('refreshMetrics reads cashInHand from the dashboard metrics', async () => {
    const api = installApi();
    await useAppStore.getState().refreshMetrics();
    expect(api.getDashboardMetrics).toHaveBeenCalled();
    expect(useAppStore.getState().cashInHand).toBe(123);
  });

  it('refreshMetrics keeps the last value on failure', async () => {
    installApi({ getDashboardMetrics: vi.fn(async () => { throw new Error('x'); }) });
    useAppStore.setState({ cashInHand: 55 });
    await useAppStore.getState().refreshMetrics();
    expect(useAppStore.getState().cashInHand).toBe(55);
  });

  it('lookup open/close', () => {
    useAppStore.getState().openLookup();
    expect(useAppStore.getState().lookupOpen).toBe(true);
    useAppStore.getState().closeLookup();
    expect(useAppStore.getState().lookupOpen).toBe(false);
  });

  it('requestBackup routes to settings and sets the flag; consume clears it once', () => {
    useAppStore.getState().requestBackup();
    expect(useAppStore.getState().activeRoute).toBe('settings');
    expect(useAppStore.getState().backupRequested).toBe(true);
    expect(useAppStore.getState().consumeBackupRequest()).toBe(true);
    expect(useAppStore.getState().consumeBackupRequest()).toBe(false);
  });

  it('setShopProfile merges and setDbConnected toggles', () => {
    useAppStore.getState().setShopProfile({ shopName: 'A', phone: '1' });
    useAppStore.getState().setShopProfile({ address: 'B' });
    expect(useAppStore.getState().shopProfile).toEqual({ shopName: 'A', phone: '1', address: 'B' });
    useAppStore.getState().setDbConnected(false);
    expect(useAppStore.getState().isDbConnected).toBe(false);
  });
});
