import { describe, it, expect } from 'vitest';
import { lotPriceEffect, lotPriceUpfront } from '@/lib/lotPriceEffect';
// The same table drives the Rust test `payment_rule_table`.
import cases from '../../../src-tauri/tests/fixtures/lot_price_cases.json';

describe('lotPriceUpfront (shared V4 table)', () => {
  for (const c of cases as Array<{
    name: string;
    newTotal: number;
    applied: number;
    upfrontOld: number;
    oldBalanceDue: number;
    upfrontNew?: number;
    error?: string;
  }>) {
    it(c.name, () => {
      const r = lotPriceUpfront(c);
      if (c.error) expect(r).toEqual({ ok: false, code: c.error });
      else expect(r).toEqual({ ok: true, upfrontNew: c.upfrontNew });
    });
  }
});

describe('lotPriceEffect', () => {
  const base = { charges: [], settled: 0, applied: 0, upfrontOld: 0, oldBalanceDue: 0 };

  it('cash bill: total and cash follow, stays PAID', () => {
    const e = lotPriceEffect({ ...base, otherLineTotals: [30], editedLineTotal: 120, upfrontOld: 130 });
    expect(e).toEqual({ blocked: false, newTotal: 150, newPaid: 150, balanceDue: 0, status: 'PAID', upfrontNew: 150 });
  });

  it('uses stored lines plus charges and discounts', () => {
    const e = lotPriceEffect({
      ...base,
      otherLineTotals: [50],
      editedLineTotal: 50,
      charges: [
        { effect: 'CHARGE', amount: 10 },
        { effect: 'DISCOUNT', amount: 5 },
      ],
      upfrontOld: 100,
    });
    expect(e).toMatchObject({ blocked: false, newTotal: 105, newPaid: 105 });
  });

  it('partial bill becomes PAID when the upfront covers the lower total', () => {
    const e = lotPriceEffect({ ...base, otherLineTotals: [], editedLineTotal: 30, upfrontOld: 40, oldBalanceDue: 60 });
    expect(e).toMatchObject({ blocked: false, newPaid: 30, balanceDue: 0, status: 'PAID' });
  });

  it('settled-later bill gets no invented upfront', () => {
    const e = lotPriceEffect({ ...base, otherLineTotals: [], editedLineTotal: 120, settled: 100, applied: 100 });
    expect(e).toMatchObject({ blocked: false, upfrontNew: 0, newPaid: 100, balanceDue: 20, status: 'PARTIAL' });
  });

  it('is blocked below the applied amount', () => {
    const e = lotPriceEffect({ ...base, otherLineTotals: [], editedLineTotal: 40, settled: 50, applied: 50 });
    expect(e).toEqual({ blocked: true, code: 'TOTAL_BELOW_APPLIED', newTotal: 40, applied: 50 });
  });
});
