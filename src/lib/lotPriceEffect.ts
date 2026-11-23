import type { PaymentStatus } from '@/types';
import { round2 } from '@/lib/money';
import { billTotals, type BillChargeLine } from '@/lib/billMath';

/** Same tolerance as the backend `MONEY_EPS`. */
const MONEY_EPS = 0.005;

export interface LotPriceRuleInput {
  /** Bill total after the price change. */
  newTotal: number;
  /** Σ SETTLEMENT + Σ RETURN_CREDIT of the bill. */
  applied: number;
  /** Σ UPFRONT of the bill. */
  upfrontOld: number;
  /** The bill's balance due before the edit. */
  oldBalanceDue: number;
}

export type LotPriceRuleResult = { ok: true; upfrontNew: number } | { ok: false; code: 'TOTAL_BELOW_APPLIED' };

/**
 * V4 payment rule (FR-25 / DD-51). Mirrors the Rust `bill_math::lot_price_upfront`; both are tested
 * with `src-tauri/tests/fixtures/lot_price_cases.json`.
 */
export function lotPriceUpfront(i: LotPriceRuleInput): LotPriceRuleResult {
  const open = i.newTotal - i.applied;
  if (open < -MONEY_EPS) return { ok: false, code: 'TOTAL_BELOW_APPLIED' };
  if (i.oldBalanceDue <= MONEY_EPS && i.applied <= MONEY_EPS) return { ok: true, upfrontNew: round2(i.newTotal) };
  return { ok: true, upfrontNew: round2(Math.min(i.upfrontOld, round2(Math.max(open, 0)))) };
}

export interface LotPriceEffectInput {
  /** Stored totals of the bill's OTHER lines (never rebuilt from rates). */
  otherLineTotals: number[];
  /** The edited line's new total: round2(packQuantity * price). */
  editedLineTotal: number;
  charges: Pick<BillChargeLine, 'effect' | 'amount'>[];
  /** Σ SETTLEMENT. */
  settled: number;
  /** Σ SETTLEMENT + Σ RETURN_CREDIT. */
  applied: number;
  upfrontOld: number;
  oldBalanceDue: number;
}

export type LotPriceEffect =
  | { blocked: true; code: 'TOTAL_BELOW_APPLIED'; newTotal: number; applied: number }
  | {
      blocked: false;
      newTotal: number;
      newPaid: number;
      balanceDue: number;
      status: PaymentStatus;
      upfrontNew: number;
    };

/** Preview of a PURCHASE-lot price edit: new total (stored lines + charges), cash paid, balance, status. */
export function lotPriceEffect(i: LotPriceEffectInput): LotPriceEffect {
  const subtotal = i.otherLineTotals.reduce((s, n) => s + n, 0) + i.editedLineTotal;
  const newTotal = Math.max(0, billTotals(subtotal, i.charges).grandTotal);
  const r = lotPriceUpfront({ newTotal, applied: i.applied, upfrontOld: i.upfrontOld, oldBalanceDue: i.oldBalanceDue });
  if (!r.ok) return { blocked: true, code: r.code, newTotal, applied: round2(i.applied) };
  const newPaid = round2(r.upfrontNew + i.settled);
  const credit = round2(i.applied - i.settled);
  const raw = round2(newTotal - newPaid - credit);
  const balanceDue = raw < 0 ? 0 : raw;
  const status: PaymentStatus = balanceDue <= MONEY_EPS ? 'PAID' : newPaid + credit > MONEY_EPS ? 'PARTIAL' : 'UNPAID';
  return { blocked: false, newTotal, newPaid, balanceDue, status, upfrontNew: r.upfrontNew };
}
