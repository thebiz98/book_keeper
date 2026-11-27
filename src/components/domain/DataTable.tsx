import React, { useEffect, useState } from 'react';
import { Loader2, AlertCircle } from 'lucide-react';
import { cn } from '@/lib/utils';
import { Table, type TableScroll, TableHeader, TableBody, TableFooter, TableRow, TableHead, TableCell } from '../ui/Table';
import { Pager } from './Pager';

export interface DataTableColumn<T> {
  key: string;
  header: string;
  align?: 'left' | 'center' | 'right';
  mono?: boolean;
  width?: string;
  /** Extra classes on the body cells of this column (e.g. tighter padding). */
  className?: string;
  /** Extra classes on this column's header cell. */
  headerClassName?: string;
  render: (row: T, index: number) => React.ReactNode;
}

export interface DataTableProps<T> {
  columns: DataTableColumn<T>[];
  /** Current page rows (server-side paging). */
  items: T[];
  rowKey: (row: T) => string | number;
  isLoading?: boolean;
  /** Already-formatted error text (use errorMessage(e)). Shown instead of the empty state. */
  error?: string | null;
  onRetry?: () => void;
  emptyMessage?: string;
  onRowClick?: (row: T) => void;
  /** Optional expandable detail row. */
  isExpanded?: (row: T) => boolean;
  renderExpanded?: (row: T) => React.ReactNode;
  dense?: boolean;
  /** Scroll mode of the shared Table (default 'bounded'). Use 'fill' for full-page lists. */
  scroll?: TableScroll;
  className?: string;
  /** Fixed totals row under the rows, keyed by column key (shown only while rows are shown). */
  footer?: Partial<Record<string, React.ReactNode>>;
  /** Test id of the footer row. */
  footerTestId?: string;
  // Server-side paging
  total?: number;
  page?: number;
  limit?: number;
  onPageChange?: (page: number) => void;
}

/**
 * Presentational table with loading / error / empty states, keyboard row navigation
 * (ArrowUp/Down, Home/End, Enter) and an optional server-side Pager.
 */
export function DataTable<T>({
  columns,
  items,
  rowKey,
  isLoading = false,
  error = null,
  onRetry,
  emptyMessage = 'No records found',
  onRowClick,
  isExpanded,
  renderExpanded,
  dense = true,
  scroll = 'bounded',
  className,
  footer,
  footerTestId,
  total,
  page = 1,
  limit = 50,
  onPageChange,
}: DataTableProps<T>) {
  const [active, setActive] = useState(0);

  useEffect(() => {
    setActive(0);
  }, [items]);

  const onKeyDown = (e: React.KeyboardEvent<HTMLDivElement>) => {
    if (e.target instanceof HTMLElement && ['INPUT', 'SELECT', 'TEXTAREA', 'BUTTON'].includes(e.target.tagName)) {
      return;
    }
    if (items.length === 0) return;
    if (e.key === 'ArrowDown') {
      e.preventDefault();
      setActive((a) => Math.min(items.length - 1, a + 1));
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      setActive((a) => Math.max(0, a - 1));
    } else if (e.key === 'Home') {
      e.preventDefault();
      setActive(0);
    } else if (e.key === 'End') {
      e.preventDefault();
      setActive(items.length - 1);
    } else if (e.key === 'Enter' && onRowClick) {
      e.preventDefault();
      onRowClick(items[Math.min(active, items.length - 1)]);
    }
  };

  const colSpan = columns.length;
  const showRows = !isLoading && !error && items.length > 0;

  return (
    <div
      className={cn(
        'flex flex-col rounded-lg border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 overflow-hidden shadow-xs',
        className
      )}
    >
      <div
        className="flex-1 min-h-0 flex flex-col focus:outline-none focus:ring-1 focus:ring-blue-500/40"
        tabIndex={0}
        role="grid"
        aria-rowcount={items.length}
        onKeyDown={onKeyDown}
      >
        <Table dense={dense} scroll={scroll}>
          <TableHeader>
            <TableRow>
              {columns.map((col) => (
                <TableHead
                  key={col.key}
                  align={col.align ?? 'left'}
                  className={col.headerClassName ?? col.className}
                  style={col.width ? { width: col.width } : undefined}
                >
                  {col.header}
                </TableHead>
              ))}
            </TableRow>
          </TableHeader>
          <TableBody>
            {isLoading && (
              <TableRow>
                <TableCell colSpan={colSpan} className="py-10 text-center text-slate-400 text-xs">
                  <div className="flex items-center justify-center gap-2">
                    <Loader2 className="h-4 w-4 animate-spin text-blue-600" />
                    <span>Loading...</span>
                  </div>
                </TableCell>
              </TableRow>
            )}
            {!isLoading && error && (
              <TableRow>
                <TableCell colSpan={colSpan} className="py-10 text-center text-xs">
                  <div role="alert" className="flex flex-col items-center gap-2 text-rose-600 dark:text-rose-400">
                    <span className="flex items-center gap-1.5 font-medium">
                      <AlertCircle className="h-4 w-4" />
                      {error}
                    </span>
                    {onRetry && (
                      <button type="button" onClick={onRetry} className="underline cursor-pointer">
                        Retry
                      </button>
                    )}
                  </div>
                </TableCell>
              </TableRow>
            )}
            {!isLoading && !error && items.length === 0 && (
              <TableRow>
                <TableCell colSpan={colSpan} className="py-10 text-center text-slate-400 text-xs">
                  {emptyMessage}
                </TableCell>
              </TableRow>
            )}
            {showRows &&
              items.map((row, rIdx) => (
                <React.Fragment key={rowKey(row)}>
                  <TableRow
                    role="row"
                    aria-selected={rIdx === active}
                    selected={rIdx === active}
                    onClick={() => {
                      setActive(rIdx);
                      onRowClick?.(row);
                    }}
                    className={cn(onRowClick && 'cursor-pointer')}
                  >
                    {columns.map((col) => (
                      <TableCell key={col.key} align={col.align ?? 'left'} mono={col.mono} className={col.className}>
                        {col.render(row, rIdx)}
                      </TableCell>
                    ))}
                  </TableRow>
                  {isExpanded?.(row) && renderExpanded && (
                    <TableRow>
                      <TableCell colSpan={colSpan} className="bg-slate-50/60 dark:bg-slate-800/30 p-3">
                        {renderExpanded(row)}
                      </TableCell>
                    </TableRow>
                  )}
                </React.Fragment>
              ))}
          </TableBody>
          {showRows && footer && (
            <TableFooter data-testid={footerTestId}>
              <TableRow className="hover:bg-transparent dark:hover:bg-transparent">
                {columns.map((col) => (
                  <TableCell
                    key={col.key}
                    align={col.align ?? 'left'}
                    mono={col.mono}
                    className={cn(
                      col.className,
                      'sticky bottom-0 z-10 bg-slate-50 dark:bg-slate-800 shadow-[inset_0_1px_0_#e2e8f0] dark:shadow-[inset_0_1px_0_#334155]'
                    )}
                  >
                    {footer[col.key]}
                  </TableCell>
                ))}
              </TableRow>
            </TableFooter>
          )}
        </Table>
      </div>
      {total !== undefined && onPageChange && (
        <Pager page={page} total={total} limit={limit} onPageChange={onPageChange} />
      )}
    </div>
  );
}

export default DataTable;
