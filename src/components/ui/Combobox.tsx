import React, { useState, useRef, useEffect } from 'react';
import { Search, ChevronDown, Check } from 'lucide-react';
import { cn } from '@/lib/utils';

export interface ComboboxOption<T = string | number> {
  value: T;
  label: string;
  sublabel?: string;
  badge?: React.ReactNode;
  disabled?: boolean;
}

export interface ComboboxProps<T = string | number> {
  options: ComboboxOption<T>[];
  value: T | null;
  onChange: (value: T) => void;
  placeholder?: string;
  onSearchChange?: (query: string) => void;
  renderOption?: (option: ComboboxOption<T>) => React.ReactNode;
  className?: string;
  error?: string;
  disabled?: boolean;
}

export function Combobox<T extends string | number = string>({
  options,
  value,
  onChange,
  placeholder = 'Select option...',
  onSearchChange,
  renderOption,
  className,
  error,
  disabled = false,
}: ComboboxProps<T>) {
  const [isOpen, setIsOpen] = useState(false);
  const [search, setSearch] = useState('');
  const [highlightedIndex, setHighlightedIndex] = useState(0);
  const containerRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  const selectedOption = options.find((opt) => opt.value === value);

  const filteredOptions = React.useMemo(() => {
    if (!search) return options;
    return options.filter((opt) =>
      opt.label.toLowerCase().includes(search.toLowerCase()) ||
      (opt.sublabel && opt.sublabel.toLowerCase().includes(search.toLowerCase()))
    );
  }, [options, search]);

  useEffect(() => {
    const handleClickOutside = (e: MouseEvent) => {
      if (containerRef.current && !containerRef.current.contains(e.target as Node)) {
        setIsOpen(false);
      }
    };
    document.addEventListener('mousedown', handleClickOutside);
    return () => document.removeEventListener('mousedown', handleClickOutside);
  }, []);

  useEffect(() => {
    setHighlightedIndex(0);
  }, [filteredOptions]);

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (disabled) return;

    if (!isOpen) {
      if (e.key === 'ArrowDown' || e.key === 'Enter') {
        e.preventDefault();
        setIsOpen(true);
      }
      return;
    }

    if (e.key === 'ArrowDown') {
      e.preventDefault();
      setHighlightedIndex((prev) => (prev + 1) % Math.max(filteredOptions.length, 1));
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      setHighlightedIndex((prev) =>
        prev <= 0 ? Math.max(filteredOptions.length - 1, 0) : prev - 1
      );
    } else if (e.key === 'Enter') {
      e.preventDefault();
      if (filteredOptions.length > 0) {
        const item = filteredOptions[highlightedIndex];
        if (item && !item.disabled) {
          onChange(item.value);
          setIsOpen(false);
          setSearch('');
        }
      }
    } else if (e.key === 'Escape') {
      e.preventDefault();
      setIsOpen(false);
    }
  };

  const handleSelect = (option: ComboboxOption<T>) => {
    if (option.disabled) return;
    onChange(option.value);
    setIsOpen(false);
    setSearch('');
  };

  return (
    <div ref={containerRef} className={cn('relative w-full', className)}>
      <div
        className={cn(
          'flex items-center justify-between w-full rounded-md border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-900 text-slate-900 dark:text-slate-100 text-sm px-3 py-1.5 cursor-pointer focus-within:ring-2 focus-within:ring-blue-500 focus-within:border-blue-500',
          disabled && 'opacity-50 cursor-not-allowed bg-slate-50 dark:bg-slate-800',
          error && 'border-rose-500 focus-within:ring-rose-500'
        )}
        onClick={() => {
          if (!disabled) {
            setIsOpen((prev) => !prev);
            setTimeout(() => inputRef.current?.focus(), 50);
          }
        }}
        onKeyDown={handleKeyDown}
        tabIndex={disabled ? -1 : 0}
      >
        <div className="flex items-center gap-2 overflow-hidden truncate">
          {selectedOption ? (
            <span className="truncate">{selectedOption.label}</span>
          ) : (
            <span className="text-slate-400">{placeholder}</span>
          )}
          {selectedOption?.badge}
        </div>
        <ChevronDown className="h-4 w-4 shrink-0 text-slate-400 dark:text-slate-500" />
      </div>

      {isOpen && (
        <div className="absolute z-50 mt-1 max-h-60 w-full overflow-auto rounded-md bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 shadow-lg py-1">
          <div className="px-2 py-1 sticky top-0 bg-white dark:bg-slate-900 border-b border-slate-100 dark:border-slate-800">
            <div className="relative flex items-center">
              <Search className="h-3.5 w-3.5 absolute left-2 text-slate-400" />
              <input
                ref={inputRef}
                value={search}
                onChange={(e) => {
                  setSearch(e.target.value);
                  onSearchChange?.(e.target.value);
                }}
                onKeyDown={handleKeyDown}
                placeholder="Search..."
                className="w-full text-xs pl-7 pr-2 py-1 rounded border border-slate-200 dark:border-slate-700 bg-slate-50 dark:bg-slate-800 text-slate-900 dark:text-slate-100 focus:outline-none focus:ring-1 focus:ring-blue-500"
              />
            </div>
          </div>
          <div className="py-1">
            {filteredOptions.length === 0 ? (
              <div className="px-3 py-2 text-xs text-slate-400 text-center">
                No results found
              </div>
            ) : (
              filteredOptions.map((opt, idx) => {
                const isSelected = opt.value === value;
                const isHighlighted = idx === highlightedIndex;

                return (
                  <div
                    key={String(opt.value)}
                    onClick={() => handleSelect(opt)}
                    onMouseEnter={() => setHighlightedIndex(idx)}
                    className={cn(
                      'flex items-center justify-between px-3 py-1.5 text-xs cursor-pointer select-none transition-colors',
                      isHighlighted && 'bg-blue-50 dark:bg-blue-950/40 text-blue-900 dark:text-blue-100',
                      isSelected && 'font-semibold text-blue-600 dark:text-blue-400',
                      opt.disabled && 'opacity-40 cursor-not-allowed'
                    )}
                  >
                    {renderOption ? (
                      renderOption(opt)
                    ) : (
                      <div className="flex flex-col">
                        <span className="text-slate-800 dark:text-slate-200">{opt.label}</span>
                        {opt.sublabel && (
                          <span className="text-[10px] text-slate-400">{opt.sublabel}</span>
                        )}
                      </div>
                    )}
                    <div className="flex items-center gap-1.5">
                      {opt.badge}
                      {isSelected && <Check className="h-3.5 w-3.5 text-blue-600 dark:text-blue-400" />}
                    </div>
                  </div>
                );
              })
            )}
          </div>
        </div>
      )}
      {error && (
        <p role="alert" className="text-xs text-rose-600 dark:text-rose-400 mt-0.5">
          {error}
        </p>
      )}
    </div>
  );
}

export default Combobox;
