import { describe, it, expect, vi, beforeEach } from 'vitest';
import { act } from 'react';
import { render } from '../testUtils';
import { flush, installApi, makeItem, typeInto } from '../inventory/testHelpers';
import { __resetHotkeys } from '@/hooks/useHotkeys';
import type { ItemSearchResult, LotDraw } from '@/types';

let pickItem: (i: ItemSearchResult) => void = () => {};
vi.mock('@/components/domain/ItemSearchTypeahead', () => ({
  ItemSearchTypeahead: (p: {
    onSelectItem: (i: ItemSearchResult) => void;
    onSelect?: (i: ItemSearchResult, info: { matchedBundleId: number | null }) => void;
  }) => {
    pickItem = (i) => {
      p.onSelectItem(i);
      p.onSelect?.(i, { matchedBundleId: null });
    };
    return <input data-testid="stub-search" />;
  },
}));

import { RapidBillingRow } from '@/components/domain/RapidBillingRow';
import { LotDrawLine } from '@/components/domain/LotDrawLine';

const box = { bundleId: 5, bundleName: 'Box', unitsPerBundle: 10, bundlePrice: 100, defaultRate: 100, barcode: null };
const lot = (id: number, unitCost: number, remaining: number, draw: number) => ({
  purchaseItemId: id,
  transactionCode: `PUR-${id}`,
  source: 'PURCHASE' as const,
  unitCost,
  remainingQuantity: remaining,
  drawQuantity: draw,
});
const drawOf = (lots: LotDraw['lots'], over: Partial<LotDraw> = {}): LotDraw => ({
  lots,
  drawCost: 0,
  available: lots.reduce((s, l) => s + l.remainingQuantity, 0),
  shortfall: 0,
  ...over,
});

beforeEach(() => {
  document.body.innerHTML = '';
  __resetHotkeys();
});

describe('LotDrawLine (FR-31)', () => {
  it('merges adjacent equal prices, highlights the drawn group and shows the cost', () => {
    const d = drawOf([lot(1, 40, 5, 5), lot(2, 40, 3, 1), lot(3, 42.5, 20, 0)], { drawCost: 240 });
    const { container } = render(<LotDrawLine draw={d} baseUnit="PCS" />);
    const groups = Array.from(container.querySelectorAll('[data-testid="lot-group"]'));
    expect(groups.map((g) => g.textContent)).toEqual(['₹40.00 × 8', '₹42.50 × 20']);
    expect(groups[0].getAttribute('data-drawn')).toBe('true');
    expect(groups[0].className).toContain('font-bold');
    expect(groups[1].getAttribute('data-drawn')).toBe('false');
    expect(container.textContent).toContain('PCS');
    expect(container.querySelector('[data-testid="lot-draw-cost"]')?.textContent).toContain('₹240.00');
    expect(container.querySelector('[data-testid="lot-shortfall"]')).toBeNull();
  });

  it('shows the shortfall in rose and caps the groups at four with a tooltip', () => {
    const lots = [1, 2, 3, 4, 5, 6].map((i) => lot(i, 10 + i, 2, 0));
    const { container } = render(<LotDrawLine draw={drawOf(lots, { shortfall: 3 })} baseUnit="PCS" />);
    expect(container.querySelectorAll('[data-testid="lot-group"]')).toHaveLength(4);
    const more = container.querySelector('[data-testid="lot-more"]') as HTMLElement;
    expect(more.textContent).toBe('+2 more');
    expect(more.title).toContain('₹16.00 × 2');
    const short = container.querySelector('[data-testid="lot-shortfall"]') as HTMLElement;
    expect(short.textContent).toBe('short by 3');
    expect(short.className).toContain('rose');
    expect(container.querySelector('[data-testid="lot-draw-cost"]')).toBeNull();
  });
});

describe('RapidBillingRow info line (FR-31)', () => {
  it('shows lot prices from previewLotDraw, no Rate text, and sends base units with the prior quantity', async () => {
    const previewLotDraw = vi.fn(async () => drawOf([lot(1, 40, 5, 5), lot(2, 42.5, 20, 5)], { drawCost: 412.5 }));
    installApi({ previewLotDraw });
    const { container } = render(<RapidBillingRow priorQuantity={(id) => (id === 9 ? 3 : 0)} />);
    act(() => pickItem(makeItem({ itemId: 9, name: 'Pencil', bundles: [box] })));
    await flush();
    await act(async () => {
      await new Promise((r) => setTimeout(r, 150));
    });
    const info = container.querySelector('[data-testid="stock-indicator"]') as HTMLElement;
    expect(info.textContent).toContain('Purchase price:');
    expect(info.textContent).toContain('₹40.00 × 5');
    expect(info.textContent).toContain('₹42.50 × 20');
    expect(info.textContent).toContain('→ cost');
    expect(info.textContent).not.toContain('Rate:');
    expect(previewLotDraw).toHaveBeenLastCalledWith(9, 1, 3);

    // 2 boxes of 10 -> 20 base units
    const sel = container.querySelector('[data-testid="bundle-select"]') as HTMLSelectElement;
    typeInto(sel, '5');
    typeInto(container.querySelector('[data-testid="quantity-input"]') as HTMLInputElement, '2');
    await act(async () => {
      await new Promise((r) => setTimeout(r, 150));
    });
    expect(previewLotDraw).toHaveBeenLastCalledWith(9, 20, 3);
  });

  it('has no lot text when the preview returns no lots', async () => {
    installApi({ previewLotDraw: vi.fn(async () => drawOf([])) });
    const { container } = render(<RapidBillingRow />);
    act(() => pickItem(makeItem({ itemId: 9, name: 'Pencil' })));
    await flush();
    await act(async () => {
      await new Promise((r) => setTimeout(r, 150));
    });
    const info = container.querySelector('[data-testid="stock-indicator"]') as HTMLElement;
    expect(info.textContent).not.toContain('Purchase price');
    expect(info.textContent).not.toContain('Rate:');
  });
});
