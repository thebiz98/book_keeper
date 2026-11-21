import React, { useState, useEffect } from 'react';
import { Package, Sparkles } from 'lucide-react';
import { api } from '@/api';
import type { BundleSuggestion } from '@/types';
import { Modal } from '../ui/Modal';
import { Button } from '../ui/Button';
import { TextInput } from '../ui/TextInput';
import { NumericInput } from '../ui/NumericInput';

export interface BundleCreationModalProps {
  isOpen: boolean;
  onClose: () => void;
  onBundleCreated?: (bundleId: number) => void;
  initialItemId?: number;
  baseUnit?: string;
}

export const BundleCreationModal: React.FC<BundleCreationModalProps> = ({
  isOpen,
  onClose,
  onBundleCreated,
  initialItemId,
  baseUnit = 'PCS',
}) => {
  const [bundleName, setBundleName] = useState('');
  const [multiplier, setMultiplier] = useState<number>(10);
  const [salePrice, setSalePrice] = useState<number>(0);
  const [mrp, setMrp] = useState<number>(0);
  const [barcode, setBarcode] = useState('');
  const [suggestions, setSuggestions] = useState<BundleSuggestion[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (isOpen) {
      setBundleName('');
      setMultiplier(10);
      setSalePrice(0);
      setMrp(0);
      setBarcode('');
      setError(null);

      // Fetch dynamic suggestions
      api
        .getBundleSuggestions()
        .then((sugs) => {
          setSuggestions(sugs.slice(0, 4));
        })
        .catch(() => {});
    }
  }, [isOpen]);

  const handleApplySuggestion = (sug: BundleSuggestion) => {
    setBundleName(sug.bundleName);
    setMultiplier(sug.unitsPerBundle);
  };

  const handleSave = async () => {
    if (!bundleName.trim()) {
      setError('Bundle name is required');
      return;
    }
    if (multiplier <= 0) {
      setError('Multiplier must be greater than 0');
      return;
    }

    setIsLoading(true);
    setError(null);

    const payload = {
      itemId: initialItemId ?? null,
      bundleName: bundleName.trim(),
      unitsPerBundle: multiplier,
      defaultBundlePrice: salePrice > 0 ? salePrice : null,
      mrp: mrp > 0 ? mrp : null,
      barcode: barcode.trim() || null,
    };

    try {
      const res = await api.createBundle(payload);
      onBundleCreated?.(res);
      onClose();
    } catch (err: any) {
      setError(err?.message || 'Failed to create bundle');
    } finally {
      setIsLoading(false);
    }
  };

  return (
    <Modal
      isOpen={isOpen}
      onClose={onClose}
      title="Create Packaging Bundle"
      description={`Define a packaged selling unit (e.g. Box of 10 ${baseUnit})`}
      size="md"
      footer={
        <>
          <Button variant="outline" size="sm" onClick={onClose} hotkey="Esc">
            Cancel
          </Button>
          <Button
            variant="primary"
            size="sm"
            onClick={handleSave}
            isLoading={isLoading}
            hotkey="Ctrl+Enter"
          >
            Create Bundle
          </Button>
        </>
      }
    >
      <div className="flex flex-col gap-4 py-1">
        {/* Dynamic Self-Learning Suggestions */}
        {suggestions.length > 0 && (
          <div className="flex flex-col gap-1.5 p-2.5 rounded-lg bg-blue-50/60 dark:bg-blue-950/30 border border-blue-200/60 dark:border-blue-900/40">
            <div className="flex items-center gap-1.5 text-xs font-semibold text-blue-700 dark:text-blue-300">
              <Sparkles className="h-3.5 w-3.5" />
              <span>Suggested Packaging Tiers</span>
            </div>
            <div className="flex flex-wrap gap-1.5">
              {suggestions.map((sug, idx) => {
                const name = sug.bundleName;
                const mult = sug.unitsPerBundle;
                return (
                  <button
                    key={idx}
                    type="button"
                    onClick={() => handleApplySuggestion(sug)}
                    className="inline-flex items-center gap-1 px-2 py-0.5 rounded text-xs bg-white dark:bg-slate-900 border border-blue-200 dark:border-blue-800 text-blue-800 dark:text-blue-200 hover:bg-blue-100 dark:hover:bg-blue-900/60 transition-colors cursor-pointer"
                  >
                    <Package className="h-3 w-3" />
                    <span>
                      {name} ({mult} {baseUnit})
                    </span>
                  </button>
                );
              })}
            </div>
          </div>
        )}

        <div className="grid grid-cols-2 gap-3">
          <TextInput
            label="Bundle Name *"
            placeholder="e.g. Box, Dozen, Pack"
            value={bundleName}
            onChange={(e) => setBundleName(e.target.value)}
            autoFocus
          />

          <NumericInput
            label={`Multiplier (${baseUnit} per Bundle) *`}
            value={multiplier}
            onChange={setMultiplier}
            min={1}
            allowDecimals={false}
          />
        </div>

        <div className="grid grid-cols-2 gap-3">
          <NumericInput
            label="Bundle Sale Price (₹)"
            value={salePrice}
            onChange={setSalePrice}
            min={0}
          />

          <NumericInput
            label="Bundle MRP (₹)"
            value={mrp}
            onChange={setMrp}
            min={0}
          />
        </div>

        <TextInput
          label="Outer Package Barcode"
          placeholder="Scan outer package barcode (optional)"
          value={barcode}
          onChange={(e) => setBarcode(e.target.value)}
        />

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
