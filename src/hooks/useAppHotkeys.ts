import { useAppStore } from '@/store/useAppStore';
import { useHotkeyAction } from './useHotkeys';

/**
 * Registers the DD-19 global hotkeys (F1-F6 navigation, F4 / Ctrl+F lookup,
 * Ctrl+B backup). All are `global` scope: blocked while a modal is open or a
 * submit is in flight, and `e.repeat` is ignored.
 */
export function useAppHotkeys(): void {
  const setActiveRoute = useAppStore((s) => s.setActiveRoute);
  const openLookup = useAppStore((s) => s.openLookup);
  const requestBackup = useAppStore((s) => s.requestBackup);

  useHotkeyAction('dashboard', () => setActiveRoute('dashboard'), { scope: 'global' });
  useHotkeyAction('sale', () => setActiveRoute('sale'), { scope: 'global' });
  useHotkeyAction('purchase', () => setActiveRoute('purchase'), { scope: 'global' });
  useHotkeyAction('lookup', openLookup, { scope: 'global' });
  useHotkeyAction('dues', () => setActiveRoute('dues'), { scope: 'global' });
  useHotkeyAction('inventory', () => setActiveRoute('inventory'), { scope: 'global' });
  useHotkeyAction('backup', requestBackup, { scope: 'global' });
}
