import React, { useState, useEffect, useRef } from 'react';
import { Settings, Save, DatabaseBackup, ShieldCheck, TriangleAlert } from 'lucide-react';
import { api, errorMessage } from '@/api';
import { pickFolder } from '@/api/pickFolder';
import type { AppConfigView, ResetScope } from '@/types';
import { useAppStore } from '@/store/useAppStore';
import { useThemeStore } from '@/store/useThemeStore';
import { Card, CardHeader, CardTitle, CardContent } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import { TextInput } from '@/components/ui/TextInput';
import { Badge } from '@/components/ui/Badge';
import { Modal } from '@/components/ui/Modal';
import { useCartStore } from '@/store/useCartStore';
import { formatIstDateTime } from '@/lib/dates';

type Notice = { kind: 'success' | 'error'; text: string } | null;

const NoticeLine: React.FC<{ notice: Notice }> = ({ notice }) =>
  notice ? (
    <p
      role={notice.kind === 'error' ? 'alert' : 'status'}
      className={`text-xs font-medium ${
        notice.kind === 'error'
          ? 'text-rose-600 dark:text-rose-400'
          : 'text-emerald-600 dark:text-emerald-400'
      }`}
    >
      {notice.text}
    </p>
  ) : null;

const RESET_PHRASE: Record<ResetScope, string> = { Transactions: 'RESET', All: 'DELETE ALL' };

export const SettingsPage: React.FC = () => {
  const setShopProfile = useAppStore((s) => s.setShopProfile);
  const setDbConnected = useAppStore((s) => s.setDbConnected);
  const theme = useThemeStore((s) => s.theme);
  const toggleTheme = useThemeStore((s) => s.toggleTheme);

  const [id, setId] = useState<number>(1);
  const [shopName, setShopName] = useState('');
  const [phone, setPhone] = useState('');
  const [address, setAddress] = useState('');
  const [taxId, setTaxId] = useState('');
  const [openingCash, setOpeningCash] = useState('0');
  const [isSaving, setIsSaving] = useState(false);
  const [profileNotice, setProfileNotice] = useState<Notice>(null);

  const [config, setConfig] = useState<AppConfigView | null>(null);
  const [isBackingUp, setIsBackingUp] = useState(false);
  const [isVerifying, setIsVerifying] = useState(false);
  const [dbNotice, setDbNotice] = useState<Notice>(null);
  const [integrityOk, setIntegrityOk] = useState<boolean | null>(null);
  const [resetScope, setResetScope] = useState<ResetScope | null>(null);
  const [resetText, setResetText] = useState('');
  const [isResetting, setIsResetting] = useState(false);
  const [resetNotice, setResetNotice] = useState<Notice>(null);
  const [resetError, setResetError] = useState<string | null>(null);
  const [lastBackup, setLastBackup] = useState<{ path: string; at: Date } | null>(null);

  // The shell sets the app-store `requestBackup` flag for Ctrl+B; it runs a backup into the current folder.
  const requestBackup = useAppStore((s) => (s as unknown as { requestBackup?: boolean }).requestBackup);
  const handleBackupRef = useRef<() => void>(() => {});

  useEffect(() => {
    api
      .getAppConfig()
      .then(setConfig)
      .catch((err) => setDbNotice({ kind: 'error', text: `Failed to load backup settings: ${errorMessage(err)}` }));
  }, []);

  useEffect(() => {
    api
      .getShopSettings()
      .then((s) => {
        if (!s) return;
        setId(s.id);
        setShopName(s.shopName);
        setPhone(s.phone ?? '');
        setAddress(s.address ?? '');
        setTaxId(s.taxId ?? '');
        setOpeningCash(String(s.openingCash ?? 0));
      })
      .catch((err) => setProfileNotice({ kind: 'error', text: `Failed to load settings: ${errorMessage(err)}` }));
  }, []);

  const handleSave = async () => {
    if (!shopName.trim()) {
      setProfileNotice({ kind: 'error', text: 'Shop name is required' });
      return;
    }
    setIsSaving(true);
    setProfileNotice(null);
    try {
      await api.updateShopSettings({
        id,
        shopName: shopName.trim(),
        phone: phone.trim() || null,
        address: address.trim() || null,
        taxId: taxId.trim() || null,
        openingCash: openingCash.trim() === '' ? 0 : Number(openingCash),
      });
      void useAppStore.getState().refreshMetrics();
      setShopProfile({
        shopName: shopName.trim(),
        phone: phone.trim(),
        address: address.trim(),
        taxId: taxId.trim(),
      });
      setProfileNotice({ kind: 'success', text: 'Shop profile saved' });
    } catch (err) {
      setProfileNotice({ kind: 'error', text: errorMessage(err) });
    } finally {
      setIsSaving(false);
    }
  };

  const handleBackup = async () => {
    if (!config) return;
    setIsBackingUp(true);
    setDbNotice(null);
    try {
      const written = await api.backupDatabase(config.backupDirectory);
      setDbNotice({ kind: 'success', text: `Backup written to ${written}` });
      setLastBackup({ path: written, at: new Date() });
    } catch (err) {
      setDbNotice({ kind: 'error', text: errorMessage(err) });
    } finally {
      setIsBackingUp(false);
    }
  };

  const applyDirectory = async (path: string | null) => {
    setDbNotice(null);
    try {
      setConfig(await api.setBackupDirectory(path));
    } catch (err) {
      setDbNotice({ kind: 'error', text: errorMessage(err) });
    }
  };

  const handleChoose = async () => {
    try {
      const picked = await pickFolder();
      if (picked) await applyDirectory(picked);
    } catch (err) {
      setDbNotice({ kind: 'error', text: errorMessage(err) });
    }
  };

  const openReset = (scope: ResetScope) => {
    setResetText('');
    setResetError(null);
    setResetScope(scope);
  };
  const closeReset = () => {
    if (!isResetting) setResetScope(null);
  };
  const handleReset = async () => {
    if (!resetScope || resetText !== RESET_PHRASE[resetScope]) return;
    setIsResetting(true);
    setResetError(null);
    try {
      const summary = await api.resetData(resetScope);
      const total = summary.deleted.reduce((n, d) => n + d.rows, 0);
      setResetScope(null);
      setResetNotice({
        kind: 'success',
        text: `Reset complete: ${total} rows deleted. Backup written to ${summary.backupPath}`,
      });
      // Other pages load their data on mount; refresh what is cached app-wide.
      useCartStore.getState().clearCart();
      void useAppStore.getState().refreshMetrics();
      api.getAppConfig().then(setConfig).catch(() => {});
    } catch (err) {
      setResetError(errorMessage(err));
    } finally {
      setIsResetting(false);
    }
  };

  handleBackupRef.current = handleBackup;
  useEffect(() => {
    if (!requestBackup) return;
    useAppStore.setState({ requestBackup: false } as unknown as Partial<ReturnType<typeof useAppStore.getState>>);
    handleBackupRef.current();
  }, [requestBackup]);

  const handleVerify = async () => {
    setIsVerifying(true);
    setDbNotice(null);
    try {
      const ok = await api.verifyDatabaseIntegrity();
      setIntegrityOk(ok);
      setDbConnected(ok);
      setDbNotice(
        ok
          ? { kind: 'success', text: 'Integrity check passed' }
          : { kind: 'error', text: 'Integrity check reported problems' }
      );
    } catch (err) {
      setDbNotice({ kind: 'error', text: errorMessage(err) });
    } finally {
      setIsVerifying(false);
    }
  };

  return (
    <div className="flex flex-col gap-4 max-w-3xl">
      <div className="flex items-center gap-2 px-1">
        <Settings className="h-5 w-5 text-blue-600" />
        <h2 className="text-sm font-bold">Settings & Backup</h2>
      </div>

      <Card>
        <CardHeader>
          <CardTitle>Shop Profile</CardTitle>
        </CardHeader>
        <CardContent className="flex flex-col gap-3">
          <p className="text-xs text-slate-500">Printed at the top of bills</p>
          <TextInput label="Shop Name" value={shopName} onChange={(e) => setShopName(e.target.value)} />
          <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
            <TextInput label="Phone" value={phone} onChange={(e) => setPhone(e.target.value)} />
            <TextInput label="Tax / GST ID" value={taxId} onChange={(e) => setTaxId(e.target.value)} />
          </div>
          <TextInput label="Address" value={address} onChange={(e) => setAddress(e.target.value)} />
          <div className="border-t border-slate-100 dark:border-slate-800 pt-3 flex flex-col gap-1">
            <div className="max-w-xs">
              <TextInput
                label="Opening cash (₹)"
                type="number"
                inputMode="decimal"
                min={0}
                step="0.01"
                data-testid="opening-cash"
                aria-describedby="opening-cash-help"
                className="tabular-nums text-right"
                value={openingCash}
                onChange={(e) => setOpeningCash(e.target.value)}
              />
            </div>
            <p id="opening-cash-help" className="text-xs text-slate-500">
              Cash already in the drawer when you started using Book Keeper
            </p>
          </div>
          <div className="flex items-center gap-3">
            <Button
              variant="primary"
              isLoading={isSaving}
              leftIcon={<Save className="h-4 w-4" />}
              onClick={handleSave}
            >
              Save Profile
            </Button>
            <NoticeLine notice={profileNotice} />
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>Backup & Integrity</CardTitle>
          {integrityOk !== null && (
            <Badge variant={integrityOk ? 'success' : 'danger'}>
              {integrityOk ? 'Database OK' : 'Database Issues'}
            </Badge>
          )}
        </CardHeader>
        <CardContent className="flex flex-col gap-3">
          <p className="text-xs text-slate-500">
            Database file: <span className="font-mono">&lt;app data&gt;/shop.db</span> (or the path in{' '}
            <span className="font-mono">BOOK_KEEPER_DB_PATH</span>).
          </p>
          <div className="text-xs">
            <p className="font-medium text-slate-600 dark:text-slate-300">Backup folder</p>
            <p className="font-mono break-all" data-testid="backup-folder">
              {config?.backupDirectory ?? '…'}
              {config?.isDefaultDirectory && <span className="font-sans text-slate-500"> (default)</span>}
            </p>
          </div>
          <p className="text-xs text-slate-500" data-testid="last-auto-backup">
            Last auto backup:{' '}
            {config?.lastAutoBackupAt ? formatIstDateTime(config.lastAutoBackupAt) : 'Never'}
          </p>
          <div className="flex flex-wrap items-center gap-2">
            <Button variant="outline" onClick={handleChoose}>
              Choose folder…
            </Button>
            <Button
              variant="outline"
              disabled={!config || config.isDefaultDirectory}
              onClick={() => applyDirectory(null)}
            >
              Use default
            </Button>
          </div>
          <p className="text-xs text-slate-500" data-testid="last-backup">
            {lastBackup
              ? `Last backup this session: ${formatIstDateTime(lastBackup.at)} (${lastBackup.path})`
              : 'No backup yet this session.'}
          </p>
          <div className="flex flex-wrap items-center gap-2">
            <Button
              variant="primary"
              isLoading={isBackingUp}
              disabled={!config}
              leftIcon={<DatabaseBackup className="h-4 w-4" />}
              onClick={handleBackup}
            >
              Backup Now
            </Button>
            <Button
              variant="outline"
              isLoading={isVerifying}
              leftIcon={<ShieldCheck className="h-4 w-4" />}
              onClick={handleVerify}
            >
              Verify Integrity
            </Button>
          </div>
          <NoticeLine notice={dbNotice} />
        </CardContent>
      </Card>

      {config?.testingMode === true && (
        <Card className="border-rose-300 dark:border-rose-900">
          <CardHeader>
            <CardTitle>Testing tools</CardTitle>
            <Badge variant="danger">Testing mode</Badge>
          </CardHeader>
          <CardContent className="flex flex-col gap-3">
            <p className="text-xs text-rose-600 dark:text-rose-400 flex items-center gap-1.5">
              <TriangleAlert className="h-4 w-4 shrink-0" />
              For testing only. A backup is written to the backup folder first, but the data is removed.
            </p>
            <div className="flex flex-col gap-1">
              <div>
                <Button variant="danger" onClick={() => openReset('Transactions')}>
                  Clear transactions
                </Button>
              </div>
              <p className="text-xs text-slate-500">Keeps items, bundles, parties; stock returns to opening stock.</p>
            </div>
            <div className="flex flex-col gap-1">
              <div>
                <Button variant="danger" onClick={() => openReset('All')}>
                  Clear all data
                </Button>
              </div>
              <p className="text-xs text-slate-500">Deletes everything except the shop profile.</p>
            </div>
            <NoticeLine notice={resetNotice} />
          </CardContent>
        </Card>
      )}

      <Modal
        isOpen={resetScope !== null}
        onClose={closeReset}
        title={resetScope === 'All' ? 'Clear all data' : 'Clear transactions'}
        description="A backup is written first. This cannot be undone from the app."
        size="sm"
        footer={
          <>
            <Button variant="outline" onClick={closeReset} disabled={isResetting}>
              Cancel
            </Button>
            <Button
              variant="danger"
              isLoading={isResetting}
              disabled={!resetScope || resetText !== RESET_PHRASE[resetScope]}
              onClick={handleReset}
            >
              Confirm
            </Button>
          </>
        }
      >
        {resetScope && (
          <div className="flex flex-col gap-3">
            <TextInput
              label={`Type ${RESET_PHRASE[resetScope]} to confirm`}
              value={resetText}
              onChange={(e) => setResetText(e.target.value)}
              autoComplete="off"
            />
            {resetError && (
              <p role="alert" className="text-xs font-medium text-rose-600 dark:text-rose-400">
                {resetError}
              </p>
            )}
          </div>
        )}
      </Modal>

      <Card>
        <CardHeader>
          <CardTitle>Appearance</CardTitle>
        </CardHeader>
        <CardContent>
          <Button variant="outline" onClick={toggleTheme}>
            Switch to {theme === 'dark' ? 'Light' : 'Dark'} Theme
          </Button>
        </CardContent>
      </Card>
    </div>
  );
};

export default SettingsPage;
