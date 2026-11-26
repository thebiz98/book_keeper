import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { api, errorMessage } from '@/api';
import { purchaseLineToDraft } from '@/lib/billEditDraft';
import { toChargeInputs, validChargeAmount, type BillChargeLine } from '@/lib/billMath';
import type {
  CreatePurchasePayload,
  ItemBundleSearchResult,
  ItemSearchResult,
  PaymentStatus,
  PreviewPurchasePayload,
  SettlementsToMove,
} from '@/types';
import type { PurchaseLine } from '../purchaseMath';

/** A purchase line of the edit form. `unitCost` is null until the user types one (new lines). */
export interface EditPurchaseLine extends Omit<PurchaseLine, 'unitCost'> {
  unitCost: number | null;
  /** The item's bundles, for the unit select. */
  bundles: ItemBundleSearchResult[];
}

export interface PurchaseDraft {
  code: string;
  date: string;
  partyId: number | null;
  supplierInvoiceRef: string;
  notes: string;
  lines: EditPurchaseLine[];
  charges: BillChargeLine[];
  paymentStatus: PaymentStatus;
  paidInput: number;
}

export type DraftLoad = { status: 'loading' } | { status: 'ready' } | { status: 'error'; message: string };

type HeaderPatch = Partial<
  Pick<PurchaseDraft, 'date' | 'partyId' | 'supplierInvoiceRef' | 'notes' | 'paymentStatus' | 'paidInput'>
>;

/** What counts as a change: the editable fields only (row keys and the bundle lists do not). */
function snapshot(d: PurchaseDraft): string {
  return JSON.stringify([
    d.date,
    d.partyId,
    d.supplierInvoiceRef,
    d.notes,
    d.lines.map((l) => [l.itemId, l.bundleId, l.quantity, l.unitCost, l.purchaseItemId ?? null]),
    d.charges.map((c) => [c.chargeTypeId, c.amount]),
    d.paymentStatus,
    d.paidInput,
  ]);
}

/**
 * Loads a purchase into a local edit draft (FR-20 / DD-57) and exposes the form actions.
 * Nothing here decides money: totals, payment and errors come from the backend preview (DD-56).
 * Every effect run owns its `live` flag, so a StrictMode double mount still ends with the loaded bill.
 */
export function usePurchaseEditDraft(billId: number) {
  const [load, setLoad] = useState<DraftLoad>({ status: 'loading' });
  const [draft, setDraft] = useState<PurchaseDraft | null>(null);
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
        const [d, g] = await Promise.all([api.getPurchaseDetails(billId), api.getBillEditGuard('PURCHASE', billId)]);
        if (!live) return;
        if (!g.canEdit) {
          setLoad({
            status: 'error',
            message: g.reasons.find((r) => r.blocksEdit)?.message ?? 'This purchase cannot be edited',
          });
          return;
        }
        const found = await Promise.all(
          d.items.map((it) =>
            api
              .searchItemsFts(it.purchaseItem.itemName, 8)
              .then((rows) => rows.find((r) => r.itemId === it.purchaseItem.itemId) ?? null)
              .catch(() => null)
          )
        );
        if (!live) return;
        const lines: EditPurchaseLine[] = d.items.map((it, i) => {
          const lot = it.purchaseItem;
          const lim = g.lotLimits.find((l) => l.purchaseItemId === lot.id);
          return {
            ...purchaseLineToDraft(lot, found[i]),
            key: seq.current++,
            minBaseQty: lim?.consumedQuantity ?? 0,
            locked: lim?.referenced ?? false,
            costLocked: lim?.vendorReturned ?? false,
            bundles: found[i]?.bundles ?? [],
          };
        });
        const p = d.purchase;
        const up = g.upfrontAmount;
        const next: PurchaseDraft = {
          code: p.transactionCode,
          date: p.transactionDate,
          partyId: p.partyId,
          supplierInvoiceRef: p.supplierInvoiceRef ?? '',
          notes: p.notes ?? '',
          lines,
          charges: d.charges.map((c) => ({
            key: seq.current++,
            chargeTypeId: c.chargeTypeId,
            name: c.chargeName,
            effect: c.effect,
            amount: c.amount,
          })),
          // Only seeds the control from what is stored; the backend preview judges the result.
          paymentStatus: p.paymentStatus === 'PAID' ? 'PAID' : up > 0 ? 'PARTIAL' : 'UNPAID',
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

  const patch = useCallback((fn: (d: PurchaseDraft) => PurchaseDraft) => setDraft((d) => (d ? fn(d) : d)), []);

  const setHeader = useCallback((p: HeaderPatch) => patch((d) => ({ ...d, ...p })), [patch]);

  const setLine = useCallback(
    (key: number, p: Partial<Pick<EditPurchaseLine, 'quantity' | 'unitCost'>>) =>
      patch((d) => ({ ...d, lines: d.lines.map((l) => (l.key === key ? { ...l, ...p } : l)) })),
    [patch]
  );

  /** Switch a line's unit: `null` = the base unit. The typed quantity and price stay as they are. */
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
            packLabel: b ? b.bundleName : l.baseUnit,
            unitsPerBundle: b ? b.unitsPerBundle : 1,
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

  /** Appends a new line (no lot id); returns its row key so the form can focus it. */
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
            itemName: item.name,
            baseUnit: item.baseUnit,
            bundleId: b ? b.bundleId : null,
            packLabel: b ? b.bundleName : item.baseUnit,
            unitsPerBundle: b ? b.unitsPerBundle : 1,
            quantity: 1,
            unitCost: null,
            purchaseItemId: null,
            bundles: item.bundles,
          },
        ],
      }));
      return key;
    },
    [patch]
  );

  /** A referenced lot (sold or moved) stays; the guard's `referenced` flag says which. */
  const removeLine = useCallback(
    (key: number) => patch((d) => ({ ...d, lines: d.lines.filter((l) => l.key !== key || l.locked) })),
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

  /** Format check only (a number is missing); every other rule is the backend's. */
  const incomplete = useMemo(
    () => !draft || draft.lines.length === 0 || draft.lines.some((l) => l.unitCost === null || !(l.quantity > 0)),
    [draft]
  );

  /** The bill without a payment: what the previews are asked about. `null` while the form is incomplete. */
  const bill = useMemo<PreviewPurchasePayload | null>(
    () =>
      !draft || incomplete
        ? null
        : {
            transactionDate: draft.date,
            partyId: draft.partyId,
            supplierInvoiceRef: draft.supplierInvoiceRef.trim() || null,
            paymentMode: 'CASH',
            charges: toChargeInputs(draft.charges),
            notes: draft.notes.trim() || null,
            items: draft.lines.map((l) => ({
              itemId: l.itemId,
              bundleId: l.bundleId,
              bundleQuantity: l.quantity,
              unitCost: l.unitCost as number,
              purchaseItemId: l.purchaseItemId ?? null,
            })),
          },
    [draft, incomplete]
  );

  /** The `updatePurchase` payload: the bill plus the chosen payment (`paidAmount` comes from the preview for PAID). */
  const toPayload = useCallback(
    (paidAmount: number): CreatePurchasePayload | null =>
      bill && draft ? { ...bill, paymentStatus: draft.paymentStatus, paidAmount } : null,
    [bill, draft]
  );

  return {
    load,
    draft,
    dirty,
    incomplete,
    /** Settlements and return credits that stay on the bill ("Already settled" hint). */
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

export type PurchaseEditDraftApi = ReturnType<typeof usePurchaseEditDraft>;
