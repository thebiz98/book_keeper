import React from 'react';
import { ChevronLeft, ChevronRight } from 'lucide-react';
import { cn } from '@/lib/utils';
import { totalPages } from '@/lib/pagination';

export interface PagerProps {
  page: number;
  total: number;
  limit: number;
  onPageChange: (page: number) => void;
  className?: string;
}

/** Server-side pager: page count is ceil(total / limit). Renders nothing for empty results. */
export const Pager: React.FC<PagerProps> = ({ page, total, limit, onPageChange, className }) => {
  if (total <= 0) return null;
  const pages = totalPages(total, limit);
  const from = (page - 1) * limit + 1;
  const to = Math.min(total, page * limit);
  const btn =
    'p-1 rounded border border-slate-200 dark:border-slate-700 bg-white dark:bg-slate-800 disabled:opacity-40 hover:bg-slate-100 dark:hover:bg-slate-700 cursor-pointer';
  return (
    <div
      className={cn(
        'px-4 py-2 border-t border-slate-100 dark:border-slate-800 bg-slate-50 dark:bg-slate-900/40 flex items-center justify-between text-xs',
        className
      )}
    >
      <span className="text-slate-500 dark:text-slate-400 font-mono" data-testid="pager-summary">
        {from}-{to} of {total} · Page {page} of {pages}
      </span>
      <div className="flex items-center gap-1">
        <button
          type="button"
          aria-label="Previous page"
          disabled={page <= 1}
          onClick={() => onPageChange(page - 1)}
          className={btn}
        >
          <ChevronLeft className="h-3.5 w-3.5" />
        </button>
        <button
          type="button"
          aria-label="Next page"
          disabled={page >= pages}
          onClick={() => onPageChange(page + 1)}
          className={btn}
        >
          <ChevronRight className="h-3.5 w-3.5" />
        </button>
      </div>
    </div>
  );
};

export default Pager;
