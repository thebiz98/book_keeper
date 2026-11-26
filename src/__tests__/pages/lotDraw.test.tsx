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
  it('shows one row per lot (equal prices are not merged), highlights drawn lots and shows the cost', () => {
    const d = drawOf([lot(1, 40, 5, 5), lot(2, 40, 3, 1), lot(3, 42.5, 20, 0)], { drawCost: 240 });
    const { container } = render(<LotDrawLine draw={d} baseUnit="PCS" />);
    const rows = Array.from(container.querySelectorAll('[data-testid="lot-row"]'));
    expect(rows).toHaveLength(3);
    expect(rows[0].textContent).toContain('PUR-1');
    expect(rows[0].textContent).toContain('₹40.00');
    expect(rows[0].textContent).toContain('× 5 PCS left');
    expect(rows[0].textContent).toContain('uses 5');
    expect(rows[1].textContent).toContain('uses 1');
    expect(rows[0].getAttribute('data-drawn')).toBe('true');
    expect(rows[0].className).toContain('font-bold');
    expect(rows[2].getAttribute('data-drawn')).toBe('false');
    expect(rows[2].textContent).not.toContain('uses');
    expect(container.querySelector('[data-testid="lot-draw-cost"]')?.textContent).toContain('₹240.00');
    expect(container.querySelector('[data-testid="lot-shortfall"]')).toBeNull();
  });

  it('tags opening and unrecorded lots', () => {
    const d = drawOf([
      { ...lot(1, 40, 5, 0), source: 'OPENING', transactionCode: 'OPN-1' },
      { ...lot(2, 41, 5, 0), source: 'ADJUSTMENT', transactionCode: 'ADJ-1' },
    ]);
    const { container } = render(<LotDrawLine draw={d} baseUnit="PCS" />);
    const rows = Array.from(container.querySelectorAll('[data-testid="lot-row"]'));
    expect(rows[0].textContent).toContain('(opening)');
    expect(rows[1].textContent).toContain('(unrecorded)');
  });

  it('shows the shortfall in rose and caps the rows at five with a tooltip', () => {
    const lots = [1, 2, 3, 4, 5, 6, 7].map((i) => lot(i, 10 + i, 2, 0));
    const { container } = render(<LotDrawLine draw={drawOf(lots, { shortfall: 3 })} baseUnit="PCS" />);
    expect(container.querySelectorAll('[data-testid="lot-row"]')).toHaveLength(5);
    const more = container.querySelector('[data-testid="lot-more"]') as HTMLElement;
    expect(more.textContent).toBe('+2 more');
    expect(more.title).toContain('PUR-6 ₹16.00 × 2 PCS left');
    const short = container.querySelector('[data-testid="lot-shortfall"]') as HTMLElement;
    expect(short.textContent).toBe('short by 3');
    expect(short.className).toContain('rose');
    expect(container.querySelector('[data-testid="lot-draw-cost"]')).toBeNull();
  });
});

describe('RapidBillingRow info line (FR-31)', () => {
  it('shows lot prices from previewLotDraw, no Rate text, and sends base units with the earlier cart lines', async () => {
    const previewLotDraw = vi.fn(async () => drawOf([lot(1, 40, 5, 5), lot(2, 42.5, 20, 5)], { drawCost: 412.5 }));
    installApi({ previewLotDraw });
    const { container } = render(<RapidBillingRow priorLines={() => [{ itemId: 9, quantity: 3 }]} />);
    act(() => pickItem(makeItem({ itemId: 9, name: 'Pencil', bundles: [box] })));
    await flush();
    await act(async () => {
      await new Promise((r) => setTimeout(r, 150));
    });
    const info = container.querySelector('[data-testid="stock-indicator"]') as HTMLElement;
    expect(info.textContent).toContain('Purchase price per lot');
    expect(info.textContent).toContain('₹40.00× 5 PCS left');
    expect(info.textContent).toContain('₹42.50× 20 PCS left');
    expect(info.textContent).toContain('→ cost');
    expect(info.textContent).not.toContain('Rate:');
    expect(previewLotDraw).toHaveBeenLastCalledWith(9, 1, [{ itemId: 9, quantity: 3 }]);

    // 2 boxes of 10 -> 20 base units
    const sel = container.querySelector('[data-testid="bundle-select"]') as HTMLSelectElement;
    typeInto(sel, '5');
    typeInto(container.querySelector('[data-testid="quantity-input"]') as HTMLInputElement, '2');
    await act(async () => {
      await new Promise((r) => setTimeout(r, 150));
    });
    expect(previewLotDraw).toHaveBeenLastCalledWith(9, 20, [{ itemId: 9, quantity: 3 }]);
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

describe('RapidBillingRow service info line (FR-37)', () => {
  const svcDraw = (): LotDraw => ({
    lots: [],
    drawCost: 10,
    available: 8,
    shortfall: 0,
    components: [
      { itemId: 1, name: 'Photo paper', baseUnit: 'SHEET', quantity: 4, lots: [], drawCost: 10, available: 20, shortfall: 0 },
    ],
    componentsCost: 10,
    miscCost: 2.5,
    lineCost: 12.5,
  });

  it('shows the components, their cost, misc. and line cost from the backend, Can make, and no new-bundle option', async () => {
    const previewLotDraw = vi.fn(async () => svcDraw());
    installApi({ previewLotDraw });
    const { container } = render(
      <RapidBillingRow onOpenBundleModal={() => undefined} priorLines={() => [{ itemId: 1, quantity: 2 }]} />
    );
    act(() => pickItem(makeItem({ itemId: 7, name: 'Photo print', baseUnit: 'PRINT', itemType: 'SERVICE', canMake: 8, bundles: [] })));
    await flush();
    await act(async () => {
      await new Promise((r) => setTimeout(r, 150));
    });
    const info = container.querySelector('[data-testid="stock-indicator"]') as HTMLElement;
    expect(info.textContent).toContain('Can make: 8 PRINT');
    expect(info.textContent).not.toContain('Available');
    expect(info.querySelector('[data-testid="service-component"]')?.textContent).toContain('4 SHEET Photo paper');
    expect(info.querySelector('[data-testid="service-components-cost"]')?.textContent).toContain('₹10.00');
    expect(info.querySelector('[data-testid="service-misc-cost"]')?.textContent).toContain('₹2.50');
    expect(info.querySelector('[data-testid="service-line-cost"]')?.textContent).toContain('₹12.50');
    expect(previewLotDraw).toHaveBeenLastCalledWith(7, 1, [{ itemId: 1, quantity: 2 }]);
    const options = Array.from(container.querySelectorAll('[data-testid="bundle-select"] option')).map((o) => o.textContent);
    expect(options).toEqual(['Base (PRINT)']);
  });
});
