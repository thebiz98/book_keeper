import React from 'react';
import { cn } from '@/lib/utils';
import type { LotDraw, LotDrawLot, LotSource } from '@/types';
import { CurrencyDisplay } from '../common/CurrencyDisplay';

export interface LotDrawLineProps {
  draw: LotDraw | null;
  baseUnit: string;
  className?: string;
}

const MAX_ROWS = 5;
const fmtQty = (n: number) => String(Number(n.toFixed(3)));
const fmtMoney = (n: number) => `₹${n.toFixed(2)}`;

const SOURCE_TAG: Partial<Record<LotSource, string>> = {
  OPENING: 'opening',
  ADJUSTMENT: 'unrecorded',
  RETURN: 'return',
};

const lotText = (l: LotDrawLot, baseUnit: string) =>
  `${l.transactionCode} ${fmtMoney(l.unitCost)} × ${fmtQty(l.remainingQuantity)} ${baseUnit} left`;

/**
 * FR-31 / FR-48: the open lots' purchase prices on the sale entry row, one row per lot (oldest
 * first, as FIFO draws them), with the part the entered quantity would use. Everything shown comes
 * from `preview_lot_draw`; this only formats it.
 */
export const LotDrawLine: React.FC<LotDrawLineProps> = ({ draw, baseUnit, className }) => {
  if (!draw || draw.lots.length === 0) return null;
  const shown = draw.lots.slice(0, MAX_ROWS);
  const hidden = draw.lots.slice(MAX_ROWS);
  const used = draw.lots.some((l) => l.drawQuantity > 0);

  return (
    <div data-testid="lot-draw-line" className={cn('flex flex-col gap-0.5 text-xs font-mono', className)}>
      <span className="font-sans font-semibold text-slate-500 dark:text-slate-400">Purchase price per lot</span>
      <ul className="flex flex-col gap-0.5">
        {shown.map((l) => {
          const drawn = l.drawQuantity > 0;
          const tag = SOURCE_TAG[l.source];
          return (
            <li
              key={l.purchaseItemId}
              data-testid="lot-row"
              data-drawn={drawn ? 'true' : 'false'}
              className={cn(
                'grid grid-cols-[8rem_6rem_9rem_auto] items-baseline gap-x-2 tabular-nums rounded px-1',
                drawn && 'font-bold text-blue-700 dark:text-blue-300 bg-blue-50 dark:bg-blue-950/50'
              )}
            >
              <span className="truncate" title={l.transactionCode}>
                {l.transactionCode}
                {tag && <span className="ml-1 font-sans font-normal text-slate-500 dark:text-slate-400">({tag})</span>}
              </span>
              <span className="text-right">{fmtMoney(l.unitCost)}</span>
              <span>
                × {fmtQty(l.remainingQuantity)} {baseUnit} left
              </span>
              <span data-testid="lot-uses">{drawn ? `uses ${fmtQty(l.drawQuantity)}` : ''}</span>
            </li>
          );
        })}
      </ul>
      {hidden.length > 0 && (
        <span
          data-testid="lot-more"
          title={hidden.map((l) => lotText(l, baseUnit)).join(' · ')}
          className="px-1 text-slate-500 dark:text-slate-400 cursor-help"
        >
          +{hidden.length} more
        </span>
      )}
      {(used || draw.shortfall > 0) && (
        <span className="flex flex-wrap gap-x-2 px-1">
          {used && (
            <span data-testid="lot-draw-cost" className="tabular-nums">
              → cost <CurrencyDisplay amount={draw.drawCost} size="sm" /> for this qty
            </span>
          )}
          {draw.shortfall > 0 && (
            <span data-testid="lot-shortfall" className="font-semibold text-rose-600 dark:text-rose-400 tabular-nums">
              short by {fmtQty(draw.shortfall)}
            </span>
          )}
        </span>
      )}
    </div>
  );
};

export interface ServiceDrawLineProps {
  draw: LotDraw;
  className?: string;
}

/**
 * FR-37: what a service line would consume and cost, straight from `preview_lot_draw`: each
 * component's draw, the components' cost, the misc. cost and the line cost. Formatting only.
 */
export const ServiceDrawLine: React.FC<ServiceDrawLineProps> = ({ draw, className }) => {
  const comps = draw.components ?? [];
  if (comps.length === 0) return null;
  return (
    <span data-testid="service-draw-line" className={cn('inline-flex flex-wrap items-center gap-x-2', className)}>
      <span>Uses:</span>
      {comps.map((c) => (
        <span key={c.itemId} data-testid="service-component" className="tabular-nums">
          {fmtQty(c.quantity)} {c.baseUnit} {c.name}
          {c.shortfall > 0 && (
            <span className="ml-1 font-semibold text-rose-600 dark:text-rose-400">(short by {fmtQty(c.shortfall)})</span>
          )}
        </span>
      ))}
      <span aria-hidden>·</span>
      <span data-testid="service-components-cost" className="tabular-nums">
        Components {fmtMoney(draw.componentsCost ?? 0)}
      </span>
      <span data-testid="service-misc-cost" className="tabular-nums">
        Misc {fmtMoney(draw.miscCost ?? 0)}
      </span>
      <span data-testid="service-line-cost" className="tabular-nums font-semibold">
        Cost {fmtMoney(draw.lineCost ?? 0)}
      </span>
      {draw.shortfall > 0 && (
        <span data-testid="service-shortfall" className="font-semibold text-rose-600 dark:text-rose-400 tabular-nums">
          short by {fmtQty(draw.shortfall)}
        </span>
      )}
    </span>
  );
};

export default LotDrawLine;
