import React from 'react';
import { useAppStore } from '@/store/useAppStore';
import {
  HOTKEYS,
  hotkeyBadge,
  hotkeyLabel,
  displayCombo,
  type HotkeyAction,
} from '@/hooks/useHotkeys';

interface Hint {
  action?: HotkeyAction;
  /** Literal combo for keys that are not registry actions. */
  combo?: string;
  label?: string;
}

const GLOBAL_ACTIONS: HotkeyAction[] = ['dashboard', 'sale', 'purchase', 'lookup', 'dues', 'inventory', 'expenses'];

function hintsFor(route: string): Hint[] {
  switch (route) {
    case 'sale':
    case 'billing':
    case 'new-sale':
      return [
        { action: 'completeBill', label: 'Complete Bill' },
        { action: 'addCharge' },
        { action: 'newBundle' },
        { action: 'newItem' },
        { action: 'newCustomer' },
        { action: 'back' },
      ];
    case 'purchase':
    case 'purchases':
    case 'new-purchase':
      return [
        { action: 'completeBill', label: 'Record Inward' },
        { action: 'addCharge' },
        { action: 'newBundle' },
        { action: 'newItem' },
        { action: 'newVendor' },
      ];
    case 'dues':
    case 'credit-dues':
      return [{ combo: 'Enter', label: 'Settle Bill Payment' }];
    case 'inventory':
    case 'items':
      return [{ action: 'newBundle' }, { action: 'lookup', label: 'Barcode Lookup' }];
    case 'expenses':
      return [{ action: 'completeBill', label: 'Save Expense' }, { action: 'back' }];
    case 'settings':
      return [{ action: 'backup', label: 'Backup Database' }];
    default:
      return [];
  }
}

const Key: React.FC<{ combo: string; label: string }> = ({ combo, label }) => (
  <div className="flex items-center gap-1 select-none">
    <kbd className="px-1.5 py-0.5 rounded bg-slate-200 dark:bg-slate-800 text-slate-700 dark:text-slate-300 font-mono text-[10px] font-semibold border border-slate-300 dark:border-slate-700 shadow-2xs">
      {combo}
    </kbd>
    <span className="text-[11px] text-slate-600 dark:text-slate-400 font-medium">{label}</span>
  </div>
);

export const HotkeyBar: React.FC = () => {
  const activeRoute = useAppStore((s) => s.activeRoute);

  return (
    <footer className="h-8 shrink-0 border-t border-slate-200 dark:border-slate-800 bg-slate-50 dark:bg-slate-900 px-4 flex items-center justify-between text-xs z-30 overflow-x-auto">
      <div className="flex items-center gap-3">
        {GLOBAL_ACTIONS.map((a) => (
          <Key key={a} combo={hotkeyBadge(a)} label={hotkeyLabel(a)} />
        ))}
      </div>

      <div className="flex items-center gap-3 pl-3">
        {hintsFor(activeRoute).map((h) =>
          h.action ? (
            <Key
              key={h.action}
              combo={displayCombo(HOTKEYS[h.action].combos[0])}
              label={h.label ?? hotkeyLabel(h.action)}
            />
          ) : (
            <Key key={h.combo} combo={h.combo ?? ''} label={h.label ?? ''} />
          )
        )}
      </div>
    </footer>
  );
};

export default HotkeyBar;
