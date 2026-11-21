import React, { useState, useEffect } from 'react';
import { api } from '@/api';
import type { AdjustmentReason } from '@/types';
import { Modal } from '../ui/Modal';
import { Button } from '../ui/Button';
import { NumericInput } from '../ui/NumericInput';
import { Select } from '../ui/Select';

export interface StockAdjustmentModalProps {
  isOpen: boolean;
  onClose: () => void;
  itemId?: number;
  itemName?: string;
  currentStock?: number;
  baseUnit?: string;
  onAdjusted?: () => void;
}

export const StockAdjustmentModal: React.FC<StockAdjustmentModalProps> = ({
  isOpen,
  onClose,
  itemId,
  itemName = 'Item',
  currentStock = 0,
  baseUnit = 'PCS',
  onAdjusted,
}) => {
  const [adjustmentType, setAdjustmentType] = useState<'DECREASE' | 'INCREASE'>('DECREASE');
  const [quantity, setQuantity] = useState<number>(1);
  const [reason, setReason] = useState<AdjustmentReason>('Damaged');
  const [notes, setNotes] = useState<string>('');
  const [isLoading, setIsLoading] = useState<boolean>(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (isOpen) {
      setAdjustmentType('DECREASE');
      setQuantity(1);
      setReason('Damaged');
      setNotes('');
      setError(null);
    }
  }, [isOpen]);

  const delta = adjustmentType === 'DECREASE' ? -quantity : quantity;
  const resultingStock = currentStock + delta;

  const handleAdjust = async () => {
    if (!itemId) {
      setError('Item ID is required for stock adjustment');
      return;
    }
    if (quantity <= 0) {
      setError('Adjustment quantity must be greater than 0');
      return;
    }
    if (resultingStock < 0) {
      setError(`Resulting stock cannot be negative (Current: ${currentStock} ${baseUnit})`);
      return;
    }

    setIsLoading(true);
    setError(null);

    try {
      await api.createStockAdjustment({
        itemId,
        adjustmentDate: new Date().toISOString().split('T')[0],
        quantity: delta,
        reason,
        notes: notes.trim() || null,
      });
      onAdjusted?.();
      onClose();
    } catch (err: any) {
      setError(err?.message || 'Failed to record stock adjustment');
    } finally {
      setIsLoading(false);
    }
  };

  return (
    <Modal
      isOpen={isOpen}
      onClose={onClose}
      title="Adjust Physical Inventory Stock"
      description={`Record scrap, damage, or cycle count correction for ${itemName}`}
      size="md"
      footer={
        <>
          <Button variant="outline" size="sm" onClick={onClose} hotkey="Esc">
            Cancel
          </Button>
          <Button
            variant={adjustmentType === 'DECREASE' ? 'danger' : 'primary'}
            size="sm"
            onClick={handleAdjust}
            isLoading={isLoading}
            hotkey="Ctrl+Enter"
          >
            Apply Adjustment
          </Button>
        </>
      }
    >
      <div className="flex flex-col gap-4 py-1">
        {/* Current vs Resulting Stock Preview */}
        <div className="flex items-center justify-between p-3 rounded-lg bg-slate-50 dark:bg-slate-800/60 border border-slate-200 dark:border-slate-800">
          <div>
            <span className="text-xs text-slate-500 dark:text-slate-400">Current Stock</span>
            <div className="text-lg font-bold font-mono text-slate-900 dark:text-slate-100">
              {currentStock} {baseUnit}
            </div>
          </div>
          <div className="text-right">
            <span className="text-xs text-slate-500 dark:text-slate-400">New Physical Stock</span>
            <div
              className={`text-lg font-bold font-mono ${
                resultingStock < 0
                  ? 'text-rose-600'
                  : resultingStock < 5
                  ? 'text-amber-600'
                  : 'text-emerald-600'
              }`}
            >
              {resultingStock} {baseUnit}
            </div>
          </div>
        </div>

        <div className="grid grid-cols-2 gap-3">
          <Select
            label="Adjustment Type"
            value={adjustmentType}
            onChange={(val) => setAdjustmentType(val as 'DECREASE' | 'INCREASE')}
            options={[
              { value: 'DECREASE', label: 'Write-Off / Reduce (-)' },
              { value: 'INCREASE', label: 'Surplus / Increase (+)' },
            ]}
          />

          <NumericInput
            label={`Adjustment Qty (${baseUnit}) *`}
            value={quantity}
            onChange={setQuantity}
            min={1}
            allowDecimals={false}
          />
        </div>

        <Select
          label="Reason Code *"
          value={reason}
          onChange={(val) => setReason(val as AdjustmentReason)}
          options={[
            { value: 'Damaged', label: 'Damaged / Defective in Store' },
            { value: 'Expired', label: 'Expired / Dried Ink' },
            { value: 'DamageWriteOff', label: 'Written off to Scrap' },
            { value: 'AuditCorrection', label: 'Cycle Count Audit Correction' },
            { value: 'Theft', label: 'Shrinkage / Pilferage' },
            { value: 'FoundStock', label: 'Found Stock / Surplus' },
          ]}
        />

        <div className="flex flex-col gap-1">
          <label className="text-xs font-medium text-slate-700 dark:text-slate-300">
            Audit Notes
          </label>
          <input
            type="text"
            placeholder="Details or investigation notes..."
            value={notes}
            onChange={(e) => setNotes(e.target.value)}
            className="w-full rounded-md border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-900 text-slate-900 dark:text-slate-100 text-sm px-3 py-1.5 focus:outline-none focus:ring-2 focus:ring-blue-500"
          />
        </div>

        {error && (
          <p role="alert" className="text-xs text-rose-600 dark:text-rose-400 font-medium">
            {error}
          </p>
        )}
      </div>
    </Modal>
  );
};

export default StockAdjustmentModal;
