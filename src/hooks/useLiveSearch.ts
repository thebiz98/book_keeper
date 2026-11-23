import { useCallback, useEffect, useRef, useState } from 'react';

/** Typeahead dropdowns (items, parties, expense/charge types). */
export const TYPEAHEAD_DEBOUNCE_MS = 50;
/** Search boxes above lists and tables. */
export const LIST_SEARCH_DEBOUNCE_MS = 150;
/** A spinner only appears when a search takes longer than this, so quick searches never flash. */
export const SPINNER_DELAY_MS = 150;

export interface UseLiveSearchOptions<T> {
  /** Latest closure is always used (held in a ref). Receives the trimmed query. */
  fetcher: (query: string) => Promise<T>;
  /** Results shown before any search and after the query is cleared (`emptyBehavior: 'clear'`). */
  initial: T;
  debounceMs?: number; // default LIST_SEARCH_DEBOUNCE_MS
  minChars?: number; // default 1
  /** Query shorter than minChars: 'clear' => results = initial, no fetch; 'fetch' => fetcher(''). */
  emptyBehavior?: 'clear' | 'fetch'; // default 'clear'
  /** Non-query triggers (filters, page): refetch immediately with the committed query. */
  deps?: ReadonlyArray<unknown>;
  /** Called in the same tick the debounced query commits (batch setPage(1) here). */
  onQueryCommit?: (query: string) => void;
  enabled?: boolean; // default true
}

export interface UseLiveSearchResult<T> {
  query: string;
  setQuery: (q: string) => void;
  /** Trimmed query of the current/next fetch. */
  committedQuery: string;
  results: T;
  /** Trimmed query the results belong to; null = none/cleared. */
  resultsFor: string | null;
  isLoading: boolean;
  /** isLoading for longer than SPINNER_DELAY_MS. */
  showSpinner: boolean;
  error: unknown | null;
  /** Cancel the debounce and run now (dedupes the same in-flight query). */
  flush: () => Promise<T>;
  /** Refetch the committed query with the current deps. */
  refresh: () => Promise<T>;
  /** Clear query/results and invalidate anything in flight. */
  reset: () => void;
}

function sameDeps(a: ReadonlyArray<unknown>, b: ReadonlyArray<unknown>): boolean {
  return a.length === b.length && a.every((v, i) => Object.is(v, b[i]));
}

/**
 * One implementation of "type, wait, fetch, show": debounce, stale-response guard (only the
 * latest request may apply), minimum characters, filters/paging via `deps`, and a delayed spinner.
 * Pages never hand-roll `setTimeout` debounces.
 */
export function useLiveSearch<T>(opts: UseLiveSearchOptions<T>): UseLiveSearchResult<T> {
  const { initial, deps = [], enabled = true } = opts;

  const optsRef = useRef(opts);
  optsRef.current = opts;
  const initialRef = useRef(initial);

  const [query, setQueryState] = useState('');
  const [committedQuery, setCommittedQuery] = useState('');
  const [results, setResults] = useState<T>(initial);
  const [resultsFor, setResultsFor] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [showSpinner, setShowSpinner] = useState(false);
  const [error, setError] = useState<unknown | null>(null);

  const mounted = useRef(true);
  const requestId = useRef(0);
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const committedRef = useRef('');
  const resultsRef = useRef<T>(initial);
  const inflight = useRef<{ id: number; query: string; deps: ReadonlyArray<unknown>; promise: Promise<T> } | null>(null);
  const waiters = useRef<Array<(r: T) => void>>([]);

  const settleWaiters = (r: T) => {
    const w = waiters.current;
    waiters.current = [];
    w.forEach((fn) => fn(r));
  };

  const run = useCallback(
    (q: string, force = false): Promise<T> => {
      const o = optsRef.current;
      const min = o.minChars ?? 1;
      const short = q.length < min;
      if (short && (o.emptyBehavior ?? 'clear') === 'clear') {
        requestId.current++;
        inflight.current = null;
        resultsRef.current = initialRef.current;
        setResults(initialRef.current);
        setResultsFor(null);
        setIsLoading(false);
        setError(null);
        settleWaiters(initialRef.current);
        return Promise.resolve(initialRef.current);
      }
      const effective = short ? '' : q;
      const currentDeps = o.deps ?? [];
      const cur = inflight.current;
      // Reuse only a request that is still current: a superseded one (e.g. invalidated by an unmount,
      // which React StrictMode does on every mount in dev) never applies its result, so waiting on it
      // would leave isLoading stuck.
      if (!force && cur && cur.id === requestId.current && cur.query === effective && sameDeps(cur.deps, currentDeps)) {
        return cur.promise;
      }

      const id = ++requestId.current;
      setIsLoading(true);
      const promise = o.fetcher(effective).then(
        (r) => {
          if (id === requestId.current && mounted.current) {
            resultsRef.current = r;
            setResults(r);
            setResultsFor(effective);
            setError(null);
            setIsLoading(false);
            inflight.current = null;
            settleWaiters(r);
          }
          return r;
        },
        (e: unknown) => {
          if (id === requestId.current && mounted.current) {
            setError(e);
            setIsLoading(false);
            inflight.current = null;
            settleWaiters(resultsRef.current);
          }
          return resultsRef.current;
        }
      );
      inflight.current = { id, query: effective, deps: currentDeps, promise };
      return promise;
    },
    []
  );

  // Fetch whenever the committed query, enabled flag or any dep changes (one effect = one fetch,
  // even when onQueryCommit changed a dep such as the page in the same tick).
  useEffect(() => {
    if (!enabled) return;
    void run(committedQuery);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [committedQuery, enabled, run, ...deps]);

  const commit = useCallback((trimmed: string) => {
    committedRef.current = trimmed;
    setCommittedQuery(trimmed);
    optsRef.current.onQueryCommit?.(trimmed);
  }, []);

  const clearTimer = () => {
    if (timer.current) {
      clearTimeout(timer.current);
      timer.current = null;
    }
  };

  const setQuery = useCallback(
    (q: string) => {
      setQueryState(q);
      const trimmed = q.trim();
      clearTimer();
      if (trimmed === committedRef.current) return;
      timer.current = setTimeout(() => {
        timer.current = null;
        commit(trimmed);
      }, optsRef.current.debounceMs ?? LIST_SEARCH_DEBOUNCE_MS);
    },
    [commit]
  );

  const queryRef = useRef('');
  queryRef.current = query;

  const flush = useCallback((): Promise<T> => {
    clearTimer();
    const trimmed = queryRef.current.trim();
    if (trimmed !== committedRef.current) {
      commit(trimmed);
      if (optsRef.current.onQueryCommit) {
        // Deps (e.g. page) may have changed in onQueryCommit: let the effect run the fetch with them.
        return new Promise<T>((resolve) => {
          waiters.current.push(resolve);
        });
      }
    }
    return run(trimmed);
  }, [commit, run]);

  const refresh = useCallback((): Promise<T> => run(committedRef.current, true), [run]);

  const reset = useCallback(() => {
    clearTimer();
    requestId.current++;
    inflight.current = null;
    committedRef.current = '';
    setQueryState('');
    setCommittedQuery('');
    resultsRef.current = initialRef.current;
    setResults(initialRef.current);
    setResultsFor(null);
    setIsLoading(false);
    setError(null);
    settleWaiters(initialRef.current);
  }, []);

  useEffect(() => {
    if (!isLoading) {
      setShowSpinner(false);
      return;
    }
    const t = setTimeout(() => setShowSpinner(true), SPINNER_DELAY_MS);
    return () => clearTimeout(t);
  }, [isLoading]);

  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
      requestId.current++;
      inflight.current = null;
      if (timer.current) clearTimeout(timer.current);
    };
  }, []);

  return {
    query,
    setQuery,
    committedQuery,
    results,
    resultsFor,
    isLoading,
    showSpinner,
    error,
    flush,
    refresh,
    reset,
  };
}
