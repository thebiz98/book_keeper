import React from 'react';
import { cn } from '@/lib/utils';
import { isService } from '@/lib/itemKind';
import type { ItemSearchResult } from '@/types';
import { StockStatusBadge, STOCK_BADGE_BASE, STOCK_TONE } from './StockStatusBadge';

export interface ServiceAvailabilityBadgeProps {
  /** Backend-computed units that can be made; null when the service has no components. */
  canMake?: number | null;
  baseUnit?: string;
  className?: string;
}

/** Maps the backend `canMake` to a badge; no rule is decided here. */
export const ServiceAvailabilityBadge: React.FC<ServiceAvailabilityBadgeProps> = ({
  canMake,
  baseUnit = 'PCS',
  className,
}) => {
  let label: string;
  let tone: string;
  if (canMake == null) {
    label = '—';
    tone = STOCK_TONE.neutral;
  } else if (canMake > 0) {
    label = `Can make ${canMake} ${baseUnit}`;
    tone = STOCK_TONE.ok;
  } else {
    label = "Can't make";
    tone = STOCK_TONE.out;
  }
  return (
    <span data-testid="can-make" className={cn(STOCK_BADGE_BASE, tone, className)}>
      {label}
    </span>
  );
};

export interface ItemStockStatusProps {
  item: Pick<
    ItemSearchResult,
    'itemType' | 'currentStock' | 'canMake' | 'isLowStock' | 'lowStockThreshold' | 'baseUnit'
  >;
  className?: string;
}

/** The one place that chooses between the stock badge and the service availability badge. */
export const ItemStockStatus: React.FC<ItemStockStatusProps> = ({ item, className }) =>
  isService(item) ? (
    <ServiceAvailabilityBadge canMake={item.canMake} baseUnit={item.baseUnit} className={className} />
  ) : (
    <StockStatusBadge
      currentStock={item.currentStock}
      isLowStock={item.isLowStock}
      lowStockThreshold={item.lowStockThreshold}
      baseUnit={item.baseUnit}
      className={className}
    />
  );

export default ItemStockStatus;
