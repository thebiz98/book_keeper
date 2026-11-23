import React, {
  useState,
  useRef,
  useEffect,
  useImperativeHandle,
  forwardRef,
} from 'react';
import { Plus, Package } from 'lucide-react';
import { cn } from '@/lib/utils';
import { round2 } from '@/lib/money';
import type { ItemBundle, ItemBundleSearchResult, ItemSearchResult } from '@/types';
import type { BundleInput } from '@/types/store';
import { useEscapeLayer, useHotkeyAction } from '@/hooks/useHotkeys';
import { cartBaseQuantity, STOCK_EPS, type CartLine } from '@/store/useCartStore';
import { Button } from '../ui/Button';
import { CurrencyDisplay } from '../common/CurrencyDisplay';
import { ItemSearchTypeahead } from './ItemSearchTypeahead';
import { ItemQuickCreateModal } from './ItemQuickCreateModal';

export interface BillingLineEntry {
  item: ItemSearchResult;
  quantity: number;
  /** `null` = base unit. */
  bundle: BundleInput | null;
  rate: number;
}

export interface RapidBillingRowProps {
  /** Current cart lines, used to show stock net of what is already in the cart. */
  cart?: CartLine[];
  /** Commit a line. Return `false` to report a rejection (the row stays filled). */
  onAddLine?: (entry: BillingLineEntry) => boolean | void;
  /** Enter on an empty search box. */
  onCompleteBill?: () => void;
  /** Alt+B / Bundle button. The selected item (if any) is passed so the modal can preselect it. */
  onOpenBundleModal?: (item: ItemSearchResult | null) => void;
  /** A bundle just created through the modal; it is added to the unit list and selected. */
  newBundle?: { bundle: ItemBundle; seq: number } | null;
  autoFocus?: boolean;
  className?: string;
}

export interface RapidBillingRowHandle {
  focusSearch: () => void;
}

const BASE = 'base';
const NUMBER_RE = /^\d*\.?\d*$/;

const fmtQty = (n: number) => String(Number(n.toFixed(3)));

function toBundleInput(b: ItemBundleSearchResult, mrp: number | null): BundleInput {
  return {
    id: b.bundleId,
    bundleName: b.bundleName,
    unitsPerBundle: b.unitsPerBundle,
    salePrice: b.bundlePrice,
    mrp,
  };
}

function defaultRate(item: ItemSearchResult, bundle: ItemBundleSearchResult | null): number {
  if (!bundle) return item.defaultSalePrice;
  return bundle.bundlePrice ?? round2(item.defaultSalePrice * bundle.unitsPerBundle);
}

const inputClass =
  'w-full rounded-md border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-900 text-slate-900 dark:text-slate-100 text-sm px-3 py-1.5 text-right tabular-nums font-mono focus:outline-none focus:ring-2 focus:ring-blue-500';

export const RapidBillingRow = forwardRef<RapidBillingRowHandle, RapidBillingRowProps>(
  (
    { cart = [], onAddLine, onCompleteBill, onOpenBundleModal, newBundle, autoFocus = false, className },
    ref
  ) => {
    const [item, setItem] = useState<ItemSearchResult | null>(null);
    const [bundles, setBundles] = useState<ItemBundleSearchResult[]>([]);
    const [bundleMrp, setBundleMrp] = useState<Record<number, number | null>>({});
    const [qty, setQty] = useState('1');
    const [unit, setUnit] = useState<string>(BASE);
    const [rate, setRate] = useState('');
    const [error, setError] = useState<string | null>(null);

    const searchRef = useRef<HTMLInputElement | null>(null);
    const qtyRef = useRef<HTMLInputElement>(null);
    const unitRef = useRef<HTMLSelectElement>(null);
    const rateRef = useRef<HTMLInputElement>(null);
    const handledBundleSeq = useRef<number | null>(null);

    useImperativeHandle(ref, () => ({ focusSearch: () => searchRef.current?.focus() }));

    const selectedBundle =
      unit === BASE ? null : bundles.find((b) => String(b.bundleId) === unit) ?? null;
    const units = selectedBundle ? selectedBundle.unitsPerBundle : 1;

    const inCart = item ? cartBaseQuantity(cart, item.itemId) : 0;
    const available = item ? item.currentStock - inCart : 0;
    const mrpShown = item
      ? selectedBundle
        ? bundleMrp[selectedBundle.bundleId] ?? null
        : item.mrp
      : null;

    // Focus Qty whenever a new item is selected.
    useEffect(() => {
      if (item) {
        qtyRef.current?.focus();
        qtyRef.current?.select();
      }
    }, [item]);

    useEffect(() => {
      if (autoFocus) searchRef.current?.focus();
    }, [autoFocus]);

    const clearEntry = (focusSearch = true) => {
      setItem(null);
      setBundles([]);
      setQty('1');
      setUnit(BASE);
      setRate('');
      setError(null);
      if (focusSearch) searchRef.current?.focus();
    };

    // Esc first backs out of a half-entered line; with nothing pending it falls through to the page.
    useEscapeLayer(() => clearEntry(), item !== null);

    // A bundle created from the modal is auto-selected for the current item.
    useEffect(() => {
      if (!newBundle || handledBundleSeq.current === newBundle.seq) return;
      handledBundleSeq.current = newBundle.seq;
      if (!item || newBundle.bundle.itemId !== item.itemId) return;
      const b = newBundle.bundle;
      const row: ItemBundleSearchResult = {
        bundleId: b.id,
        bundleName: b.bundleName,
        unitsPerBundle: b.unitsPerBundle,
        bundlePrice: b.defaultBundlePrice,
        barcode: b.barcode,
      };
      setBundles((prev) => (prev.some((x) => x.bundleId === b.id) ? prev : [...prev, row]));
      setBundleMrp((prev) => ({ ...prev, [b.id]: b.mrp }));
      setUnit(String(b.id));
      setRate(String(defaultRate(item, row)));
      setError(null);
      // After the modal's own focus restore has run.
      setTimeout(() => unitRef.current?.focus(), 0);
    }, [newBundle, item]);

    useHotkeyAction('newBundle', () => onOpenBundleModal?.(item), {
      scope: 'page',
      enabled: !!onOpenBundleModal,
    });

    // Alt+N / "+ Create" / Enter on no match: quick-create an item with opening stock (FR-24).
    const [quickName, setQuickName] = useState<string | null>(null);
    useHotkeyAction('newItem', () => setQuickName(searchRef.current?.value.trim() ?? ''), { scope: 'page' });

    const chooseUnit = (next: string, list: ItemBundleSearchResult[] = bundles) => {
      if (!item) return;
      setUnit(next);
      const b = next === BASE ? null : list.find((x) => String(x.bundleId) === next) ?? null;
      setRate(String(defaultRate(item, b)));
      setError(null);
    };

    const handleSelectItem = (it: ItemSearchResult, matchedBundleId: number | null) => {
      setItem(it);
      setBundles(it.bundles);
      setBundleMrp({});
      setQty('1');
      setError(null);
      const matched = matchedBundleId != null ? it.bundles.find((b) => b.bundleId === matchedBundleId) : null;
      setUnit(matched ? String(matched.bundleId) : BASE);
      setRate(String(defaultRate(it, matched ?? null)));
    };

    const parsedQty = parseFloat(qty);
    const parsedRate = parseFloat(rate);

    const commit = () => {
      if (!item) {
        searchRef.current?.focus();
        return;
      }
      if (!Number.isFinite(parsedQty) || parsedQty <= 0) {
        setError('Enter a quantity greater than 0');
        qtyRef.current?.focus();
        return;
      }
      if (!Number.isFinite(parsedRate) || parsedRate < 0) {
        setError('Enter a valid rate');
        rateRef.current?.focus();
        return;
      }
      const base = parsedQty * units;
      if (inCart + base > item.currentStock + STOCK_EPS) {
        setError(
          `Negative stock: ${item.name} has ${fmtQty(Math.max(available, 0))} ${item.baseUnit} available, ` +
            `${fmtQty(base)} ${item.baseUnit} requested`
        );
        qtyRef.current?.focus();
        qtyRef.current?.select();
        return;
      }
      const ok = onAddLine?.({
        item,
        quantity: parsedQty,
        bundle: selectedBundle ? toBundleInput(selectedBundle, bundleMrp[selectedBundle.bundleId] ?? null) : null,
        rate: parsedRate,
      });
      if (ok === false) {
        setError(`Cannot add "${item.name}": insufficient stock or invalid values`);
        return;
      }
      clearEntry(true);
    };

    const onQtyKeyDown = (e: React.KeyboardEvent) => {
      if (e.key === 'Enter' && !e.ctrlKey && !e.metaKey) {
        e.preventDefault();
        if (e.repeat) return;
        if (!Number.isFinite(parsedQty) || parsedQty <= 0) {
          setError('Enter a quantity greater than 0');
          return;
        }
        unitRef.current?.focus();
      }
    };

    const options = [BASE, ...bundles.map((b) => String(b.bundleId))];
    const onUnitKeyDown = (e: React.KeyboardEvent) => {
      if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
        e.preventDefault();
        const i = options.indexOf(unit);
        const n = e.key === 'ArrowDown' ? Math.min(i + 1, options.length - 1) : Math.max(i - 1, 0);
        chooseUnit(options[n]);
      } else if (e.key === 'Enter' && !e.ctrlKey && !e.metaKey) {
        e.preventDefault();
        if (e.repeat) return;
        rateRef.current?.focus();
        rateRef.current?.select();
      }
    };

    const onRateKeyDown = (e: React.KeyboardEvent) => {
      if (e.key === 'Enter' && !e.ctrlKey && !e.metaKey) {
        e.preventDefault();
        if (e.repeat) return;
        commit();
      }
    };

    return (
      <div
        className={cn(
          'flex flex-col gap-1.5 p-3 rounded-lg border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 shadow-xs',
          className
        )}
      >
        <div className="flex flex-wrap md:flex-nowrap items-center gap-2">
          <div className="flex-1 min-w-[200px]">
            <ItemSearchTypeahead
              inputRef={searchRef}
              autoFocus={autoFocus}
              onSelectItem={() => undefined}
              onSelect={(it, info) => handleSelectItem(it, info.matchedBundleId)}
              onEmptyEnter={onCompleteBill}
              onCreate={(q) => setQuickName(q)}
            />
          </div>

          <div className="w-24">
            <input
              ref={qtyRef}
              data-testid="quantity-input"
              aria-label="Quantity"
              type="text"
              inputMode="decimal"
              placeholder="Qty"
              disabled={!item}
              value={qty}
              onChange={(e) => {
                if (NUMBER_RE.test(e.target.value)) {
                  setQty(e.target.value);
                  setError(null);
                }
              }}
              onKeyDown={onQtyKeyDown}
              className={inputClass}
            />
          </div>

          <div className="w-40">
            <select
              ref={unitRef}
              data-testid="bundle-select"
              aria-label="Unit"
              disabled={!item}
              value={unit}
              onChange={(e) => chooseUnit(e.target.value)}
              onKeyDown={onUnitKeyDown}
              className="w-full rounded-md border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-900 text-slate-900 dark:text-slate-100 text-sm px-2.5 py-1.5 cursor-pointer focus:outline-none focus:ring-2 focus:ring-blue-500"
            >
              <option value={BASE}>Base ({item?.baseUnit ?? 'PCS'})</option>
              {bundles.map((b) => (
                <option key={b.bundleId} value={String(b.bundleId)}>
                  {b.bundleName}
                </option>
              ))}
            </select>
          </div>

          <div className="w-28">
            <input
              ref={rateRef}
              data-testid="rate-input"
              aria-label="Rate"
              type="text"
              inputMode="decimal"
              placeholder="Rate"
              disabled={!item}
              value={rate}
              onChange={(e) => {
                if (NUMBER_RE.test(e.target.value)) {
                  setRate(e.target.value);
                  setError(null);
                }
              }}
              onFocus={(e) => e.currentTarget.select()}
              onKeyDown={onRateKeyDown}
              className={inputClass}
            />
          </div>

          <Button
            type="button"
            data-testid="add-item-btn"
            variant="primary"
            size="md"
            disabled={!item}
            onClick={commit}
            leftIcon={<Plus className="h-4 w-4" />}
            className="shrink-0"
          >
            Add
          </Button>

          {onOpenBundleModal && (
            <Button
              type="button"
              variant="outline"
              size="md"
              onClick={() => onOpenBundleModal(item)}
              hotkey="Alt+B"
              title="Create Packaging Bundle (Alt+B)"
              leftIcon={<Package className="h-4 w-4" />}
              className="shrink-0"
            >
              Bundle
            </Button>
          )}
        </div>

        {item && (
          <div
            data-testid="stock-indicator"
            className="flex flex-wrap items-center gap-x-3 gap-y-0.5 text-xs font-mono text-slate-600 dark:text-slate-300 pl-1"
          >
            <span className="font-sans font-semibold text-slate-900 dark:text-slate-100">{item.name}</span>
            <span className={available <= STOCK_EPS ? 'text-rose-600 dark:text-rose-400 font-semibold' : ''}>
              Available: {fmtQty(Math.max(available, 0))} {item.baseUnit}
            </span>
            <span>|</span>
            <span>
              Rate: <CurrencyDisplay amount={Number.isFinite(parsedRate) ? parsedRate : 0} size="sm" />
            </span>
            <span>|</span>
            <span>
              MRP: {mrpShown != null ? <CurrencyDisplay amount={mrpShown} size="sm" /> : '—'}
            </span>
          </div>
        )}

        {error && (
          <div
            role="alert"
            data-testid="stock-error"
            className="text-xs font-medium text-rose-600 dark:text-rose-400 pl-1"
          >
            {error}
          </div>
        )}

        <ItemQuickCreateModal
          isOpen={quickName !== null}
          initialName={quickName ?? ''}
          withStock
          onClose={() => {
            setQuickName(null);
            setTimeout(() => searchRef.current?.focus(), 0);
          }}
          onCreated={(it) => {
            setQuickName(null);
            handleSelectItem(it, null);
            // After the modal's own focus restore has run.
            setTimeout(() => {
              qtyRef.current?.focus();
              qtyRef.current?.select();
            }, 0);
          }}
        />
      </div>
    );
  }
);

RapidBillingRow.displayName = 'RapidBillingRow';
export default RapidBillingRow;
