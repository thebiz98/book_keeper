import React, { forwardRef, useCallback, useEffect, useImperativeHandle, useRef, useState } from 'react';
import { Plus, X } from 'lucide-react';
import type { ChargeType, TransactionType } from '@/types';
import { useEscapeLayer } from '@/hooks/useHotkeys';
import { validChargeAmount, type BillChargeLine } from '@/lib/billMath';
import { cn } from '@/lib/utils';
import { Button } from '../ui/Button';
import { TextInput } from '../ui/TextInput';
import { ChargeTypeTypeahead } from './ChargeTypeTypeahead';
import { ChargeTypeQuickCreateModal } from './ChargeTypeQuickCreateModal';

export interface BillChargesPanelHandle {
  /** Opens the entry row and focuses the Charge Type field (Alt+X). */
  open: () => void;
}

export interface BillChargesPanelProps {
  side: TransactionType;
  lines: BillChargeLine[];
  onAdd: (line: Omit<BillChargeLine, 'key'>) => boolean;
  onUpdateAmount: (key: number, amount: number) => boolean;
  onRemove: (key: number) => void;
  /** After a line was added or the row was dismissed: the page returns focus to item search. */
  onDone: () => void;
  /** True while the quick-create modal is open (pages that run their own key listener pause it). */
  onModalOpenChange?: (open: boolean) => void;
}

const Badge: React.FC<{ effect: 'CHARGE' | 'DISCOUNT' }> = ({ effect }) => (
  <span
    className={cn(
      'rounded px-1.5 text-xs font-semibold',
      effect === 'CHARGE'
        ? 'bg-emerald-100 text-emerald-700 dark:bg-emerald-950 dark:text-emerald-300'
        : 'bg-rose-100 text-rose-700 dark:bg-rose-950 dark:text-rose-300'
    )}
  >
    {effect === 'CHARGE' ? '+' : '−'}
  </span>
);

/**
 * "Charges & Discounts" strip of the sale and purchase screens (FR-19). Alt+X opens the entry row:
 * type (live search / inline create) -> amount (prefilled with the default) -> Enter adds the line.
 */
export const BillChargesPanel = forwardRef<BillChargesPanelHandle, BillChargesPanelProps>(
  ({ side, lines, onAdd, onUpdateAmount, onRemove, onDone, onModalOpenChange }, ref) => {
    const [open, setOpen] = useState(false);
    const [selected, setSelected] = useState<ChargeType | null>(null);
    const [amount, setAmount] = useState('');
    const [createText, setCreateText] = useState<string | null>(null);
    const [error, setError] = useState<string | null>(null);
    const typeRef = useRef<HTMLInputElement | null>(null);
    const amountRef = useRef<HTMLInputElement | null>(null);

    const setModal = useCallback(
      (text: string | null) => {
        setCreateText(text);
        onModalOpenChange?.(text !== null);
      },
      [onModalOpenChange]
    );

    const reset = () => {
      setSelected(null);
      setAmount('');
      setError(null);
    };
    const dismiss = () => {
      reset();
      setOpen(false);
      onDone();
    };

    useImperativeHandle(ref, () => ({
      open: () => {
        setOpen(true);
        setTimeout(() => (selected ? amountRef.current : typeRef.current)?.focus(), 0);
      },
    }));

    useEscapeLayer(dismiss, open && createText === null);

    const pick = (ct: ChargeType) => {
      setSelected(ct);
      setAmount(ct.defaultAmount != null ? String(ct.defaultAmount) : '');
      setError(null);
      setTimeout(() => {
        amountRef.current?.focus();
        amountRef.current?.select();
      }, 0);
    };

    const add = () => {
      if (!selected) return;
      const n = Number(amount);
      if (!validChargeAmount(n)) {
        setError('Enter an amount greater than 0');
        return;
      }
      if (onAdd({ chargeTypeId: selected.id, name: selected.name, effect: selected.effect, amount: n })) {
        reset();
        setOpen(false);
        onDone();
      }
    };

    useEffect(() => {
      if (!open) return;
      const t = setTimeout(() => typeRef.current?.focus(), 0);
      return () => clearTimeout(t);
      // Only when the row opens.
      // eslint-disable-next-line react-hooks/exhaustive-deps
    }, [open]);

    return (
      <section
        aria-label="Charges and discounts"
        data-testid="bill-charges-panel"
        className="rounded-md border border-slate-200 dark:border-slate-700 px-3 py-2 flex flex-col gap-2"
      >
        <div className="flex items-center justify-between">
          <h3 className="text-xs font-semibold uppercase tracking-wide text-slate-500">Charges &amp; Discounts</h3>
          {!open && (
            <Button
              type="button"
              variant="outline"
              size="xs"
              hotkey="Alt+X"
              data-testid="add-charge-btn"
              onClick={() => {
                setOpen(true);
              }}
            >
              <Plus className="h-3 w-3" /> Charge / Discount
            </Button>
          )}
        </div>

        {open && (
          <div className="flex flex-wrap items-start gap-2" data-testid="charge-entry-row">
            <div className="min-w-[16rem] flex-1">
              {selected ? (
                <div className="flex items-center gap-2 text-sm h-9" data-testid="charge-selected">
                  <Badge effect={selected.effect} />
                  <span className="font-medium">{selected.name}</span>
                  <button
                    type="button"
                    className="text-xs underline text-slate-500 cursor-pointer"
                    onClick={() => {
                      reset();
                      setTimeout(() => typeRef.current?.focus(), 0);
                    }}
                  >
                    change
                  </button>
                </div>
              ) : (
                <ChargeTypeTypeahead
                  side={side}
                  inputRef={typeRef}
                  autoFocus
                  disabled={createText !== null}
                  onSelect={pick}
                  onCreateRequest={(text) => setModal(text)}
                  onEmptyEnter={dismiss}
                />
              )}
            </div>
            {selected && (
              <>
                <TextInput
                  ref={amountRef}
                  aria-label="Charge amount"
                  data-testid="charge-amount"
                  type="number"
                  min={0}
                  step="any"
                  value={amount}
                  onChange={(e) => {
                    setAmount(e.target.value);
                    setError(null);
                  }}
                  onKeyDown={(e) => {
                    if (e.key === 'Enter') {
                      e.preventDefault();
                      add();
                    }
                  }}
                  error={error ?? undefined}
                  className="w-32 text-right font-mono"
                />
                <Button type="button" size="sm" variant="primary" data-testid="charge-add" onClick={add}>
                  Add
                </Button>
              </>
            )}
            <Button type="button" size="sm" variant="outline" onClick={dismiss}>
              Close
            </Button>
          </div>
        )}

        {lines.length > 0 && (
          <ul className="flex flex-col gap-1" aria-label="Added charges">
            {lines.map((l) => (
              <li
                key={l.key}
                data-testid="charge-line"
                tabIndex={0}
                onKeyDown={(e) => {
                  if (e.key === 'Delete' && e.target === e.currentTarget) {
                    e.preventDefault();
                    onRemove(l.key);
                  }
                }}
                className="flex items-center gap-2 text-sm rounded px-1 focus:outline-none focus:ring-1 focus:ring-blue-400"
              >
                <Badge effect={l.effect} />
                <span className="flex-1">{l.name}</span>
                <input
                  aria-label={`${l.name} amount`}
                  type="number"
                  min={0}
                  step="any"
                  defaultValue={l.amount}
                  key={`${l.key}-${l.amount}`}
                  className="w-28 rounded border border-slate-300 dark:border-slate-600 bg-transparent px-2 py-0.5 text-right font-mono text-sm"
                  onBlur={(e) => {
                    const n = Number(e.target.value);
                    if (n !== l.amount && !onUpdateAmount(l.key, n)) e.target.value = String(l.amount);
                  }}
                  onKeyDown={(e) => {
                    if (e.key === 'Enter') (e.target as HTMLInputElement).blur();
                  }}
                />
                <button
                  type="button"
                  aria-label={`Remove ${l.name}`}
                  className="text-slate-400 hover:text-red-600 cursor-pointer"
                  onClick={() => onRemove(l.key)}
                >
                  <X className="h-4 w-4" />
                </button>
              </li>
            ))}
          </ul>
        )}

        <ChargeTypeQuickCreateModal
          isOpen={createText !== null}
          initialName={createText ?? ''}
          side={side}
          onClose={() => {
            setModal(null);
            setTimeout(() => typeRef.current?.focus(), 0);
          }}
          onCreated={(ct) => {
            setModal(null);
            pick(ct);
          }}
        />
      </section>
    );
  }
);
BillChargesPanel.displayName = 'BillChargesPanel';

export default BillChargesPanel;
