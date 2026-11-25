import React, { useCallback, useState } from 'react';
import { ArrowDown, ArrowUp, Search } from 'lucide-react';
import { api } from '@/api';
import { cn } from '@/lib/utils';
import type { ItemSearchResult, ReportSort } from '@/types';
import { TableHead } from '../ui/Table';
import { ItemDetailsDrawer } from './ItemDetailsDrawer';
import { ItemEditDrawer } from './ItemEditDrawer';

export const REPORT_PAGE_SIZE = 20;

/** Header cell that sorts by `field` (sorting itself runs in the backend). */
export const SortableHead: React.FC<{
  field: string;
  label: string;
  sort: ReportSort;
  onSort: (field: string) => void;
  align?: 'left' | 'right';
}> = ({ field, label, sort, onSort, align = 'left' }) => {
  const active = sort.field === field;
  return (
    <TableHead
      align={align}
      aria-sort={active ? (sort.dir === 'ASC' ? 'ascending' : 'descending') : 'none'}
    >
      <button
        type="button"
        data-testid={`sort-${field}`}
        onClick={() => onSort(field)}
        className={cn(
          'inline-flex items-center gap-1 uppercase tracking-wider cursor-pointer hover:text-blue-600 dark:hover:text-blue-400',
          active && 'text-blue-600 dark:text-blue-400'
        )}
      >
        {label}
        {active && (sort.dir === 'ASC' ? <ArrowUp className="h-3 w-3" /> : <ArrowDown className="h-3 w-3" />)}
      </button>
    </TableHead>
  );
};

/** Next sort when `field` is clicked: same field flips direction; a new field starts as given. */
export function nextSort(sort: ReportSort, field: string, firstDir: 'ASC' | 'DESC'): ReportSort {
  if (sort.field === field) return { field, dir: sort.dir === 'ASC' ? 'DESC' : 'ASC' };
  return { field, dir: firstDir };
}

export const ReportSearch: React.FC<{
  value: string;
  onChange: (v: string) => void;
  label: string;
  placeholder: string;
}> = ({ value, onChange, label, placeholder }) => (
  <div className="relative min-w-[200px] max-w-xs flex-1">
    <Search className="absolute left-2.5 top-2.5 text-slate-400" size={16} />
    <input
      type="text"
      aria-label={label}
      placeholder={placeholder}
      value={value}
      onChange={(e) => onChange(e.target.value)}
      className="w-full pl-8 pr-3 py-1.5 text-sm bg-slate-50 dark:bg-slate-800 border border-slate-300 dark:border-slate-700 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500"
    />
  </div>
);

/** Row click on a report list opens Item details (and its Edit drawer), as on Inventory. */
export function useReportItemDrawers(): {
  open: (r: { itemId: number; name: string; baseUnit: string }) => void;
  drawers: React.ReactNode;
} {
  const [details, setDetails] = useState<ItemSearchResult | null>(null);
  const [editing, setEditing] = useState<ItemSearchResult | null>(null);
  const open = useCallback((r: { itemId: number; name: string; baseUnit: string }) => {
    api.getItem(r.itemId).then(setDetails).catch(() => setDetails(null));
  }, []);
  const drawers = (
    <>
      <ItemDetailsDrawer
        item={details}
        onClose={() => setDetails(null)}
        onEdit={(item) => {
          setDetails(null);
          setEditing(item);
        }}
      />
      <ItemEditDrawer isOpen={editing !== null} item={editing} onClose={() => setEditing(null)} onSaved={() => {}} />
    </>
  );
  return { open, drawers };
}
