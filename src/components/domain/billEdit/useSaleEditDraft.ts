import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { api, errorMessage } from '@/api';
import { saleLineToCart } from '@/lib/billEditDraft';
import { toChargeInputs, validChargeAmount, type BillChargeLine } from '@/lib/billMath';
import type {
  CreateSalePayload,
  ItemBundleSearchResult,
  ItemSearchResult,
  PaymentStatus,
  PreviewSalePayload,
  SettlementsToMove,
} from '@/types';
import type { DraftLoad } from './usePurchaseEditDraft';

/**
 * A sale line of the edit form. `quantity` and `rate` are per selected unit (bundle or base unit).
 * This is local state: the sale cart store (a sale in progress on the billing screen) is never touched.
 */
export interface EditSaleLine {
  key: number;
  itemId: number;
  name: string;
  baseUnit: string;
  bundleId: number | null;
  unitsPerBundle: number;
  quantity: number;
  /** Null until the user types one (never for a loaded line: it comes from the stored total). */
  rate: number | null;
  /** The item's sale price per base unit, to suggest a rate when the unit changes (null = unknown). */
  defaultSalePrice: number | null;
  /** The item's bundles, for the unit select. */
  bundles: ItemBundleSearchResult[];
}

export interface SaleDraft {
  code: string;
  date: string;
  partyId: number | null;
  notes: string;
  lines: EditSaleLine[];
  charges: BillChargeLine[];
  paymentStatus: PaymentStatus;
  paidInput: number;
}

type HeaderPatch = Partial<Pick<SaleDraft, 'date' | 'partyId' | 'notes' | 'paymentStatus' | 'paidInput'>>;

function snapshot(d: SaleDraft): string {
  return JSON.stringify([
    d.date,
    d.partyId,
    d.notes,
    d.lines.map((l) => [l.itemId, l.bundleId, l.quantity, l.rate]),
    d.charges.map((c) => [c.chargeTypeId, c.amount]),
    d.paymentStatus,
    d.paidInput,
  ]);
}

/** Suggested rate when a unit is chosen: the bundle's backend-suggested rate, else the item's sale price. */
function suggestedRate(item: { defaultSalePrice: number | null }, bundle: ItemBundleSearchResult | null): number | null {
  return bundle ? bundle.defaultRate : item.defaultSalePrice;
}

/** Sale twin of `usePurchaseEditDraft` (FR-20 / DD-57): same StrictMode-safe load, local draft, preview-backed. */
export function useSaleEditDraft(billId: number) {
  const [load, setLoad] = useState<DraftLoad>({ status: 'loading' });
  const [draft, setDraft] = useState<SaleDraft | null>(null);
  const [baseline, setBaseline] = useState('');
  const [applied, setApplied] = useState(0);
  // DD-88: the bill's later payments a party change would move (confirm dialog text).
  const [settlementsToMove, setSettlementsToMove] = useState<SettlementsToMove>({ count: 0, amount: 0 });
  const seq = useRef(1);

  useEffect(() => {
    let live = true;
    setLoad({ status: 'loading' });
    setDraft(null);
    (async () => {
      try {
        const [d, g] = await Promise.all([api.getSaleDetails(billId), api.getBillEditGuard('SALE', billId)]);
        if (!live) return;
        if (!g.canEdit) {
          setLoad({
            status: 'error',
            message: g.reasons.find((r) => r.blocksEdit)?.message ?? 'This bill cannot be edited',
          });
          return;
        }
        const found = await Promise.all(
          d.items.map((it) =>
            api
              .searchItemsFts(it.saleItem.itemName, 8)
              .then((rows) => rows.find((r) => r.itemId === it.saleItem.itemId) ?? null)
              .catch(() => null)
          )
        );
        if (!live) return;
        const lines: EditSaleLine[] = d.items.map((it, i) => {
          const c = saleLineToCart(it.saleItem, found[i]);
          return {
            key: seq.current++,
            itemId: c.itemId,
            name: c.name,
            baseUnit: c.baseUnit,
            bundleId: c.bundleId ?? null,
            unitsPerBundle: c.unitsPerBundle,
            quantity: c.quantity,
            rate: c.rate,
            defaultSalePrice: c.defaultSalePrice ?? null,
            bundles: found[i]?.bundles ?? [],
          };
        });
        const s = d.sale;
        const up = g.upfrontAmount;
        const next: SaleDraft = {
          code: s.transactionCode,
          date: s.transactionDate,
          partyId: s.partyId,
          notes: s.notes ?? '',
          lines,
          charges: d.charges.map((c) => ({
            key: seq.current++,
            chargeTypeId: c.chargeTypeId,
            name: c.chargeName,
            effect: c.effect,
            amount: c.amount,
          })),
          // Only seeds the control from what is stored; the backend preview judges the result.
          paymentStatus: s.paymentStatus === 'PAID' ? 'PAID' : up > 0 ? 'PARTIAL' : 'UNPAID',
          paidInput: up,
        };
        setDraft(next);
        setBaseline(snapshot(next));
        setApplied(g.appliedAmount);
        setSettlementsToMove(g.settlementsToMove ?? { count: 0, amount: 0 });
        setLoad({ status: 'ready' });
      } catch (e) {
        if (live) setLoad({ status: 'error', message: errorMessage(e) });
      }
    })();
    return () => {
      live = false;
    };
  }, [billId]);

  const patch = useCallback((fn: (d: SaleDraft) => SaleDraft) => setDraft((d) => (d ? fn(d) : d)), []);

  const setHeader = useCallback((p: HeaderPatch) => patch((d) => ({ ...d, ...p })), [patch]);

  const setLine = useCallback(
    (key: number, p: Partial<Pick<EditSaleLine, 'quantity' | 'rate'>>) =>
      patch((d) => ({ ...d, lines: d.lines.map((l) => (l.key === key ? { ...l, ...p } : l)) })),
    [patch]
  );

  /** Switch a line's unit (`null` = base unit); the rate becomes the suggested one for that unit. */
  const setUnit = useCallback(
    (key: number, bundleId: number | null) =>
      patch((d) => ({
        ...d,
        lines: d.lines.map((l) => {
          if (l.key !== key) return l;
          const b = bundleId === null ? null : (l.bundles.find((x) => x.bundleId === bundleId) ?? null);
          return {
            ...l,
            bundleId: b ? b.bundleId : null,
            unitsPerBundle: b ? b.unitsPerBundle : 1,
            rate: suggestedRate(l, b) ?? l.rate,
          };
        }),
      })),
    [patch]
  );

  /** Makes a freshly created bundle available in the unit list of every line of that item. */
  const addBundle = useCallback(
    (itemId: number, bundle: ItemBundleSearchResult) =>
      patch((d) => ({
        ...d,
        lines: d.lines.map((l) =>
          l.itemId === itemId ? { ...l, bundles: [...l.bundles.filter((x) => x.bundleId !== bundle.bundleId), bundle] } : l
        ),
      })),
    [patch]
  );

  /** Appends a new line at the item's suggested rate; returns its row key so the form can focus it. */
  const addLine = useCallback(
    (item: ItemSearchResult): number => {
      const key = seq.current++;
      const b =
        item.matchedBundleId != null ? (item.bundles.find((x) => x.bundleId === item.matchedBundleId) ?? null) : null;
      patch((d) => ({
        ...d,
        lines: [
          ...d.lines,
          {
            key,
            itemId: item.itemId,
            name: item.name,
            baseUnit: item.baseUnit,
            bundleId: b ? b.bundleId : null,
            unitsPerBundle: b ? b.unitsPerBundle : 1,
            quantity: 1,
            rate: suggestedRate(item, b),
            defaultSalePrice: item.defaultSalePrice,
            bundles: item.bundles,
          },
        ],
      }));
      return key;
    },
    [patch]
  );

  const removeLine = useCallback(
    (key: number) => patch((d) => ({ ...d, lines: d.lines.filter((l) => l.key !== key) })),
    [patch]
  );

  const addCharge = useCallback(
    (line: Omit<BillChargeLine, 'key'>): boolean => {
      if (!validChargeAmount(line.amount)) return false;
      const key = seq.current++;
      patch((d) => ({ ...d, charges: [...d.charges, { ...line, key }] }));
      return true;
    },
    [patch]
  );
  const updateChargeAmount = useCallback(
    (key: number, amount: number): boolean => {
      if (!validChargeAmount(amount)) return false;
      patch((d) => ({ ...d, charges: d.charges.map((c) => (c.key === key ? { ...c, amount } : c)) }));
      return true;
    },
    [patch]
  );
  const removeCharge = useCallback(
    (key: number) => patch((d) => ({ ...d, charges: d.charges.filter((c) => c.key !== key) })),
    [patch]
  );

  const dirty = useMemo(() => draft !== null && snapshot(draft) !== baseline, [draft, baseline]);

  /** Format check only (a number is missing); every other rule (stock, party, payment) is the backend's. */
  const incomplete = useMemo(
    () => !draft || draft.lines.length === 0 || draft.lines.some((l) => l.rate === null || !(l.quantity > 0)),
    [draft]
  );

  const bill = useMemo<PreviewSalePayload | null>(
    () =>
      !draft || incomplete
        ? null
        : {
            transactionDate: draft.date,
            partyId: draft.partyId,
            paymentMode: 'CASH',
            charges: toChargeInputs(draft.charges),
            notes: draft.notes.trim() || null,
            items: draft.lines.map((l) => ({
              itemId: l.itemId,
              bundleId: l.bundleId,
              bundleQuantity: l.quantity,
              unitPrice: l.rate as number,
            })),
          },
    [draft, incomplete]
  );

  const toPayload = useCallback(
    (paidAmount: number): CreateSalePayload | null =>
      bill && draft ? { ...bill, paymentStatus: draft.paymentStatus, paidAmount } : null,
    [bill, draft]
  );

  return {
    load,
    draft,
    dirty,
    incomplete,
    applied,
    settlementsToMove,
    bill,
    toPayload,
    setHeader,
    setLine,
    setUnit,
    addBundle,
    addLine,
    removeLine,
    addCharge,
    updateChargeAmount,
    removeCharge,
  };
}

export type SaleEditDraftApi = ReturnType<typeof useSaleEditDraft>;
