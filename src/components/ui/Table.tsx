import React from 'react';
import { cn } from '@/lib/utils';

export type TableScroll = 'bounded' | 'fill' | 'page' | 'none';

export interface TableProps extends React.TableHTMLAttributes<HTMLTableElement> {
  dense?: boolean;
  /** Scroll mode; see UI LLD 5.1 #5. Default 'bounded'. */
  scroll?: TableScroll;
  /** Extra classes on the scroll container (<div data-table-scroll>). */
  containerClassName?: string;
}

const TableCtx = React.createContext({ sticky: true });

const SCROLL_MODES: Record<TableScroll, string> = {
  bounded: 'overflow-auto max-h-[60vh]',
  fill: 'overflow-auto flex-1 min-h-0',
  page: 'overflow-visible',
  none: 'overflow-x-auto',
};

export const Table: React.FC<TableProps> = ({
  className,
  dense,
  scroll = 'bounded',
  containerClassName,
  ...props
}) => (
  <TableCtx.Provider value={{ sticky: scroll !== 'none' }}>
    <div
      data-table-scroll={scroll}
      className={cn('relative w-full', SCROLL_MODES[scroll], containerClassName)}
    >
      <table
        className={cn('w-full text-left text-sm border-collapse', dense && 'text-xs', className)}
        {...props}
      />
    </div>
  </TableCtx.Provider>
);

export const TableHeader: React.FC<React.HTMLAttributes<HTMLTableSectionElement>> = ({
  className,
  ...props
}) => <thead className={className} {...props} />;

export const TableBody: React.FC<React.HTMLAttributes<HTMLTableSectionElement>> = ({
  className,
  ...props
}) => (
  <tbody className={cn('divide-y divide-slate-100 dark:divide-slate-800', className)} {...props} />
);

export const TableFooter: React.FC<React.HTMLAttributes<HTMLTableSectionElement>> = ({
  className,
  ...props
}) => (
  <tfoot
    className={cn(
      'bg-slate-50 dark:bg-slate-800/40 border-t border-slate-200 dark:border-slate-700 font-medium',
      className
    )}
    {...props}
  />
);

export interface TableRowProps extends React.HTMLAttributes<HTMLTableRowElement> {
  selected?: boolean;
}

export const TableRow: React.FC<TableRowProps> = ({ className, selected, ...props }) => (
  <tr
    className={cn(
      'border-b border-slate-100 dark:border-slate-800/80 hover:bg-slate-50/70 dark:hover:bg-slate-800/40 transition-colors',
      selected && 'bg-blue-50/60 dark:bg-blue-950/40',
      className
    )}
    {...props}
  />
);

export interface TableHeadProps extends React.ThHTMLAttributes<HTMLTableCellElement> {
  align?: 'left' | 'center' | 'right';
}

export const TableHead: React.FC<TableHeadProps> = ({ className, align = 'left', ...props }) => {
  const { sticky } = React.useContext(TableCtx);
  const aligns = {
    left: 'text-left',
    center: 'text-center',
    right: 'text-right',
  };
  return (
    <th
      className={cn(
        'px-3 py-2 text-xs font-semibold text-slate-500 dark:text-slate-400 uppercase tracking-wider select-none',
        'bg-slate-50 dark:bg-slate-800',
        // Opaque sticky header; the bottom line is an inset shadow because collapsed borders do not travel with sticky cells.
        sticky
          ? 'sticky top-0 z-10 shadow-[inset_0_-1px_0_#e2e8f0] dark:shadow-[inset_0_-1px_0_#334155]'
          : 'border-b border-slate-200 dark:border-slate-700',
        aligns[align],
        className
      )}
      {...props}
    />
  );
};

export interface TableCellProps extends React.TdHTMLAttributes<HTMLTableCellElement> {
  align?: 'left' | 'center' | 'right';
  mono?: boolean;
}

export const TableCell: React.FC<TableCellProps> = ({
  className,
  align = 'left',
  mono = false,
  ...props
}) => {
  const aligns = {
    left: 'text-left',
    center: 'text-center',
    right: 'text-right',
  };
  return (
    <td
      className={cn(
        'px-3 py-1.5 text-slate-800 dark:text-slate-200',
        mono && 'tabular-nums font-mono',
        aligns[align],
        className
      )}
      {...props}
    />
  );
};

export default Table;
