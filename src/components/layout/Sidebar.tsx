import React from 'react';
import {
  LayoutDashboard,
  ShoppingCart,
  Truck,
  Package,
  BookOpen,
  Receipt,
  ClipboardList,
  RotateCcw,
  Users,
  BarChart3,
  Settings,
} from 'lucide-react';
import { cn } from '@/lib/utils';
import { useAppStore } from '@/store/useAppStore';
import { hotkeyBadge, type HotkeyAction } from '@/hooks/useHotkeys';

interface NavItem {
  id: string;
  aliases: string[];
  label: string;
  icon: React.ComponentType<{ className?: string }>;
  hotkey?: HotkeyAction;
  isPrimary?: boolean;
}

const MAIN_ITEMS: NavItem[] = [
  {
    id: 'dashboard',
    aliases: ['dashboard'],
    label: 'Dashboard',
    icon: LayoutDashboard,
    hotkey: 'dashboard',
  },
  {
    id: 'sale',
    aliases: ['sale', 'billing', 'new-sale'],
    label: 'New Sale',
    icon: ShoppingCart,
    hotkey: 'sale',
    isPrimary: true,
  },
  {
    id: 'purchase',
    aliases: ['purchase', 'purchases', 'new-purchase'],
    label: 'New Purchase',
    icon: Truck,
    hotkey: 'purchase',
  },
  {
    id: 'dues',
    aliases: ['dues', 'credit-dues'],
    label: 'Khata Dues',
    icon: BookOpen,
    hotkey: 'dues',
  },
  {
    id: 'inventory',
    aliases: ['inventory', 'items'],
    label: 'Inventory Master',
    icon: Package,
    hotkey: 'inventory',
  },
];

const OPERATIONS_ITEMS: NavItem[] = [
  {
    id: 'sales-history',
    aliases: ['sales-history'],
    label: 'Sales History',
    icon: Receipt,
  },
  {
    id: 'purchase-history',
    aliases: ['purchase-history'],
    label: 'Purchase History',
    icon: ClipboardList,
  },
  {
    id: 'returns',
    aliases: ['returns', 'returns-adjustments'],
    label: 'Returns & Adjustments',
    icon: RotateCcw,
  },
  {
    id: 'parties',
    aliases: ['parties'],
    label: 'Parties Directory',
    icon: Users,
  },
  {
    id: 'reports',
    aliases: ['reports'],
    label: 'Financial Reports',
    icon: BarChart3,
  },
];

const SYSTEM_ITEMS: NavItem[] = [
  {
    id: 'settings',
    aliases: ['settings'],
    label: 'Settings & Backup',
    icon: Settings,
    hotkey: 'backup',
  },
];

export const Sidebar: React.FC = () => {
  const activeRoute = useAppStore((s) => s.activeRoute);
  const setActiveRoute = useAppStore((s) => s.setActiveRoute);

  const isItemActive = (item: NavItem): boolean => {
    return item.aliases.includes(activeRoute) || item.id === activeRoute;
  };

  const renderSection = (title: string, items: NavItem[]) => (
    <div className="flex flex-col gap-1 mb-4">
      <span className="px-3 text-[10px] font-bold uppercase tracking-wider text-slate-400 dark:text-slate-500 mb-1 select-none">
        {title}
      </span>
      {items.map((item) => {
        const active = isItemActive(item);
        const Icon = item.icon;

        return (
          <button
            key={item.id}
            type="button"
            aria-current={active ? 'page' : undefined}
            onClick={() => setActiveRoute(item.id)}
            className={cn(
              'flex items-center justify-between px-3 py-2 rounded-md text-xs font-medium transition-colors select-none cursor-pointer',
              active
                ? item.isPrimary
                  ? 'bg-blue-600 text-white font-semibold shadow-xs'
                  : 'bg-blue-50 text-blue-700 dark:bg-blue-950/60 dark:text-blue-300 font-semibold border-l-4 border-blue-600'
                : 'text-slate-600 dark:text-slate-400 hover:bg-slate-100 dark:hover:bg-slate-800 hover:text-slate-900 dark:hover:text-slate-200'
            )}
          >
            <div className="flex items-center gap-2.5">
              <Icon className={cn('h-4 w-4 shrink-0', active && item.isPrimary ? 'text-white' : '')} />
              <span>{item.label}</span>
            </div>

            {item.hotkey && (
              <kbd
                className={cn(
                  'px-1.5 py-0.5 rounded text-[10px] font-mono font-semibold uppercase',
                  active && item.isPrimary
                    ? 'bg-blue-700 text-white'
                    : 'bg-black/5 dark:bg-white/10 text-slate-500 dark:text-slate-400'
                )}
              >
                {hotkeyBadge(item.hotkey)}
              </kbd>
            )}
          </button>
        );
      })}
    </div>
  );

  return (
    <aside className="w-56 shrink-0 bg-white dark:bg-slate-900 border-r border-slate-200 dark:border-slate-800 p-3 flex flex-col justify-between overflow-y-auto z-20">
      <div>
        {renderSection('Counter Operations', MAIN_ITEMS)}
        {renderSection('Logs & Audits', OPERATIONS_ITEMS)}
        {renderSection('System & Database', SYSTEM_ITEMS)}
      </div>

      <div className="pt-3 border-t border-slate-100 dark:border-slate-800 text-[10px] text-slate-400 text-center select-none">
        Book Keeper Desktop v0.1.0
      </div>
    </aside>
  );
};

export default Sidebar;
