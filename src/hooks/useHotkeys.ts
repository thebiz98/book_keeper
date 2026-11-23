/**
 * Central hotkey registry (DD-19, UI LLD §4).
 *
 * One window `keydown` listener dispatches to every registered hotkey. Rules:
 *  - Scopes: `modal` handlers run only while a modal layer is open; `page` and
 *    `global` handlers are blocked while a modal is open or a submit is in flight.
 *  - Esc is layered: the top-most open modal / dropdown layer handles it first;
 *    only when no layer is open do plain `Escape` hotkeys run.
 *  - `e.repeat` is ignored unless a hotkey sets `allowRepeat`.
 *  - Function keys and Ctrl/Alt combos fire inside inputs; plain keys do not
 *    unless `allowInInputs` is set.
 *  - F5 (reload) and Ctrl+F (browser find) are always preventDefault'ed.
 */
import { useEffect, useRef } from 'react';

export type HotkeyScope = 'global' | 'page' | 'modal';

export interface HotkeyDef {
  /** Canonical combos; the first one is the badge shown in the UI. */
  combos: string[];
  label: string;
}

/** DD-19 hotkey map. Sidebar and HotkeyBar badges render from this. */
export const HOTKEYS = {
  dashboard: { combos: ['F1'], label: 'Dashboard' },
  sale: { combos: ['F2'], label: 'Billing' },
  purchase: { combos: ['F3'], label: 'Purchases' },
  lookup: { combos: ['F4', 'Ctrl+F'], label: 'Item Lookup' },
  dues: { combos: ['F5'], label: 'Khata' },
  inventory: { combos: ['F6'], label: 'Inventory' },
  expenses: { combos: ['F7'], label: 'Expenses' },
  newBundle: { combos: ['Alt+B'], label: 'New Bundle' },
  newItem: { combos: ['Alt+N'], label: 'New Item' },
  addCharge: { combos: ['Alt+X'], label: 'Charge / Discount' },
  newCustomer: { combos: ['Alt+C'], label: 'New Customer' },
  newVendor: { combos: ['Alt+V'], label: 'New Vendor' },
  backup: { combos: ['Ctrl+B'], label: 'Backup' },
  completeBill: { combos: ['Ctrl+Enter'], label: 'Complete Bill' },
  back: { combos: ['Escape'], label: 'Close / Back' },
} as const satisfies Record<string, HotkeyDef>;

export type HotkeyAction = keyof typeof HOTKEYS;

export function hotkeyCombos(action: HotkeyAction): string[] {
  return [...HOTKEYS[action].combos];
}

/** Display text of the primary combo, e.g. `Esc` for `Escape`. */
export function hotkeyBadge(action: HotkeyAction): string {
  return displayCombo(HOTKEYS[action].combos[0]);
}

export function hotkeyLabel(action: HotkeyAction): string {
  return HOTKEYS[action].label;
}

export function displayCombo(combo: string): string {
  return combo.replace('Escape', 'Esc').replace('Delete', 'Del');
}

const KEY_ALIASES: Record<string, string> = {
  esc: 'Escape',
  escape: 'Escape',
  del: 'Delete',
  delete: 'Delete',
  enter: 'Enter',
  return: 'Enter',
  space: ' ',
  up: 'ArrowUp',
  down: 'ArrowDown',
};

/** Normalise a combo string ("ctrl+enter") to canonical form ("Ctrl+Enter"). */
export function normalizeCombo(combo: string): string {
  const parts = combo.split('+').map((p) => p.trim()).filter(Boolean);
  let ctrl = false;
  let alt = false;
  let shift = false;
  let key = '';
  for (const p of parts) {
    const l = p.toLowerCase();
    if (l === 'ctrl' || l === 'cmd' || l === 'meta' || l === 'control') ctrl = true;
    else if (l === 'alt' || l === 'option') alt = true;
    else if (l === 'shift') shift = true;
    else key = KEY_ALIASES[l] ?? (p.length === 1 ? p.toUpperCase() : /^f\d+$/i.test(p) ? p.toUpperCase() : p);
  }
  return [ctrl && 'Ctrl', alt && 'Alt', shift && 'Shift', key].filter(Boolean).join('+');
}

/** Canonical combo for a keyboard event. Letters use `code` so Alt+B works on every layout. */
export function eventCombo(e: KeyboardEvent): string {
  let key = e.key;
  if (e.code && /^Key[A-Z]$/.test(e.code)) key = e.code.slice(3);
  else if (key.length === 1) key = key.toUpperCase();
  return [(e.ctrlKey || e.metaKey) && 'Ctrl', e.altKey && 'Alt', e.shiftKey && 'Shift', key]
    .filter(Boolean)
    .join('+');
}

export interface HotkeyOptions {
  scope?: HotkeyScope;
  enabled?: boolean;
  /** Fire while an input / textarea / select is focused. Default: true for F-keys and Ctrl/Alt combos. */
  allowInInputs?: boolean;
  allowRepeat?: boolean;
  /** Let a page/global hotkey run while a modal is open or a submit is busy. */
  allowInModal?: boolean;
}

interface Latest {
  handler: (e: KeyboardEvent) => void;
  opts: HotkeyOptions;
}

interface Entry {
  id: number;
  combos: string[];
  latest: { current: Latest };
}

interface Layer {
  id: number;
  handler: { current: () => void };
}

const entries: Entry[] = [];
const escLayers: Layer[] = [];
let modalCount = 0;
let busyCount = 0;
let seq = 0;
let attached = false;

const SCOPE_RANK: Record<HotkeyScope, number> = { modal: 3, page: 2, global: 1 };
const ALWAYS_PREVENT = new Set(['F5', 'Ctrl+F']);

export function isModalOpen(): boolean {
  return modalCount > 0;
}
export function isBusy(): boolean {
  return busyCount > 0;
}

function isEditable(t: EventTarget | null): boolean {
  const el = t as HTMLElement | null;
  if (!el || !el.tagName) return false;
  return (
    el.tagName === 'INPUT' ||
    el.tagName === 'TEXTAREA' ||
    el.tagName === 'SELECT' ||
    el.isContentEditable === true
  );
}

function defaultAllowInInputs(combo: string): boolean {
  return /^F\d+$/.test(combo) || combo.startsWith('Ctrl+') || combo.startsWith('Alt+');
}

function onKeyDown(e: KeyboardEvent): void {
  const combo = eventCombo(e);
  if (ALWAYS_PREVENT.has(combo)) e.preventDefault();

  if (combo === 'Escape') {
    if (e.repeat) return;
    const top = escLayers[escLayers.length - 1];
    if (top) {
      e.preventDefault();
      top.handler.current();
      return;
    }
  }

  const matching = entries
    .filter((en) => en.combos.includes(combo))
    .sort(
      (a, b) =>
        SCOPE_RANK[b.latest.current.opts.scope ?? 'page'] -
          SCOPE_RANK[a.latest.current.opts.scope ?? 'page'] || b.id - a.id
    );
  if (matching.length === 0) return;

  const modalOpen = modalCount > 0;
  const busy = busyCount > 0;
  const inInput = isEditable(e.target);

  for (const en of matching) {
    const { opts, handler } = en.latest.current;
    if (opts.enabled === false) continue;
    const scope = opts.scope ?? 'page';
    if (scope === 'modal') {
      if (!modalOpen) continue;
    } else if ((modalOpen || busy) && !opts.allowInModal) {
      continue;
    }
    if (inInput && !(opts.allowInInputs ?? defaultAllowInInputs(combo))) continue;
    e.preventDefault();
    if (e.repeat && !opts.allowRepeat) return;
    handler(e);
    return;
  }
}

function ensureListener(): void {
  if (attached || typeof window === 'undefined') return;
  window.addEventListener('keydown', onKeyDown);
  attached = true;
}

/** Register a hotkey for the lifetime of the component. Handler/opts changes never re-register. */
export function useHotkey(
  combo: string | string[],
  handler: (e: KeyboardEvent) => void,
  opts: HotkeyOptions = {}
): void {
  const latest = useRef<Latest>({ handler, opts });
  latest.current = { handler, opts };
  const key = (Array.isArray(combo) ? combo : [combo]).map(normalizeCombo).join('|');

  useEffect(() => {
    ensureListener();
    const entry: Entry = { id: ++seq, combos: key.split('|'), latest };
    entries.push(entry);
    return () => {
      const i = entries.indexOf(entry);
      if (i >= 0) entries.splice(i, 1);
    };
  }, [key]);
}

/** Register the hotkey(s) of a DD-19 action. */
export function useHotkeyAction(
  action: HotkeyAction,
  handler: (e: KeyboardEvent) => void,
  opts: HotkeyOptions = {}
): void {
  useHotkey(hotkeyCombos(action), handler, opts);
}

function useLayer(onEscape: () => void, active: boolean, modal: boolean): void {
  const handler = useRef(onEscape);
  handler.current = onEscape;
  useEffect(() => {
    if (!active) return;
    ensureListener();
    const layer: Layer = { id: ++seq, handler };
    escLayers.push(layer);
    if (modal) modalCount++;
    return () => {
      const i = escLayers.indexOf(layer);
      if (i >= 0) escLayers.splice(i, 1);
      if (modal) modalCount = Math.max(0, modalCount - 1);
    };
  }, [active, modal]);
}

/** A modal / drawer layer: blocks page + global hotkeys and owns Esc while top-most. */
export function useModalLayer(onEscape: () => void, active: boolean): void {
  useLayer(onEscape, active, true);
}

/** A dropdown / popover layer: owns Esc while it is the top-most open layer. */
export function useEscapeLayer(onEscape: () => void, active: boolean): void {
  useLayer(onEscape, active, false);
}

/** Block page/global hotkeys (F-key navigation, Ctrl+Enter) while a submit is in flight. */
export function useBusyLock(busy: boolean): void {
  useEffect(() => {
    if (!busy) return;
    busyCount++;
    return () => {
      busyCount = Math.max(0, busyCount - 1);
    };
  }, [busy]);
}

/** Test helper: drop every registration. */
export function __resetHotkeys(): void {
  entries.length = 0;
  escLayers.length = 0;
  modalCount = 0;
  busyCount = 0;
}
