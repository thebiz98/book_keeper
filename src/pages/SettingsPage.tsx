import React, { useState, useEffect, useRef } from 'react';
import { Settings, Save, DatabaseBackup, ShieldCheck } from 'lucide-react';
import { api, errorMessage } from '@/api';
import { useAppStore } from '@/store/useAppStore';
import { useThemeStore } from '@/store/useThemeStore';
import { Card, CardHeader, CardTitle, CardContent } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import { TextInput } from '@/components/ui/TextInput';
import { Badge } from '@/components/ui/Badge';

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
  const [isSaving, setIsSaving] = useState(false);
  const [profileNotice, setProfileNotice] = useState<Notice>(null);

  const [backupPath, setBackupPath] = useState('');
  const [isBackingUp, setIsBackingUp] = useState(false);
  const [isVerifying, setIsVerifying] = useState(false);
  const [dbNotice, setDbNotice] = useState<Notice>(null);
  const [integrityOk, setIntegrityOk] = useState<boolean | null>(null);
  const [lastBackup, setLastBackup] = useState<{ path: string; at: Date } | null>(null);
  const backupInput = useRef<HTMLInputElement>(null);

  // TODO(F6-integrate): the shell adds an app-store `requestBackup` flag for Ctrl+B. Typed loosely
  // until it lands. We cannot pick a folder natively, so we run the backup when a folder is set,
  // otherwise focus the folder field.
  const requestBackup = useAppStore((s) => (s as unknown as { requestBackup?: boolean }).requestBackup);
  const handleBackupRef = useRef<() => void>(() => {});

  useEffect(() => {
    try {
      const saved = localStorage.getItem('bk.backupDir');
      if (saved) setBackupPath(saved);
    } catch {
      /* storage unavailable */
    }
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
      });
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
    if (!backupPath.trim()) {
      setDbNotice({ kind: 'error', text: 'Enter a destination folder for the backup' });
      backupInput.current?.focus();
      return;
    }
    setIsBackingUp(true);
    setDbNotice(null);
    try {
      const written = await api.backupDatabase(backupPath.trim());
      setDbNotice({ kind: 'success', text: `Backup written to ${written}` });
      setLastBackup({ path: written, at: new Date() });
      try {
        localStorage.setItem('bk.backupDir', backupPath.trim());
      } catch {
        /* storage unavailable */
      }
    } catch (err) {
      setDbNotice({ kind: 'error', text: errorMessage(err) });
    } finally {
      setIsBackingUp(false);
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
          <TextInput label="Shop Name" value={shopName} onChange={(e) => setShopName(e.target.value)} />
          <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
            <TextInput label="Phone" value={phone} onChange={(e) => setPhone(e.target.value)} />
            <TextInput label="Tax / GST ID" value={taxId} onChange={(e) => setTaxId(e.target.value)} />
          </div>
          <TextInput label="Address" value={address} onChange={(e) => setAddress(e.target.value)} />
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
          <TextInput
            ref={backupInput}
            label="Backup Destination Folder (must already exist)"
            placeholder="/storage/emulated/0/Backups"
            value={backupPath}
            onChange={(e) => setBackupPath(e.target.value)}
          />
          <p className="text-xs text-slate-500" data-testid="last-backup">
            {lastBackup
              ? `Last backup this session: ${lastBackup.at.toLocaleString()} (${lastBackup.path})`
              : 'No backup yet this session.'}
          </p>
          <div className="flex flex-wrap items-center gap-2">
            <Button
              variant="primary"
              isLoading={isBackingUp}
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
