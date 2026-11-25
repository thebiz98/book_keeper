import React, { useCallback, useEffect, useRef, useState } from 'react';
import { Search } from 'lucide-react';
import { errorMessage } from '@/api/errors';
import type { Paginated } from '@/types/api';
import { Modal } from '../ui/Modal';
import { Button } from '../ui/Button';
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '../ui/Table';
import { Pager } from './Pager';

export interface DeletedColumn<R> {
  header: string;
  render: (row: R) => React.ReactNode;
  align?: 'left' | 'right';
}

export interface DeletedRecordsDialogProps<R> {
  isOpen: boolean;
  title: string;
  columns: DeletedColumn<R>[];
  /** Backend page: name search and paging happen in SQL. */
  fetchPage: (search: string, page: number, pageSize: number) => Promise<Paginated<R>>;
  rowId: (row: R) => number;
  restore: (row: R) => Promise<void>;
  restoreLabel?: string;
  /** Called after a successful restore so the page behind refreshes. */
  onRestored: () => void;
  /** Optional extra per-row action, e.g. "Delete permanently" (E2 A1). */
  extraAction?: {
    label: string;
    /** The row's backend flag decides whether the button is offered. */
    visible: (row: R) => boolean;
    run: (row: R) => Promise<void>;
  };
  onClose: () => void;
  pageSize?: number;
}

const DEBOUNCE_MS = 250;

/**
 * Shared "Deleted items / Deleted parties" list: debounced search, backend paging, and a Restore
 * button per row. Columns, fetch and restore are props, so E3 reuses it for parties.
 */
export function DeletedRecordsDialog<R>({
  isOpen,
  title,
  columns,
  fetchPage,
  rowId,
  restore,
  restoreLabel = 'Restore',
  onRestored,
  extraAction,
  onClose,
  pageSize = 10,
}: DeletedRecordsDialogProps<R>): React.ReactElement | null {
  const [query, setQuery] = useState('');
  const [committed, setCommitted] = useState('');
  const [page, setPage] = useState(1);
  const [data, setData] = useState<Paginated<R> | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busyId, setBusyId] = useState<number | null>(null);
  const fetchRef = useRef(fetchPage);
  fetchRef.current = fetchPage;
  const seq = useRef(0);

  const load = useCallback(
    (search: string, p: number) => {
      const my = ++seq.current;
      fetchRef
        .current(search, p, pageSize)
        .then((res) => {
          if (my === seq.current) setData(res);
        })
        .catch((e) => {
          if (my === seq.current) setError(errorMessage(e));
        });
    },
    [pageSize]
  );

  useEffect(() => {
    if (!isOpen) return;
    setQuery('');
    setCommitted('');
    setPage(1);
    setData(null);
    setError(null);
  }, [isOpen]);

  useEffect(() => {
    if (!isOpen) return;
    const t = setTimeout(() => {
      const q = query.trim();
      if (q !== committed) {
        setCommitted(q);
        setPage(1);
      }
    }, DEBOUNCE_MS);
    return () => clearTimeout(t);
  }, [query, committed, isOpen]);

  useEffect(() => {
    if (isOpen) load(committed, page);
  }, [isOpen, committed, page, load]);

  if (!isOpen) return null;

  const act = async (row: R, fn: (r: R) => Promise<void>, afterRestore: boolean) => {
    setBusyId(rowId(row));
    setError(null);
    try {
      await fn(row);
      if (afterRestore) onRestored();
      load(committed, page);
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusyId(null);
    }
  };

  return (
    <Modal isOpen onClose={onClose} title={title} size="full" bodyClassName="flex flex-col gap-3">
      <div data-testid="deleted-records" className="flex flex-col gap-3 min-h-0">
        <div className="relative max-w-sm">
          <Search className="absolute left-2.5 top-1/2 -translate-y-1/2 h-4 w-4 text-slate-400" />
          <input
            type="text"
            aria-label="Search deleted"
            data-testid="deleted-records-search"
            placeholder="Search by name..."
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            className="w-full text-xs pl-8 pr-3 py-1.5 rounded-md border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-900 focus:outline-none focus:ring-2 focus:ring-blue-500"
          />
        </div>
        {error && (
          <p role="alert" className="text-xs font-medium text-rose-600">
            {error}
          </p>
        )}
        <Table dense scroll="bounded">
          <TableHeader>
            <TableRow>
              {columns.map((c) => (
                <TableHead key={c.header} align={c.align}>
                  {c.header}
                </TableHead>
              ))}
              <TableHead className="text-center w-48">Action</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {data === null ? (
              <TableRow>
                <TableCell colSpan={columns.length + 1} className="py-8 text-center text-xs text-slate-400">
                  Loading...
                </TableCell>
              </TableRow>
            ) : data.items.length === 0 ? (
              <TableRow>
                <TableCell colSpan={columns.length + 1} className="py-8 text-center text-xs text-slate-400">
                  Nothing deleted{committed ? ' matches this search' : ''}.
                </TableCell>
              </TableRow>
            ) : (
              data.items.map((row) => {
                const id = rowId(row);
                return (
                  <TableRow key={id} data-testid="deleted-row">
                    {columns.map((c) => (
                      <TableCell key={c.header} align={c.align}>
                        {c.render(row)}
                      </TableCell>
                    ))}
                    <TableCell className="text-center">
                      <div className="flex justify-center gap-1.5">
                        <Button
                          size="xs"
                          variant="outline"
                          data-testid={`deleted-restore-${id}`}
                          disabled={busyId !== null}
                          onClick={() => void act(row, restore, true)}
                        >
                          {restoreLabel}
                        </Button>
                        {extraAction?.visible(row) && (
                          <Button
                            size="xs"
                            variant="danger"
                            data-testid={`deleted-extra-${id}`}
                            disabled={busyId !== null}
                            onClick={() => void act(row, extraAction.run, false)}
                          >
                            {extraAction.label}
                          </Button>
                        )}
                      </div>
                    </TableCell>
                  </TableRow>
                );
              })
            )}
          </TableBody>
        </Table>
        {data && <Pager page={page} total={data.total} limit={pageSize} onPageChange={setPage} />}
      </div>
    </Modal>
  );
}

export default DeletedRecordsDialog;
