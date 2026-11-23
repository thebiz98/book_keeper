import React, { useMemo } from 'react';
import { Modal } from '../ui/Modal';
import { StockAdjustmentForm, type AdjustmentItem } from './StockAdjustmentForm';

export interface StockAdjustmentModalProps {
  isOpen: boolean;
  onClose: () => void;
  /** Optional: when omitted the modal offers an item search. */
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
  const item: AdjustmentItem | null = useMemo(
    () => (itemId ? { itemId, name: itemName, currentStock, baseUnit } : null),
    [itemId, itemName, currentStock, baseUnit]
  );
  return (
    <Modal
      isOpen={isOpen}
      onClose={onClose}
      title="Stock Adjustment"
      description="Write off damaged, expired or lost stock, correct a count, or add unrecorded stock"
      size="md"
    >
      <StockAdjustmentForm item={item} onAdjusted={() => onAdjusted?.()} onCancel={onClose} />
    </Modal>
  );
};

export default StockAdjustmentModal;
