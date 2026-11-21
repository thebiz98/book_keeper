import React from 'react';
import { useAppStore } from '@/store/useAppStore';

export const HotkeyBar: React.FC = () => {
  const activeRoute = useAppStore((s) => s.activeRoute);

  const renderKey = (key: string, label: string) => (
    <div className="flex items-center gap-1 select-none">
      <kbd className="px-1.5 py-0.5 rounded bg-slate-200 dark:bg-slate-800 text-slate-700 dark:text-slate-300 font-mono text-[10px] font-semibold border border-slate-300 dark:border-slate-700 shadow-2xs">
        {key}
      </kbd>
      <span className="text-[11px] text-slate-600 dark:text-slate-400 font-medium">{label}</span>
    </div>
  );

  const getRouteSpecificHints = () => {
    switch (activeRoute) {
      case 'sale':
      case 'billing':
      case 'new-sale':
        return (
          <>
            {renderKey('Ctrl+Enter', 'Complete Sale')}
            {renderKey('Alt+B', 'New Bundle')}
            {renderKey('Esc', 'Clear Cart')}
          </>
        );
      case 'purchase':
      case 'purchases':
      case 'new-purchase':
        return (
          <>
            {renderKey('Ctrl+Enter', 'Record Inward')}
            {renderKey('Alt+B', 'New Bundle')}
          </>
        );
      case 'dues':
      case 'credit-dues':
        return <>{renderKey('Enter', 'Settle Bill Payment')}</>;
      case 'inventory':
      case 'items':
        return (
          <>
            {renderKey('Alt+B', 'New Bundle')}
            {renderKey('F4', 'Barcode Lookup')}
          </>
        );
      case 'settings':
        return <>{renderKey('Ctrl+B', 'Backup Database')}</>;
      default:
        return null;
    }
  };

  return (
    <footer className="h-8 shrink-0 border-t border-slate-200 dark:border-slate-800 bg-slate-50 dark:bg-slate-900 px-4 flex items-center justify-between text-xs z-30 overflow-x-auto">
      {/* Global Hotkeys */}
      <div className="flex items-center gap-3">
        {renderKey('F1', 'Dashboard')}
        {renderKey('F2', 'Billing')}
        {renderKey('F3', 'Purchases')}
        {renderKey('F5', 'Khata')}
        {renderKey('F6', 'Inventory')}
      </div>

      {/* Contextual Hotkeys */}
      <div className="hidden sm:flex items-center gap-3">{getRouteSpecificHints()}</div>
    </footer>
  );
};

export default HotkeyBar;
