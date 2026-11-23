import { round2 } from '@/lib/money';
import type { ItemLot } from '@/types';

/**
 * Cost of the most recently entered lot (highest `purchaseItemId`), or null when there are none.
 * Matches the backend's `get_latest_lot_cost` (`ORDER BY id DESC`, DD-09), not the lot date.
 */
export function latestLotCost(lots: ItemLot[]): number | null {
  if (lots.length === 0) return null;
  let best = lots[0];
  for (const l of lots) if (l.purchaseItemId > best.purchaseItemId) best = l;
  return best.unitCost;
}

/**
 * Preview of the FIFO cost of taking `qty` units: oldest lots first (ascending `purchaseItemId`).
 * Display only; the backend's figure is authoritative.
 */
export function estimateFifoCost(lots: ItemLot[], qty: number): number {
  let left = qty;
  let total = 0;
  const ordered = [...lots].sort((a, b) => a.purchaseItemId - b.purchaseItemId);
  for (const l of ordered) {
    if (left <= 0) break;
    if (!(l.remainingQuantity > 0)) continue;
    const take = Math.min(l.remainingQuantity, left);
    total += take * l.unitCost;
    left -= take;
  }
  return round2(total);
}
