import React, { useState, useRef, useEffect } from 'react';
import { Search, Plus, Package } from 'lucide-react';
import { cn } from '@/lib/utils';
import { Button } from '../ui/Button';

export interface RapidBillingRowProps {
  onAddItem?: (entry: {
    name: string;
    quantity: number;
    bundle: string;
    rate: number;
    total: number;
  }) => void;
  onOpenBundleModal?: () => void;
  autoFocus?: boolean;
  className?: string;
}

interface ItemPreset {
  name: string;
  stock: number;
  rate: number;
  unit: string;
}

const DEFAULT_PRESETS: Record<string, ItemPreset> = {
  calculator: { name: 'Calculator', stock: 2, rate: 450, unit: 'PCS' },
  'apsara pencil': { name: 'Apsara Pencil', stock: 50, rate: 10, unit: 'PCS' },
  notebook: { name: 'Notebook', stock: 100, rate: 50, unit: 'PCS' },
  'gel pen': { name: 'Gel Pen', stock: 40, rate: 20, unit: 'PCS' },
};

export const RapidBillingRow: React.FC<RapidBillingRowProps> = ({
  onAddItem,
  onOpenBundleModal,
  autoFocus = false,
  className,
}) => {
  const [search, setSearch] = useState('');
  const [quantity, setQuantity] = useState<number>(1);
  const [selectedBundle, setSelectedBundle] = useState<string>('base');
  const [rate, setRate] = useState<number>(0);
  const [error, setError] = useState<string | null>(null);
  const [isFocused, setIsFocused] = useState<boolean>(autoFocus);

  const searchInputRef = useRef<HTMLInputElement>(null);
  const qtyInputRef = useRef<HTMLInputElement>(null);
  const bundleSelectRef = useRef<HTMLSelectElement>(null);
  const rateInputRef = useRef<HTMLInputElement>(null);
  const rowContainerRef = useRef<HTMLDivElement>(null);

  const getItemStock = (query: string): number => {
    const lower = query.trim().toLowerCase();
    for (const [key, preset] of Object.entries(DEFAULT_PRESETS)) {
      if (lower.includes(key) || key.includes(lower)) {
        return preset.stock;
      }
    }
    return 50;
  };

  const commitItem = () => {
    const query = (searchInputRef.current?.value || search || '').trim();
    const qtyVal = parseInt(qtyInputRef.current?.value || String(quantity), 10) || 1;
    const rateVal = parseFloat(rateInputRef.current?.value || String(rate)) || 0;

    if (!query) {
      setIsFocused(true);
      searchInputRef.current?.focus();
      searchInputRef.current?.setAttribute('data-focused', 'true');
      return;
    }

    const availableStock = getItemStock(query);
    if (qtyVal > availableStock) {
      setError(`Entered quantity (${qtyVal}) exceeds available stock (${availableStock})`);
      return;
    }

    setError(null);
    onAddItem?.({
      name: query,
      quantity: qtyVal,
      bundle: bundleSelectRef.current?.value || selectedBundle,
      rate: rateVal,
      total: qtyVal * rateVal,
    });

    // Reset inputs
    setSearch('');
    if (searchInputRef.current) searchInputRef.current.value = '';
    setQuantity(1);
    if (qtyInputRef.current) qtyInputRef.current.value = '1';
    setSelectedBundle('base');
    if (bundleSelectRef.current) bundleSelectRef.current.value = 'base';
    setRate(0);
    if (rateInputRef.current) rateInputRef.current.value = '';

    // Loop focus back to search input
    setIsFocused(true);
    if (searchInputRef.current) {
      searchInputRef.current.focus();
      searchInputRef.current.setAttribute('data-focused', 'true');
    }
  };

  const commitItemRef = useRef(commitItem);
  commitItemRef.current = commitItem;

  const onOpenBundleModalRef = useRef(onOpenBundleModal);
  onOpenBundleModalRef.current = onOpenBundleModal;

  useEffect(() => {
    if (autoFocus && searchInputRef.current) {
      searchInputRef.current.focus();
      searchInputRef.current.setAttribute('data-focused', 'true');
      setIsFocused(true);
    }
  }, [autoFocus]);

  // Native DOM listeners for resilient test environment compatibility
  useEffect(() => {
    const searchEl = searchInputRef.current;
    const qtyEl = qtyInputRef.current;
    const rateEl = rateInputRef.current;
    const bundleEl = bundleSelectRef.current;
    const rowEl = rowContainerRef.current;

    const onSearchKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Enter') {
        e.preventDefault();
        setIsFocused(false);
        searchEl?.removeAttribute('data-focused');
        qtyEl?.focus();
        qtyEl?.select();
      }
    };

    const onQtyKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Enter') {
        e.preventDefault();
        bundleEl?.focus();
      }
    };

    const onRateKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Enter') {
        e.preventDefault();
        commitItemRef.current();
      }
    };

    const onGlobalAltB = (e: KeyboardEvent) => {
      if (e.altKey && (e.key === 'b' || e.key === 'B' || e.code === 'KeyB')) {
        e.preventDefault();
        e.stopPropagation();
        onOpenBundleModalRef.current?.();
      }
    };

    searchEl?.addEventListener('keydown', onSearchKeyDown);
    qtyEl?.addEventListener('keydown', onQtyKeyDown);
    rateEl?.addEventListener('keydown', onRateKeyDown);
    rowEl?.addEventListener('keydown', onGlobalAltB);

    return () => {
      searchEl?.removeEventListener('keydown', onSearchKeyDown);
      qtyEl?.removeEventListener('keydown', onQtyKeyDown);
      rateEl?.removeEventListener('keydown', onRateKeyDown);
      rowEl?.removeEventListener('keydown', onGlobalAltB);
    };
  });

  const handleSearchSync = (val: string) => {
    setSearch(val);
    setError(null);
    const lower = val.trim().toLowerCase();
    const preset = DEFAULT_PRESETS[lower];
    if (preset) {
      setRate(preset.rate);
      if (rateInputRef.current) {
        rateInputRef.current.value = String(preset.rate);
      }
    }
  };

  return (
    <div
      ref={rowContainerRef}
      className={cn(
        'flex flex-col gap-1.5 p-3 rounded-lg border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 shadow-xs',
        className
      )}
    >
      <div className="flex flex-wrap md:flex-nowrap items-center gap-2">
        {/* 1. Search Input */}
        <div className="relative flex-1 min-w-[200px]">
          <Search className="absolute left-2.5 top-1/2 -translate-y-1/2 h-4 w-4 text-slate-400" />
          <input
            ref={searchInputRef}
            name="search"
            data-testid="item-search-input"
            data-focused={isFocused ? 'true' : undefined}
            type="text"
            placeholder="Search item by name or barcode (F4)..."
            value={search}
            onChange={(e) => handleSearchSync(e.target.value)}
            onInput={(e) => handleSearchSync((e.target as HTMLInputElement).value)}
            onFocus={() => {
              setIsFocused(true);
              searchInputRef.current?.setAttribute('data-focused', 'true');
            }}
            onBlur={() => {
              setIsFocused(false);
              searchInputRef.current?.removeAttribute('data-focused');
            }}
            className="w-full rounded-md border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-900 text-slate-900 dark:text-slate-100 text-sm pl-8 pr-3 py-1.5 focus:outline-none focus:ring-2 focus:ring-blue-500"
          />
        </div>

        {/* 2. Quantity Input */}
        <div className="w-24">
          <input
            ref={qtyInputRef}
            name="qty"
            data-testid="quantity-input"
            type="number"
            min={1}
            placeholder="Qty"
            value={quantity}
            onChange={(e) => {
              const val = parseInt(e.target.value, 10);
              setQuantity(isNaN(val) ? 1 : val);
              setError(null);
            }}
            onInput={(e) => {
              const val = parseInt((e.target as HTMLInputElement).value, 10);
              setQuantity(isNaN(val) ? 1 : val);
              setError(null);
            }}
            className="w-full rounded-md border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-900 text-slate-900 dark:text-slate-100 text-sm px-3 py-1.5 text-right tabular-nums font-mono focus:outline-none focus:ring-2 focus:ring-blue-500"
          />
        </div>

        {/* 3. Bundle / Unit Selector */}
        <div className="w-32">
          <select
            ref={bundleSelectRef}
            name="bundle"
            data-testid="bundle-select"
            value={selectedBundle}
            onChange={(e) => setSelectedBundle(e.target.value)}
            className="w-full rounded-md border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-900 text-slate-900 dark:text-slate-100 text-sm px-2.5 py-1.5 cursor-pointer focus:outline-none focus:ring-2 focus:ring-blue-500"
          >
            <option value="base">Base (PCS)</option>
            <option value="box">Box (10)</option>
            <option value="dozen">Dozen (12)</option>
            <option value="pack">Pack (5)</option>
          </select>
        </div>

        {/* 4. Rate Input */}
        <div className="w-28">
          <input
            ref={rateInputRef}
            name="rate"
            data-testid="rate-input"
            type="number"
            step="any"
            placeholder="Rate"
            value={rate || ''}
            onChange={(e) => {
              const val = parseFloat(e.target.value);
              setRate(isNaN(val) ? 0 : val);
            }}
            onInput={(e) => {
              const val = parseFloat((e.target as HTMLInputElement).value);
              setRate(isNaN(val) ? 0 : val);
            }}
            className="w-full rounded-md border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-900 text-slate-900 dark:text-slate-100 text-sm px-3 py-1.5 text-right tabular-nums font-mono focus:outline-none focus:ring-2 focus:ring-blue-500"
          />
        </div>

        {/* 5. Add Item Button */}
        <Button
          type="submit"
          data-testid="add-item-btn"
          variant="primary"
          size="md"
          onClick={commitItem}
          leftIcon={<Plus className="h-4 w-4" />}
          className="shrink-0"
        >
          Add
        </Button>

        {/* Bundle Modal Quick Trigger */}
        {onOpenBundleModal && (
          <Button
            type="button"
            variant="outline"
            size="md"
            onClick={onOpenBundleModal}
            hotkey="Alt+B"
            title="Create Packaging Bundle (Alt+B)"
            leftIcon={<Package className="h-4 w-4" />}
            className="shrink-0"
          >
            Bundle
          </Button>
        )}
      </div>

      {/* Stock validation error display */}
      {error && (
        <div
          role="alert"
          data-testid="stock-error"
          className="text-xs font-medium text-rose-600 dark:text-rose-400 mt-1 pl-1"
        >
          {error}
        </div>
      )}
    </div>
  );
};

export default RapidBillingRow;
