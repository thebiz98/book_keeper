import { useCallback, useEffect, useRef, useState } from 'react';

/** Keys typed inside these are never grid commands. */
const IGNORED_TARGETS = ['INPUT', 'SELECT', 'TEXTAREA', 'BUTTON'];

export interface GridKeyboardNavOptions<T> {
  items: T[];
  getId: (item: T) => number;
  /** False while a drawer or dialog is open (or the list is loading): the hook then ignores every key. */
  enabled: boolean;
  /** Enter. */
  onOpen: (item: T) => void;
  /** Right (true) / Left (false). */
  onExpand?: (item: T, open: boolean) => void;
  /** PageUp (-1) / PageDown (+1). The page change refetches; the active row then moves to the new first row. */
  onPage?: (dir: -1 | 1) => void;
  /** Up on row 0: hand focus back to the search box. */
  onLeaveTop?: () => void;
  /** Letter shortcuts, e.g. `{ e: edit, s: setStock }` (lower-case key, no modifiers). */
  letters?: Record<string, (item: T) => void>;
}

export interface GridKeyboardNav {
  activeId: number | null;
  /** Row click and the search box's Down key use these. */
  setActiveId: (id: number | null) => void;
  /** Enter the grid on its first row (Down from the search box). */
  enterGrid: () => void;
}

/**
 * Inventory-style grid navigation (v1.2 D1). The active row is tracked by item id, so a refresh keeps it
 * on the same item; when that item is no longer listed (new page, filter) it moves to the first row.
 * Rows must carry `data-row-id` for scroll-into-view.
 */
export function useGridKeyboardNav<T>(opts: GridKeyboardNavOptions<T>): GridKeyboardNav {
  const [activeId, setActiveId] = useState<number | null>(null);
  const optsRef = useRef(opts);
  optsRef.current = opts;
  const activeRef = useRef<number | null>(null);
  activeRef.current = activeId;

  const { items, getId } = opts;
  useEffect(() => {
    if (activeId === null) return;
    if (!items.some((i) => getId(i) === activeId)) {
      setActiveId(items.length > 0 ? getId(items[0]) : null);
    }
  }, [items, getId, activeId]);

  useEffect(() => {
    if (activeId === null) return;
    const el = document.querySelector<HTMLElement>(`[data-row-id="${activeId}"]`);
    el?.scrollIntoView?.({ block: 'nearest' });
  }, [activeId]);

  const enterGrid = useCallback(() => {
    const { items: list, getId: id } = optsRef.current;
    if (list.length > 0) setActiveId(id(list[0]));
  }, []);

  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      const o = optsRef.current;
      if (!o.enabled || e.defaultPrevented) return;
      if (e.ctrlKey || e.altKey || e.metaKey) return;
      const t = e.target;
      if (t instanceof HTMLElement && (IGNORED_TARGETS.includes(t.tagName) || t.isContentEditable)) return;
      if (o.items.length === 0) return;

      const idx = o.items.findIndex((i) => o.getId(i) === activeRef.current);
      const active = idx >= 0 ? o.items[idx] : null;
      const last = o.items.length - 1;
      const go = (to: number) => {
        e.preventDefault();
        setActiveId(o.getId(o.items[Math.max(0, Math.min(last, to))]));
      };

      switch (e.key) {
        case 'ArrowDown':
          go(idx < 0 ? 0 : idx + 1);
          break;
        case 'ArrowUp':
          if (idx === 0) {
            e.preventDefault();
            setActiveId(null);
            o.onLeaveTop?.();
          } else if (idx > 0) {
            go(idx - 1);
          }
          break;
        case 'Home':
          go(0);
          break;
        case 'End':
          go(last);
          break;
        case 'Enter':
          if (active) {
            e.preventDefault();
            o.onOpen(active);
          }
          break;
        case 'ArrowRight':
          if (active && o.onExpand) {
            e.preventDefault();
            o.onExpand(active, true);
          }
          break;
        case 'ArrowLeft':
          if (active && o.onExpand) {
            e.preventDefault();
            o.onExpand(active, false);
          }
          break;
        case 'PageDown':
          if (o.onPage) {
            e.preventDefault();
            o.onPage(1);
          }
          break;
        case 'PageUp':
          if (o.onPage) {
            e.preventDefault();
            o.onPage(-1);
          }
          break;
        default: {
          const fn = !e.shiftKey && active ? o.letters?.[e.key.toLowerCase()] : undefined;
          if (fn && active) {
            e.preventDefault();
            fn(active);
          }
        }
      }
    };
    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, []);

  return { activeId, setActiveId, enterGrid };
}
