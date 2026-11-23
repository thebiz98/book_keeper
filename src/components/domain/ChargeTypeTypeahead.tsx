import React from 'react';
import { api } from '@/api';
import type { ChargeType, TransactionType } from '@/types';
import { formatINR } from '@/lib/money';
import { LiveSearchTypeahead } from '../common/LiveSearchTypeahead';

export interface ChargeTypeTypeaheadProps {
  side: TransactionType;
  onSelect: (ct: ChargeType) => void;
  /** Enter on no match, or the `[+ Create "x"]` row. Receives the typed text. */
  onCreateRequest: (text: string) => void;
  /** Enter on an empty field (the panel closes). */
  onEmptyEnter?: () => void;
  inputRef?: React.MutableRefObject<HTMLInputElement | null>;
  autoFocus?: boolean;
  disabled?: boolean;
}

/**
 * Charge / discount type picker (FR-19): live FTS typeahead filtered to the bill side, with an
 * inline create row. Built on the shared `LiveSearchTypeahead` (R1).
 */
export const ChargeTypeTypeahead: React.FC<ChargeTypeTypeaheadProps> = ({
  side,
  onSelect,
  onCreateRequest,
  onEmptyEnter,
  inputRef,
  autoFocus,
  disabled,
}) => (
  <LiveSearchTypeahead<ChargeType>
    search={(q) => api.searchChargeTypes(q, side, 8)}
    getKey={(t) => t.id}
    getLabel={(t) => t.name}
    onSelect={onSelect}
    onCreate={onCreateRequest}
    createLabel={(q) => `+ Create "${q}"`}
    onEmptyEnter={onEmptyEnter}
    renderItem={(t) => (
      <div data-testid="charge-type-option" className="flex items-center justify-between gap-2 px-3 py-2 text-sm">
        <span className="flex items-center gap-2">
          <span
            className={
              t.effect === 'CHARGE'
                ? 'rounded px-1.5 text-xs font-semibold bg-emerald-100 text-emerald-700 dark:bg-emerald-950 dark:text-emerald-300'
                : 'rounded px-1.5 text-xs font-semibold bg-rose-100 text-rose-700 dark:bg-rose-950 dark:text-rose-300'
            }
          >
            {t.effect === 'CHARGE' ? '+ Charge' : '− Discount'}
          </span>
          {t.name}
        </span>
        {t.defaultAmount != null && <span className="font-mono text-xs text-slate-500">{formatINR(t.defaultAmount)}</span>}
      </div>
    )}
    ariaLabel="Charge type"
    testId="charge-type-input"
    placeholder="Type a name, e.g. Transport or Cash Discount, then Enter to create"
    autoFocus={autoFocus}
    inputRef={inputRef}
    disabled={disabled}
  />
);

export default ChargeTypeTypeahead;
