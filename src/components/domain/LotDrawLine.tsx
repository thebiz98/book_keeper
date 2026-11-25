import React from 'react';
import { cn } from '@/lib/utils';
import type { LotDraw } from '@/types';
import { CurrencyDisplay } from '../common/CurrencyDisplay';

export interface LotDrawLineProps {
  draw: LotDraw | null;
  baseUnit: string;
  className?: string;
}

const MAX_GROUPS = 4;
const fmtQty = (n: number) => String(Number(n.toFixed(3)));
const fmtMoney = (n: number) => `₹${n.toFixed(2)}`;

interface Group {
  unitCost: number;
  remaining: number;
  drawn: number;
}

/** Display grouping only: adjacent lots with an equal purchase price read as one price. */
function groupLots(draw: LotDraw): Group[] {
  const out: Group[] = [];
  for (const l of draw.lots) {
    const last = out[out.length - 1];
    if (last && last.unitCost === l.unitCost) {
      last.remaining += l.remainingQuantity;
      last.drawn += l.drawQuantity;
    } else {
      out.push({ unitCost: l.unitCost, remaining: l.remainingQuantity, drawn: l.drawQuantity });
    }
  }
  return out;
}

/**
 * FR-31 / DD-60: the open lots' purchase prices on the sale entry row and the part the entered
 * quantity would use. Everything shown comes from `preview_lot_draw`; this only formats it.
 */
export const LotDrawLine: React.FC<LotDrawLineProps> = ({ draw, baseUnit, className }) => {
  if (!draw || draw.lots.length === 0) return null;
  const groups = groupLots(draw);
  const shown = groups.slice(0, MAX_GROUPS);
  const hidden = groups.length - shown.length;
  const all = groups.map((g) => `${fmtMoney(g.unitCost)} × ${fmtQty(g.remaining)}`).join(' · ');
  const used = draw.lots.some((l) => l.drawQuantity > 0);

  return (
    <span data-testid="lot-draw-line" className={cn('inline-flex flex-wrap items-center gap-x-1', className)}>
      <span>Purchase price:</span>
      {shown.map((g, i) => (
        <React.Fragment key={i}>
          {i > 0 && <span aria-hidden>·</span>}
          <span
            data-testid="lot-group"
            data-drawn={g.drawn > 0 ? 'true' : 'false'}
            className={cn(
              'tabular-nums',
              g.drawn > 0 && 'font-bold text-blue-700 dark:text-blue-300 bg-blue-50 dark:bg-blue-950/50 rounded px-1'
            )}
          >
            {fmtMoney(g.unitCost)} × {fmtQty(g.remaining)}
          </span>
        </React.Fragment>
      ))}
      {hidden > 0 && (
        <span data-testid="lot-more" title={all} className="text-slate-500 dark:text-slate-400 cursor-help">
          +{hidden} more
        </span>
      )}
      <span>{baseUnit}</span>
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
