import React, { useState, useEffect, useRef, useMemo } from 'react';
import { Package } from 'lucide-react';
import { api } from '@/api';
import { errorMessage } from '@/api/errors';
import type { BundleSuggestion, ItemBundle, SavedBundle } from '@/types';
import { Modal } from '../ui/Modal';
import { FormActions, enterToSubmit } from '../ui/FormActions';
import { TextInput } from '../ui/TextInput';
import { parseOptional, validationField } from './formUtils';

export interface BundleCreationModalProps {
  isOpen: boolean;
  onClose: () => void;
  /** Receives the persisted bundle with its backend-suggested rate (create or update). */
  onBundleCreated?: (bundle: SavedBundle) => void;
  initialItemId?: number;
  baseUnit?: string;
  /** When set the modal edits this bundle (updateBundle) instead of creating one. */
  bundle?: ItemBundle | null;
  /**
   * False when `bundle.mrp` is not known to the caller (catalog rows omit bundle MRP):
   * the modal warns that saving with an empty MRP clears it.
   */
  mrpKnown?: boolean;
}

type FieldErrors = Partial<
  Record<'bundleName' | 'unitsPerBundle' | 'mrp' | 'defaultBundlePrice' | 'barcode', string>
>;

const FIELD_MAP: Record<string, keyof FieldErrors> = {
  bundle_name: 'bundleName',
  bundleName: 'bundleName',
  units_per_bundle: 'unitsPerBundle',
  unitsPerBundle: 'unitsPerBundle',
  mrp: 'mrp',
  default_bundle_price: 'defaultBundlePrice',
  defaultBundlePrice: 'defaultBundlePrice',
  barcode: 'barcode',
};

export const BundleCreationModal: React.FC<BundleCreationModalProps> = ({
  isOpen,
  onClose,
  onBundleCreated,
  initialItemId,
  baseUnit = 'PCS',
  bundle = null,
  mrpKnown = true,
}) => {
  const [bundleName, setBundleName] = useState('');
  const [multiplier, setMultiplier] = useState('10');
  const [salePrice, setSalePrice] = useState('');
  const [mrp, setMrp] = useState('');
  const [barcode, setBarcode] = useState('');
  const [suggestions, setSuggestions] = useState<BundleSuggestion[]>([]);
  const [showSuggestions, setShowSuggestions] = useState(false);
  const [highlight, setHighlight] = useState(0);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [fieldErrors, setFieldErrors] = useState<FieldErrors>({});
  const submitting = useRef(false);
  const priceRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (!isOpen) return;
    setBundleName(bundle?.bundleName ?? '');
    setMultiplier(bundle ? String(bundle.unitsPerBundle) : '10');
    setSalePrice(bundle?.defaultBundlePrice != null ? String(bundle.defaultBundlePrice) : '');
    setMrp(bundle?.mrp != null ? String(bundle.mrp) : '');
    setBarcode(bundle?.barcode ?? '');
    setError(null);
    setFieldErrors({});
    setShowSuggestions(false);
    submitting.current = false;

    api
      .getBundleSuggestions()
      .then((sugs) => setSuggestions(Array.isArray(sugs) ? sugs : []))
      .catch(() => {});
  }, [isOpen, bundle]);

  const filtered = useMemo(() => {
    const q = bundleName.trim().toLowerCase();
    return suggestions
      .filter((s) => !q || s.bundleName.toLowerCase().includes(q))
      .slice(0, 6);
  }, [suggestions, bundleName]);

  const applySuggestion = (sug: BundleSuggestion) => {
    setBundleName(sug.bundleName);
    setMultiplier(String(sug.unitsPerBundle));
    setShowSuggestions(false);
    setFieldErrors((f) => ({ ...f, bundleName: undefined, unitsPerBundle: undefined }));
    setTimeout(() => priceRef.current?.focus(), 0);
  };

  const handleSave = async () => {
    if (submitting.current) return;
    const errs: FieldErrors = {};
    const mult = Number(multiplier);
    const price = parseOptional(salePrice);
    const mrpVal = parseOptional(mrp);

    if (!bundleName.trim()) errs.bundleName = 'Bundle name is required';
    // Format only (DD-56, DD-97): the ranges (multiplier, price, MRP) are the backend's to refuse.
    if (!Number.isFinite(mult)) errs.unitsPerBundle = 'Enter a number for the multiplier';
    if (price !== null && !Number.isFinite(price)) errs.defaultBundlePrice = 'Enter a number (or leave empty)';
    if (mrpVal !== null && !Number.isFinite(mrpVal)) errs.mrp = 'Enter a number (or leave empty)';
    setFieldErrors(errs);
    if (Object.keys(errs).length > 0) {
      setError(null);
      return;
    }

    submitting.current = true;
    setIsLoading(true);
    setError(null);

    const body = {
      bundleName: bundleName.trim(),
      unitsPerBundle: mult,
      defaultBundlePrice: price,
      mrp: mrpVal,
      barcode: barcode.trim() || null,
    };

    try {
      const res = bundle
        ? await api.updateBundle(bundle.id, body)
        : await api.createBundle({ itemId: initialItemId ?? null, ...body });
      onBundleCreated?.(res);
      onClose();
    } catch (err) {
      const field = validationField(err);
      const key = field ? FIELD_MAP[field] : undefined;
      if (key) setFieldErrors({ [key]: errorMessage(err) });
      else setError(errorMessage(err) || 'Failed to save bundle');
    } finally {
      submitting.current = false;
      setIsLoading(false);
    }
  };

  const onNameKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (!showSuggestions || filtered.length === 0) return;
    if (e.key === 'ArrowDown') {
      e.preventDefault();
      setHighlight((h) => (h + 1) % filtered.length);
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      setHighlight((h) => (h <= 0 ? filtered.length - 1 : h - 1));
    } else if (e.key === 'Enter' && !e.ctrlKey && !e.metaKey) {
      e.preventDefault();
      applySuggestion(filtered[Math.min(highlight, filtered.length - 1)]);
    } else if (e.key === 'Escape') {
      e.stopPropagation();
      setShowSuggestions(false);
    }
  };

  const isEdit = !!bundle;
  const multNum = Number(multiplier);

  return (
    <Modal
      isOpen={isOpen}
      onClose={onClose}
      title={isEdit ? 'Edit Packaging Bundle' : 'Create Packaging Bundle'}
      description={`Define a packaged selling unit (e.g. Box of 10 ${baseUnit})`}
      size="md"
      footer={
        <FormActions
          onSave={() => void handleSave()}
          onCancel={onClose}
          isSubmitting={isLoading}
          canSave={bundleName.trim().length > 0}
          saveLabel={isEdit ? 'Save Bundle' : 'Create Bundle'}
          saveHotkey="Enter"
        />
      }
    >
      <div className="flex flex-col gap-4 py-1" onKeyDown={enterToSubmit(() => void handleSave())}>
        <div className="grid grid-cols-2 gap-3">
          <div className="relative">
            <TextInput
              label="Bundle Name *"
              placeholder="e.g. Box, Dozen, Pack"
              value={bundleName}
              onChange={(e) => {
                setBundleName(e.target.value);
                setShowSuggestions(true);
                setHighlight(0);
              }}
              onFocus={() => setShowSuggestions(true)}
              onBlur={() => setTimeout(() => setShowSuggestions(false), 120)}
              onKeyDown={onNameKeyDown}
              error={fieldErrors.bundleName}
              autoFocus
              autoComplete="off"
              role="combobox"
              aria-expanded={showSuggestions && filtered.length > 0}
              aria-autocomplete="list"
            />
            {showSuggestions && filtered.length > 0 && (
              <ul
                role="listbox"
                aria-label="Bundle suggestions"
                className="absolute z-50 mt-0.5 w-full max-h-48 overflow-auto rounded-md border border-slate-200 dark:border-slate-700 bg-white dark:bg-slate-900 shadow-lg py-1"
              >
                {filtered.map((s, idx) => (
                  <li
                    key={`${s.bundleName}-${s.unitsPerBundle}`}
                    role="option"
                    aria-selected={idx === highlight}
                    onMouseDown={(e) => {
                      e.preventDefault();
                      applySuggestion(s);
                    }}
                    onMouseEnter={() => setHighlight(idx)}
                    className={`flex items-center gap-1.5 px-3 py-1 text-xs cursor-pointer ${
                      idx === highlight ? 'bg-blue-50 dark:bg-blue-950/40' : ''
                    }`}
                  >
                    <Package className="h-3 w-3 text-slate-400" />
                    <span className="font-medium">{s.bundleName}</span>
                    <span className="text-slate-400">
                      ({s.unitsPerBundle} {baseUnit})
                    </span>
                  </li>
                ))}
              </ul>
            )}
          </div>

          <div>
            <TextInput
              label={`Multiplier (${baseUnit} per Bundle) *`}
              type="number"
              min={2}
              step="any"
              value={multiplier}
              onChange={(e) => setMultiplier(e.target.value)}
              error={fieldErrors.unitsPerBundle}
              autoSelectOnFocus
              className="text-right font-mono"
            />
          </div>
        </div>

        <div className="grid grid-cols-2 gap-3">
          <div>
            <TextInput
              ref={priceRef}
              label="Bundle Sale Price (₹)"
              type="number"
              min={0}
              step="any"
              placeholder="optional"
              value={salePrice}
              onChange={(e) => setSalePrice(e.target.value)}
              error={fieldErrors.defaultBundlePrice}
              autoSelectOnFocus
              className="text-right font-mono"
            />
          </div>

          <div>
            <TextInput
              label="Bundle MRP (₹)"
              title={isEdit && !mrpKnown ? 'Current MRP is not loaded: leaving this empty clears it' : undefined}
              type="number"
              min={0}
              step="any"
              placeholder={isEdit && !mrpKnown ? 'not loaded: re-enter or it is cleared' : 'optional'}
              value={mrp}
              onChange={(e) => setMrp(e.target.value)}
              error={fieldErrors.mrp}
              autoSelectOnFocus
              className="text-right font-mono"
            />
          </div>
        </div>

        <div>
          <TextInput
            label="Outer Package Barcode"
            data-enter-submit="off"
            placeholder="Scan outer package barcode (optional)"
            value={barcode}
            onChange={(e) => setBarcode(e.target.value)}
            error={fieldErrors.barcode}
          />
        </div>

        {Number.isFinite(multNum) && multNum > 1 && bundleName.trim() && (
          <p className="text-[11px] text-slate-500 font-mono">
            1 {bundleName.trim()} = {multNum} {baseUnit}
          </p>
        )}

        {error && (
          <p role="alert" className="text-xs text-rose-600 dark:text-rose-400 font-medium">
            {error}
          </p>
        )}
      </div>
    </Modal>
  );
};

export default BundleCreationModal;
