import { describe, it, expect } from 'vitest';
import { act } from 'react';
import { render } from '../testUtils';
import { SalesPurchaseTrendPanel, niceTicks, compactRupees } from '@/components/domain/SalesPurchaseTrendPanel';
import { installApi, waitFor, click, bodyText } from '../pages/helpers';

const data = [
  { date: '2026-02-01', salesAmount: 1200, purchaseAmount: 500 },
  { date: '2026-02-02', salesAmount: 9800, purchaseAmount: 3000 },
  { date: '2026-02-03', salesAmount: 4000, purchaseAmount: 7500 },
];

const q = (s: string) => document.querySelector(s);
const qa = (s: string) => Array.from(document.querySelectorAll(s));

async function open() {
  installApi({ getSalesPurchaseTrend: data });
  render(<SalesPurchaseTrendPanel />);
  await waitFor(() => !!q('[data-testid="trend-svg"]'));
}

describe('SalesPurchaseTrendPanel chart (FR-40)', () => {
  it('draws one line per series and a y axis that starts at 0', async () => {
    await open();
    expect(qa('path[data-series]').map((p) => p.getAttribute('data-series'))).toEqual(['sales', 'purchases']);
    const labels = qa('[data-testid="trend-ytick"] text').map((t) => t.textContent);
    expect(labels[0]).toBe('₹0');
    expect(labels.length).toBeGreaterThanOrEqual(4);
    expect(labels.length).toBeLessThanOrEqual(6);
    expect(q('[data-testid="trend-svg"]')!.getAttribute('role')).toBe('img');
    expect(qa('circle[data-marker]')).toHaveLength(6);
  });

  it('shows both values for a bucket on hover and on Left/Right keys', async () => {
    await open();
    expect(q('[data-testid="trend-tooltip"]')).toBeNull();
    act(() => {
      qa('[data-testid="trend-hit"]')[1].dispatchEvent(new MouseEvent('mouseover', { bubbles: true }));
    });
    const tip = q('[data-testid="trend-tooltip"]')!;
    expect(tip.textContent).toContain('2026-02-02');
    expect(tip.textContent).toContain('₹9,800.00');
    expect(tip.textContent).toContain('₹3,000.00');
    expect(q('[data-testid="trend-crosshair"]')).not.toBeNull();

    const plot = q('[data-testid="trend-svg"]')!.parentElement!;
    act(() => {
      plot.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowRight', bubbles: true }));
    });
    expect(q('[data-testid="trend-tooltip"]')!.textContent).toContain('2026-02-03');
    expect(q('[data-testid="trend-tooltip"]')!.textContent).toContain('₹7,500.00');
    act(() => {
      plot.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowLeft', bubbles: true }));
      plot.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowLeft', bubbles: true }));
    });
    expect(q('[data-testid="trend-tooltip"]')!.textContent).toContain('2026-02-01');
  });

  it('the Table toggle lists every bucket', async () => {
    await open();
    click(q('[data-view="table"]')!);
    expect(q('[data-testid="trend-svg"]')).toBeNull();
    const rows = qa('[data-testid="trend-table"] tbody tr');
    expect(rows).toHaveLength(3);
    expect(rows[1].textContent).toContain('₹9,800.00');
    expect(bodyText()).toContain('Purchases');
    click(q('[data-view="chart"]')!);
    expect(q('[data-testid="trend-svg"]')).not.toBeNull();
  });

  it('nice ticks and compact labels', () => {
    expect(niceTicks(9800)).toEqual([0, 2500, 5000, 7500, 10000]);
    expect(niceTicks(0)[0]).toBe(0);
    expect(compactRupees(12000)).toBe('₹12k');
    expect(compactRupees(500)).toBe('₹500');
    expect(compactRupees(150000)).toBe('₹1.5L');
  });
});
