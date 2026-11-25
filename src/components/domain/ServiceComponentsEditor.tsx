import React, { useState } from 'react';
import { Trash2 } from 'lucide-react';
import { api } from '@/api';
import { usePreview } from '@/hooks/usePreview';
import type { ItemSearchResult, ServiceComponentInput, ServiceCostPreview } from '@/types';
import { ItemSearchTypeahead } from './ItemSearchTypeahead';

/** One editable row of a service's component list (FR-36). `qty` is the raw text the user typed. */
export interface ComponentRow {
  itemId: number;
  name: string;
  baseUnit: string;
  qty: string;
}

/** Rows -> backend payload. Rows with an unparsable quantity are sent as 0 so the backend rejects them. */
export function toComponentInputs(rows: ComponentRow[]): ServiceComponentInput[] {
  return rows.map((r) => {
    const q = parseFloat(r.qty);
    return { componentItemId: r.itemId, quantityPerUnit: Number.isFinite(q) ? q : 0 };
  });
}

export interface ServiceComponentsEditorProps {
  rows: ComponentRow[];
  onChange: (rows: ComponentRow[]) => void;
  /** Raw Service cost text (for the cost preview line). */
  serviceCost: string;
  /** The service being edited (cannot be its own component). */
  serviceItemId?: number;
}

const money = (n: number) => `₹${n.toFixed(2)}`;

/**
 * FR-36: the Components section of the Service drawer. Rows are item + quantity per unit; the cost
 * line "Components ₹x · Misc ₹y · Cost per unit ₹z" is the backend's `preview_service_cost`
 * (DD-56), shown as returned.
 */
export const ServiceComponentsEditor: React.FC<ServiceComponentsEditorProps> = ({
  rows,
  onChange,
  serviceCost,
  serviceItemId,
}) => {
  const [hint, setHint] = useState<string | null>(null);

  const cost = parseFloat(serviceCost);
  const preview = usePreview<{ components: ServiceComponentInput[]; serviceCost: number | null }, ServiceCostPreview>({
    input: {
      components: toComponentInputs(rows).filter((c) => c.quantityPerUnit > 0),
      serviceCost: Number.isFinite(cost) && cost >= 0 ? cost : null,
    },
    fetcher: (i) => api.previewServiceCost(i),
  });
  const p = preview.data;

  const add = (it: ItemSearchResult) => {
    if (it.itemType === 'SERVICE' || it.itemId === serviceItemId) {
      setHint(it.itemType === 'SERVICE' ? 'Only physical items can be components' : 'A service cannot use itself');
      return;
    }
    if (rows.some((r) => r.itemId === it.itemId)) {
      setHint(`${it.name} is already a component`);
      return;
    }
    setHint(null);
    onChange([...rows, { itemId: it.itemId, name: it.name, baseUnit: it.baseUnit, qty: '1' }]);
  };

  return (
    <div data-testid="service-components" className="flex flex-col gap-2">
      <div className="text-xs font-semibold text-slate-600 dark:text-slate-300">Components (used per 1 unit)</div>
      {rows.length === 0 ? (
        <p data-testid="no-components" className="text-xs text-slate-400">
          No components: pure labour. Add stock items the service uses up.
        </p>
      ) : (
        <ul className="flex flex-col gap-1.5">
          {rows.map((r) => (
            <li key={r.itemId} data-testid="component-row" className="flex items-center gap-2 text-sm">
              <span className="flex-1 truncate font-medium">{r.name}</span>
              <input
                aria-label={`Quantity of ${r.name} per unit`}
                data-testid="component-qty"
                type="number"
                min={0}
                step="any"
                value={r.qty}
                onChange={(e) => onChange(rows.map((x) => (x.itemId === r.itemId ? { ...x, qty: e.target.value } : x)))}
                className="w-24 rounded-md border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-900 px-2 py-1 text-right font-mono text-sm"
              />
              <span className="w-14 text-xs text-slate-500">{r.baseUnit}</span>
              <button
                type="button"
                aria-label={`Remove ${r.name}`}
                onClick={() => onChange(rows.filter((x) => x.itemId !== r.itemId))}
                className="p-1 rounded text-slate-500 hover:text-rose-600 hover:bg-slate-100 dark:hover:bg-slate-800 cursor-pointer"
              >
                <Trash2 className="h-3.5 w-3.5" />
              </button>
            </li>
          ))}
        </ul>
      )}
      <ItemSearchTypeahead
        onSelectItem={add}
        physicalOnly
        placeholder="+ Add component (search a stock item)..."
        className="mt-1"
      />
      {hint && (
        <p role="status" data-testid="component-hint" className="text-xs text-amber-600">
          {hint}
        </p>
      )}
      <p data-testid="service-cost-preview" className="text-xs font-mono text-slate-600 dark:text-slate-300">
        {p
          ? `Components ${money(p.componentsCost)} · Misc ${money(p.miscCost)} · Cost per unit ${money(p.unitCost)}`
          : 'Components — · Misc — · Cost per unit —'}
      </p>
    </div>
  );
};

export default ServiceComponentsEditor;
