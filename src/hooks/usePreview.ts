import { useEffect, useMemo, useRef, useState } from 'react';
import { deletedReference, errorMessage, isAppError } from '@/api/errors';
import type { PreviewError } from '@/types';

/** Previews follow typing closely: short enough to feel live, long enough to coalesce keystrokes. */
export const PREVIEW_DEBOUNCE_MS = 80;

export interface UsePreviewOptions<TIn, TOut> {
  /** What to preview; `null` = nothing to preview yet (no fetch, no data). Compared by value. */
  input: TIn | null;
  /** Latest closure is always used (held in a ref). */
  fetcher: (input: TIn) => Promise<TOut>;
  debounceMs?: number; // default PREVIEW_DEBOUNCE_MS
  enabled?: boolean; // default true
  /** Changing it re-runs the preview for the same input (e.g. after a Restore, DD-90). */
  refreshKey?: number;
}

export interface UsePreviewResult<TOut> {
  /** The latest applied result for the current or an earlier input; `null` before the first one. */
  data: TOut | null;
  /** `data.errors` of a bill preview, or the error the fetch rejected with (as one entry). */
  errors: PreviewError[];
  /** True from an input change until its result applies (the figures shown may be stale meanwhile). */
  isLoading: boolean;
}

function toErrors(e: unknown): PreviewError[] {
  const field = isAppError(e) && e.type === 'ValidationError' ? e.details.field : isAppError(e) && e.type === 'BillLocked' ? e.details.code : '';
  const ref = deletedReference(e);
  return [ref ? { field, message: errorMessage(e), deleted: { entity: ref.entity, id: ref.id, name: ref.name } } : { field, message: errorMessage(e) }];
}

function dataErrors(d: unknown): PreviewError[] {
  const errs = (d as { errors?: unknown } | null)?.errors;
  return Array.isArray(errs) ? (errs as PreviewError[]) : [];
}

/**
 * DD-56: the one way a screen shows a figure before Save. Debounces the input, asks the backend
 * for a dry run, and lets only the latest request apply its result (same guard as `useLiveSearch`).
 * Survives a StrictMode remount: every effect run starts its own request and the unmount cleanup
 * only cancels that run, so a remount never waits on an invalidated request.
 */
export function usePreview<TIn, TOut>(opts: UsePreviewOptions<TIn, TOut>): UsePreviewResult<TOut> {
  const { input, enabled = true, debounceMs = PREVIEW_DEBOUNCE_MS, refreshKey = 0 } = opts;
  const optsRef = useRef(opts);
  optsRef.current = opts;

  const active = enabled && input !== null;
  const key = useMemo(() => (active ? `${refreshKey}#${JSON.stringify(input)}` : null), [active, input, refreshKey]);

  const [settled, setSettled] = useState<{ key: string | null; data: TOut | null; errors: PreviewError[] }>({
    key: null,
    data: null,
    errors: [],
  });

  useEffect(() => {
    if (key === null) {
      setSettled((s) => (s.key === null && s.data === null ? s : { key: null, data: null, errors: [] }));
      return;
    }
    let live = true;
    const timer = setTimeout(() => {
      const current = optsRef.current.input;
      if (current === null) return;
      optsRef.current.fetcher(current).then(
        (data) => {
          if (live) setSettled({ key, data, errors: dataErrors(data) });
        },
        (e: unknown) => {
          if (live) setSettled({ key, data: null, errors: toErrors(e) });
        }
      );
    }, debounceMs);
    return () => {
      live = false;
      clearTimeout(timer);
    };
  }, [key, debounceMs]);

  return {
    data: key === null ? null : settled.data,
    errors: key === null ? [] : settled.errors,
    isLoading: key !== null && settled.key !== key,
  };
}
