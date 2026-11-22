import React, { useState, useEffect, useRef, useId, useMemo } from 'react';
import { User, Truck, Search, Check, ChevronDown, UserPlus } from 'lucide-react';
import { cn } from '@/lib/utils';
import { api, errorMessage } from '@/api';
import type { Party, PartyListRow, PartyType } from '@/types';
import { useEscapeLayer, useHotkey, type HotkeyScope } from '@/hooks/useHotkeys';
import { CurrencyDisplay } from '../common/CurrencyDisplay';

export interface PartySelectorProps {
  partyType: PartyType;
  selectedPartyId?: number | null;
  onSelectParty: (party: Party | null) => void;
  allowWalkIn?: boolean;
  className?: string;
  /** Hotkey scope for inline create (Alt+C customers / Alt+V vendors). Use `modal` inside dialogs. */
  hotkeyScope?: Extract<HotkeyScope, 'page' | 'modal'>;
  /** Enable inline "create new" and its hotkey (default true). */
  enableCreate?: boolean;
  /** Focus the trigger on mount. */
  autoFocus?: boolean;
  /** Called after an inline create with the new party. */
  onCreated?: (party: Party) => void;
}

type Mode = 'closed' | 'list' | 'create';

export const PartySelector: React.FC<PartySelectorProps> = ({
  partyType,
  selectedPartyId = null,
  onSelectParty,
  allowWalkIn = true,
  className,
  hotkeyScope = 'page',
  enableCreate = true,
  autoFocus = false,
  onCreated,
}) => {
  const [parties, setParties] = useState<PartyListRow[]>([]);
  const [search, setSearch] = useState('');
  const [mode, setMode] = useState<Mode>('closed');
  const [highlighted, setHighlighted] = useState(0);
  const [newName, setNewName] = useState('');
  const [newPhone, setNewPhone] = useState('');
  const [createError, setCreateError] = useState<string | null>(null);
  const [isCreating, setIsCreating] = useState(false);

  const listId = useId();
  const containerRef = useRef<HTMLDivElement>(null);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const searchRef = useRef<HTMLInputElement>(null);
  const nameRef = useRef<HTMLInputElement>(null);
  const creatingRef = useRef(false);
  const isOpen = mode !== 'closed';

  const refresh = (): Promise<PartyListRow[]> =>
    api
      .listParties(partyType, true)
      .then((data) => {
        setParties(data);
        return data;
      })
      .catch(() => [] as PartyListRow[]);

  useEffect(() => {
    void refresh();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [partyType]);

  useEffect(() => {
    if (autoFocus) triggerRef.current?.focus();
  }, [autoFocus]);

  useEffect(() => {
    const onDown = (e: MouseEvent) => {
      if (containerRef.current && !containerRef.current.contains(e.target as Node)) {
        setMode('closed');
      }
    };
    document.addEventListener('mousedown', onDown);
    return () => document.removeEventListener('mousedown', onDown);
  }, []);

  const close = (refocus = true) => {
    setMode('closed');
    setSearch('');
    setCreateError(null);
    if (refocus) triggerRef.current?.focus();
  };

  // Esc closes the dropdown / create form first (top-most layer).
  useEscapeLayer(() => close(), isOpen);

  const openList = () => {
    setMode('list');
    setHighlighted(0);
    void refresh();
    setTimeout(() => searchRef.current?.focus(), 0);
  };

  const openCreate = () => {
    if (!enableCreate) return;
    setMode('create');
    setNewName(search.trim());
    setNewPhone('');
    setCreateError(null);
    setTimeout(() => nameRef.current?.focus(), 0);
  };

  useHotkey(partyType === 'CUSTOMER' ? 'Alt+C' : 'Alt+V', openCreate, {
    scope: hotkeyScope,
    enabled: enableCreate,
  });

  const selectedParty = parties.find((p) => p.id === selectedPartyId);

  const filtered = useMemo(() => {
    const q = search.trim().toLowerCase();
    if (!q) return parties;
    return parties.filter(
      (p) => (p.name || '').toLowerCase().includes(q) || (p.phone || '').toLowerCase().includes(q)
    );
  }, [parties, search]);

  type Row =
    | { kind: 'walkin' }
    | { kind: 'party'; party: PartyListRow }
    | { kind: 'create' };
  const rows: Row[] = [
    ...(allowWalkIn ? [{ kind: 'walkin' } as Row] : []),
    ...filtered.map((party) => ({ kind: 'party', party }) as Row),
    ...(enableCreate ? [{ kind: 'create' } as Row] : []),
  ];

  useEffect(() => {
    setHighlighted((h) => Math.min(h, Math.max(rows.length - 1, 0)));
  }, [rows.length]);

  const choose = (row: Row | undefined) => {
    if (!row) return;
    if (row.kind === 'walkin') {
      onSelectParty(null);
      close();
    } else if (row.kind === 'party') {
      onSelectParty(row.party);
      close();
    } else {
      openCreate();
    }
  };

  const onListKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'ArrowDown') {
      e.preventDefault();
      setHighlighted((h) => (h + 1) % rows.length);
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      setHighlighted((h) => (h <= 0 ? rows.length - 1 : h - 1));
    } else if (e.key === 'Enter') {
      e.preventDefault();
      e.stopPropagation();
      choose(rows[highlighted]);
    }
  };

  const onTriggerKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
      e.preventDefault();
      if (!isOpen) openList();
    }
  };

  const submitCreate = async () => {
    const name = newName.trim();
    if (!name) {
      setCreateError('Name is required');
      return;
    }
    if (creatingRef.current) return;
    creatingRef.current = true;
    setIsCreating(true);
    setCreateError(null);
    try {
      const id = await api.createParty({
        name,
        phone: newPhone.trim() || null,
        partyType,
        notes: null,
      });
      const list = await refresh();
      const created =
        list.find((p) => p.id === id) ??
        ({
          id,
          name,
          phone: newPhone.trim() || null,
          partyType,
          notes: null,
          isActive: true,
          createdAt: '',
          updatedAt: '',
          outstandingBalance: 0,
          transactionCount: 0,
        } as PartyListRow);
      onSelectParty(created);
      onCreated?.(created);
      close();
    } catch (err) {
      setCreateError(errorMessage(err));
    } finally {
      creatingRef.current = false;
      setIsCreating(false);
    }
  };

  const onCreateKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter') {
      e.preventDefault();
      e.stopPropagation();
      void submitCreate();
    }
  };

  const Icon = partyType === 'CUSTOMER' ? User : Truck;
  const walkInLabel = partyType === 'CUSTOMER' ? 'Walk-in Customer' : 'Standard Vendor';
  const noun = partyType === 'CUSTOMER' ? 'customer' : 'vendor';
  const createHotkey = partyType === 'CUSTOMER' ? 'Alt+C' : 'Alt+V';

  const duePill = (amount: number) =>
    amount > 0 ? (
      <span
        data-testid="outstanding-pill"
        className="px-1.5 py-0.5 rounded text-[10px] font-mono font-semibold bg-rose-50 text-rose-700 dark:bg-rose-950 dark:text-rose-300 border border-rose-200 dark:border-rose-800"
      >
        Due: <CurrencyDisplay amount={amount} size="sm" />
      </span>
    ) : null;

  return (
    <div ref={containerRef} className={cn('relative w-full', className)}>
      <button
        ref={triggerRef}
        type="button"
        role="combobox"
        aria-haspopup="listbox"
        aria-expanded={isOpen}
        aria-controls={mode === 'list' ? listId : undefined}
        aria-label={`Select ${noun}`}
        onClick={() => (isOpen ? close(false) : openList())}
        onKeyDown={onTriggerKeyDown}
        className="flex items-center justify-between w-full rounded-md border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-900 text-slate-900 dark:text-slate-100 text-sm px-3 py-1.5 cursor-pointer select-none text-left focus:outline-none focus:ring-2 focus:ring-blue-500"
      >
        <div className="flex items-center gap-2 overflow-hidden truncate">
          <Icon className="h-4 w-4 text-slate-400 shrink-0" />
          {selectedParty ? (
            <div className="flex items-center gap-2 truncate">
              <span className="font-medium truncate">{selectedParty.name}</span>
              {selectedParty.phone && (
                <span className="text-xs text-slate-400 font-mono">({selectedParty.phone})</span>
              )}
              {duePill(selectedParty.outstandingBalance)}
            </div>
          ) : (
            <span className="text-slate-500 dark:text-slate-400 font-medium">{walkInLabel}</span>
          )}
        </div>
        <ChevronDown className="h-4 w-4 text-slate-400 shrink-0" />
      </button>

      {mode === 'list' && (
        <div className="absolute z-50 mt-1 max-h-60 w-full overflow-auto rounded-md bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 shadow-xl py-1">
          <div className="p-2 border-b border-slate-100 dark:border-slate-800 sticky top-0 bg-white dark:bg-slate-900">
            <div className="relative flex items-center">
              <Search className="h-3.5 w-3.5 absolute left-2 text-slate-400" />
              <input
                ref={searchRef}
                type="text"
                role="combobox"
                aria-expanded
                aria-controls={listId}
                aria-activedescendant={`${listId}-opt-${highlighted}`}
                aria-label={`Search ${noun}s`}
                value={search}
                onChange={(e) => {
                  setSearch(e.target.value);
                  setHighlighted(0);
                }}
                onKeyDown={onListKeyDown}
                placeholder={`Search ${noun} by name or phone...`}
                className="w-full text-xs pl-7 pr-2 py-1 rounded border border-slate-200 dark:border-slate-700 bg-slate-50 dark:bg-slate-800 text-slate-900 dark:text-slate-100 focus:outline-none focus:ring-1 focus:ring-blue-500"
              />
            </div>
          </div>

          <div id={listId} role="listbox" aria-label={`${noun}s`}>
            {rows.map((row, idx) => {
              const isHi = idx === highlighted;
              const base = cn(
                'flex items-center justify-between px-3 py-2 text-xs cursor-pointer select-none transition-colors',
                isHi ? 'bg-blue-50 dark:bg-blue-950/40 text-blue-900 dark:text-blue-100' : 'hover:bg-slate-50 dark:hover:bg-slate-800/40'
              );
              const common = {
                id: `${listId}-opt-${idx}`,
                role: 'option' as const,
                'aria-selected': isHi,
                onMouseEnter: () => setHighlighted(idx),
                onMouseDown: (e: React.MouseEvent) => e.preventDefault(),
                onClick: () => choose(row),
              };
              if (row.kind === 'walkin') {
                return (
                  <div key="walkin" {...common} className={base}>
                    <span>{walkInLabel}</span>
                    {!selectedPartyId && <Check className="h-3.5 w-3.5 text-blue-600" />}
                  </div>
                );
              }
              if (row.kind === 'create') {
                return (
                  <div key="create" {...common} className={cn(base, 'text-blue-700 dark:text-blue-300 font-medium')}>
                    <span className="flex items-center gap-1.5">
                      <UserPlus className="h-3.5 w-3.5" />
                      Create new {noun}
                    </span>
                    <kbd className="px-1 py-0.5 rounded bg-black/5 dark:bg-white/10 text-[10px] font-mono">
                      {createHotkey}
                    </kbd>
                  </div>
                );
              }
              const p = row.party;
              return (
                <div key={p.id} {...common} className={cn(base, p.id === selectedPartyId && 'font-semibold')}>
                  <div className="flex flex-col">
                    <span className="font-medium text-slate-900 dark:text-slate-100">{p.name}</span>
                    {p.phone && <span className="text-[10px] text-slate-400 font-mono">{p.phone}</span>}
                  </div>
                  <div className="flex items-center gap-2">
                    {duePill(p.outstandingBalance)}
                    {p.id === selectedPartyId && <Check className="h-3.5 w-3.5 text-blue-600" />}
                  </div>
                </div>
              );
            })}
            {filtered.length === 0 && (
              <div className="px-3 py-2 text-xs text-slate-400 text-center">No parties found</div>
            )}
          </div>
        </div>
      )}

      {mode === 'create' && (
        <div
          role="group"
          aria-label={`Create new ${noun}`}
          className="absolute z-50 mt-1 w-full rounded-md bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 shadow-xl p-3 flex flex-col gap-2"
        >
          <span className="text-xs font-semibold text-slate-700 dark:text-slate-200">
            New {noun} <span className="text-slate-400 font-normal">(Enter to save, Esc to cancel)</span>
          </span>
          <input
            ref={nameRef}
            type="text"
            aria-label={`New ${noun} name`}
            placeholder="Name"
            value={newName}
            onChange={(e) => setNewName(e.target.value)}
            onKeyDown={onCreateKeyDown}
            disabled={isCreating}
            className="w-full text-xs px-2 py-1.5 rounded border border-slate-200 dark:border-slate-700 bg-slate-50 dark:bg-slate-800 text-slate-900 dark:text-slate-100 focus:outline-none focus:ring-1 focus:ring-blue-500"
          />
          <input
            type="text"
            inputMode="tel"
            aria-label={`New ${noun} phone`}
            placeholder="Phone (optional)"
            value={newPhone}
            onChange={(e) => setNewPhone(e.target.value)}
            onKeyDown={onCreateKeyDown}
            disabled={isCreating}
            className="w-full text-xs px-2 py-1.5 rounded border border-slate-200 dark:border-slate-700 bg-slate-50 dark:bg-slate-800 text-slate-900 dark:text-slate-100 focus:outline-none focus:ring-1 focus:ring-blue-500"
          />
          {createError && (
            <p role="alert" className="text-xs text-rose-600 dark:text-rose-400">
              {createError}
            </p>
          )}
        </div>
      )}
    </div>
  );
};

export default PartySelector;
