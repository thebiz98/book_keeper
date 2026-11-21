import React, { useState, useEffect } from 'react';
import { User, Truck, Search, Check, ChevronDown } from 'lucide-react';
import { cn } from '@/lib/utils';
import { api } from '@/api';
import type { Party, PartyType } from '@/types';
import { CurrencyDisplay } from '../common/CurrencyDisplay';

export interface PartySelectorProps {
  partyType: PartyType;
  selectedPartyId?: number | null;
  onSelectParty: (party: Party | null) => void;
  allowWalkIn?: boolean;
  className?: string;
}

export const PartySelector: React.FC<PartySelectorProps> = ({
  partyType,
  selectedPartyId = null,
  onSelectParty,
  allowWalkIn = true,
  className,
}) => {
  const [parties, setParties] = useState<Party[]>([]);
  const [dues, setDues] = useState<Record<number, number>>({});
  const [search, setSearch] = useState('');
  const [isOpen, setIsOpen] = useState(false);

  useEffect(() => {
    api
      .listParties(partyType, true)
      .then((data) => {
        setParties(data);
      })
      .catch(() => {});
    api
      .getPartyDuesSummary(partyType)
      .then((rows) => setDues(Object.fromEntries(rows.map((r) => [r.partyId, r.totalDue]))))
      .catch(() => {});
  }, [partyType]);

  const selectedParty = parties.find((p) => p.id === selectedPartyId);

  const filteredParties = parties.filter((p) => {
    if (!search) return true;
    const q = search.toLowerCase();
    const name = (p.name || '').toLowerCase();
    const phone = (p.phone || '').toLowerCase();
    return name.includes(q) || phone.includes(q);
  });

  const handleSelect = (party: Party) => {
    onSelectParty(party);
    setIsOpen(false);
    setSearch('');
  };

  const handleWalkIn = () => {
    onSelectParty(null);
    setIsOpen(false);
    setSearch('');
  };

  const Icon = partyType === 'Customer' ? User : Truck;
  const walkInLabel = partyType === 'Customer' ? 'Walk-in Customer' : 'Standard Vendor';

  return (
    <div className={cn('relative w-full', className)}>
      <div
        onClick={() => setIsOpen((prev) => !prev)}
        className="flex items-center justify-between w-full rounded-md border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-900 text-slate-900 dark:text-slate-100 text-sm px-3 py-1.5 cursor-pointer select-none focus-within:ring-2 focus-within:ring-blue-500"
      >
        <div className="flex items-center gap-2 overflow-hidden truncate">
          <Icon className="h-4 w-4 text-slate-400 shrink-0" />
          {selectedParty ? (
            <div className="flex items-center gap-2 truncate">
              <span className="font-medium truncate">{selectedParty.name}</span>
              {selectedParty.phone && (
                <span className="text-xs text-slate-400 font-mono">({selectedParty.phone})</span>
              )}
              {(dues[selectedParty.id] ?? 0) > 0 && (
                <span className="px-1.5 py-0.2 rounded text-[10px] font-semibold bg-rose-50 text-rose-700 dark:bg-rose-950 dark:text-rose-300 border border-rose-200 dark:border-rose-800">
                  Due: <CurrencyDisplay amount={dues[selectedParty.id] ?? 0} size="sm" />
                </span>
              )}
            </div>
          ) : (
            <span className="text-slate-500 dark:text-slate-400 font-medium">{walkInLabel}</span>
          )}
        </div>
        <ChevronDown className="h-4 w-4 text-slate-400 shrink-0" />
      </div>

      {isOpen && (
        <div className="absolute z-50 mt-1 max-h-60 w-full overflow-auto rounded-md bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 shadow-xl py-1">
          <div className="p-2 border-b border-slate-100 dark:border-slate-800 sticky top-0 bg-white dark:bg-slate-900">
            <div className="relative flex items-center">
              <Search className="h-3.5 w-3.5 absolute left-2 text-slate-400" />
              <input
                type="text"
                value={search}
                onChange={(e) => setSearch(e.target.value)}
                placeholder={`Search ${partyType.toLowerCase()} by name or phone...`}
                className="w-full text-xs pl-7 pr-2 py-1 rounded border border-slate-200 dark:border-slate-700 bg-slate-50 dark:bg-slate-800 text-slate-900 dark:text-slate-100 focus:outline-none focus:ring-1 focus:ring-blue-500"
                autoFocus
              />
            </div>
          </div>

          {allowWalkIn && (
            <div
              onClick={handleWalkIn}
              className={cn(
                'flex items-center justify-between px-3 py-2 text-xs cursor-pointer select-none transition-colors border-b border-slate-50 dark:border-slate-800/60',
                !selectedPartyId ? 'bg-blue-50 dark:bg-blue-950/40 text-blue-900 dark:text-blue-100 font-semibold' : 'hover:bg-slate-50 dark:hover:bg-slate-800/40'
              )}
            >
              <span>{walkInLabel}</span>
              {!selectedPartyId && <Check className="h-3.5 w-3.5 text-blue-600" />}
            </div>
          )}

          {filteredParties.length === 0 ? (
            <div className="px-3 py-3 text-xs text-slate-400 text-center">No parties found</div>
          ) : (
            filteredParties.map((p) => {
              const isSelected = p.id === selectedPartyId;
              const due = dues[p.id] ?? 0;

              return (
                <div
                  key={p.id}
                  onClick={() => handleSelect(p)}
                  className={cn(
                    'flex items-center justify-between px-3 py-2 text-xs cursor-pointer select-none transition-colors',
                    isSelected
                      ? 'bg-blue-50 dark:bg-blue-950/40 text-blue-900 dark:text-blue-100 font-semibold'
                      : 'hover:bg-slate-50 dark:hover:bg-slate-800/40'
                  )}
                >
                  <div className="flex flex-col">
                    <span className="font-medium text-slate-900 dark:text-slate-100">{p.name}</span>
                    {p.phone && <span className="text-[10px] text-slate-400 font-mono">{p.phone}</span>}
                  </div>

                  <div className="flex items-center gap-2">
                    {due > 0 && (
                      <span className="text-[10px] px-1.5 py-0.5 rounded font-mono font-semibold bg-rose-50 text-rose-700 dark:bg-rose-950 dark:text-rose-300">
                        Due: <CurrencyDisplay amount={due} size="sm" />
                      </span>
                    )}
                    {isSelected && <Check className="h-3.5 w-3.5 text-blue-600" />}
                  </div>
                </div>
              );
            })
          )}
        </div>
      )}
    </div>
  );
};

export default PartySelector;
