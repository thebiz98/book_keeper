import React, { useState, useEffect } from 'react';
import { Sun, Moon, Database, Clock, Search } from 'lucide-react';
import { cn } from '@/lib/utils';
import { useAppStore } from '@/store/useAppStore';
import { useThemeStore } from '@/store/useThemeStore';
import { CurrencyDisplay } from '../common/CurrencyDisplay';

const ROUTE_TITLES: Record<string, string> = {
  dashboard: 'Executive Command Cockpit',
  sale: 'Counter Billing & Checkout',
  billing: 'Counter Billing & Checkout',
  'new-sale': 'Counter Billing & Checkout',
  purchase: 'Inward FIFO Stock Entry',
  purchases: 'Inward FIFO Stock Entry',
  'new-purchase': 'Inward FIFO Stock Entry',
  inventory: 'Item Master & Stock Valuation',
  items: 'Item Master & Stock Valuation',
  dues: 'Khata Credit Ledger & Settlements',
  'credit-dues': 'Khata Credit Ledger & Settlements',
  'sales-history': 'Sales Audit & Invoices Log',
  'purchase-history': 'Purchase Receipts Audit Log',
  returns: 'Returns, Refunds & Stock Adjustments',
  'returns-adjustments': 'Returns, Refunds & Stock Adjustments',
  reports: 'Financial Reports & P&L Analysis',
  parties: 'Parties & Accounts Directory',
  settings: 'Shop Configuration & Backups',
  migration: 'Legacy VedBill Data Migration',
  'data-migration': 'Legacy VedBill Data Migration',
};

export const Header: React.FC = () => {
  const activeRoute = useAppStore((s) => s.activeRoute);
  const shopProfile = useAppStore((s) => s.shopProfile);
  const liveCashInDrawer = useAppStore((s) => s.liveCashInDrawer);
  const isDbConnected = useAppStore((s) => s.isDbConnected);
  const setActiveRoute = useAppStore((s) => s.setActiveRoute);

  const theme = useThemeStore((s) => s.theme);
  const toggleTheme = useThemeStore((s) => s.toggleTheme);

  const [currentTime, setCurrentTime] = useState<Date>(new Date());

  useEffect(() => {
    const timer = setInterval(() => setCurrentTime(new Date()), 1000);
    return () => clearInterval(timer);
  }, []);

  const routeTitle = ROUTE_TITLES[activeRoute] || 'Retail Dashboard';
  const shopName = shopProfile?.shopName || 'Book Keeper';

  const formattedDateTime = currentTime.toLocaleString('en-IN', {
    day: '2-digit',
    month: 'short',
    year: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit',
    hour12: true,
  });

  return (
    <header className="h-14 shrink-0 bg-white dark:bg-slate-900 border-b border-slate-200 dark:border-slate-800 px-4 flex items-center justify-between z-30 select-none">
      {/* Left: Shop Name & Route Title */}
      <div className="flex items-center gap-3">
        <div className="flex items-center gap-2">
          <div className="h-8 w-8 rounded-lg bg-blue-600 text-white flex items-center justify-center font-bold text-base shadow-xs">
            BK
          </div>
          <div className="flex flex-col">
            <span className="text-xs font-bold uppercase tracking-wider text-slate-900 dark:text-slate-100 leading-tight">
              {shopName}
            </span>
            <span className="text-[11px] text-slate-500 dark:text-slate-400 font-medium leading-tight">
              {routeTitle}
            </span>
          </div>
        </div>
      </div>

      {/* Middle: Live Clock & Search Trigger */}
      <div className="hidden md:flex items-center gap-4">
        <button
          type="button"
          onClick={() => setActiveRoute('inventory')}
          className="flex items-center gap-2 px-3 py-1.5 rounded-md bg-slate-100 dark:bg-slate-800 hover:bg-slate-200 dark:hover:bg-slate-700 text-xs text-slate-600 dark:text-slate-300 transition-colors cursor-pointer border border-slate-200 dark:border-slate-700"
        >
          <Search className="h-3.5 w-3.5 text-slate-400" />
          <span>Search Items & Barcodes...</span>
          <kbd className="px-1.5 py-0.5 rounded bg-white dark:bg-slate-900 text-[10px] font-mono font-semibold border border-slate-300 dark:border-slate-700">
            F4
          </kbd>
        </button>

        <div className="flex items-center gap-1.5 text-xs text-slate-500 font-mono tabular-nums">
          <Clock className="h-3.5 w-3.5 text-slate-400" />
          <span>{formattedDateTime}</span>
        </div>
      </div>

      {/* Right: Cash Drawer Pill, DB Indicator, Theme Toggle */}
      <div className="flex items-center gap-3">
        {/* Physical Cash Drawer Pill */}
        <div
          title="Physical Cash in Register Drawer"
          className="flex items-center gap-1.5 px-3 py-1 rounded-full bg-slate-100 dark:bg-slate-800 border border-slate-200 dark:border-slate-700 text-xs font-medium"
        >
          <span className="text-[10px] text-slate-400 uppercase font-semibold">Cash</span>
          <CurrencyDisplay amount={liveCashInDrawer} colored size="sm" />
        </div>

        {/* Database Health Pill */}
        <div
          title={isDbConnected ? 'SQLite WAL Database OK' : 'Database Disconnected'}
          className="flex items-center gap-1.5 px-2.5 py-1 rounded-md text-[11px] font-medium bg-slate-50 dark:bg-slate-800/60 border border-slate-200 dark:border-slate-700"
        >
          <Database className="h-3 w-3 text-slate-400" />
          <span
            className={cn(
              'h-2 w-2 rounded-full',
              isDbConnected ? 'bg-emerald-500 animate-pulse' : 'bg-rose-500'
            )}
          />
          <span className="hidden sm:inline text-slate-600 dark:text-slate-300">
            {isDbConnected ? 'shop.db' : 'Offline'}
          </span>
        </div>

        {/* Theme Toggle */}
        <button
          type="button"
          onClick={toggleTheme}
          title={`Switch to ${theme === 'dark' ? 'Light' : 'Dark'} Mode`}
          className="p-1.5 rounded-md text-slate-500 hover:text-slate-700 dark:text-slate-400 dark:hover:text-slate-200 hover:bg-slate-100 dark:hover:bg-slate-800 transition-colors cursor-pointer"
        >
          {theme === 'dark' ? <Sun className="h-4 w-4" /> : <Moon className="h-4 w-4" />}
        </button>
      </div>
    </header>
  );
};

export default Header;
