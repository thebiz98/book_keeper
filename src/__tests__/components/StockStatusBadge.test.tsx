import { describe, it, expect } from 'vitest';
import { render } from '../testUtils';
import StockStatusBadge from '@/components/common/StockStatusBadge';

// ============================================================================
// Test Suite: StockStatusBadge
// Spec: docs/book_keeper_UI_LLD.md §5.2 (Item 3), PROJECT.md F16, F43
// Status Rules:
//   - currentStock === 0 -> Danger "Out of Stock" (bg-rose-*)
//   - isLowStock (server flag, never the number alone) -> Warning "Low Stock: {currentStock} {unit}" (bg-amber-*)
//   - currentStock >= 5  -> Success "{currentStock} {unit}" (bg-emerald-*)
// ============================================================================

describe('StockStatusBadge - Three-Tier Retail Stock Health Indicator', () => {
  describe('1. Zero Stock (Danger / Out of Stock)', () => {
    it('TC-SSB-001: renders "Out of Stock" with Danger styling when currentStock is 0', () => {
      const { container } = render(<StockStatusBadge currentStock={0} baseUnit="PCS" />);

      expect(container.textContent).toContain('Out of Stock');

      // Verify Danger semantic styling (rose or red)
      const badge = container.firstChild as HTMLElement;
      expect(badge.className).toMatch(/rose|red|danger/i);
    });

    it('TC-SSB-002: renders "Out of Stock" even if baseUnit is non-standard (e.g. BOX, KG)', () => {
      const { container } = render(<StockStatusBadge currentStock={0} baseUnit="BOX" />);
      expect(container.textContent).toContain('Out of Stock');
    });
  });

  describe('2. Low Stock (< 5 base units, Warning)', () => {
    it('TC-SSB-003: renders "Low Stock: 3 PCS" with Warning styling when currentStock is 3', () => {
      const { container } = render(<StockStatusBadge currentStock={3} isLowStock baseUnit="PCS" />);

      expect(container.textContent).toMatch(/Low Stock:\s*3\s*PCS/i);

      // Verify Warning semantic styling (amber or yellow)
      const badge = container.firstChild as HTMLElement;
      expect(badge.className).toMatch(/amber|yellow|warning/i);
    });

    it('TC-SSB-004: renders warning for stock of 1', () => {
      const { container } = render(<StockStatusBadge currentStock={1} isLowStock baseUnit="PCS" />);
      expect(container.textContent).toMatch(/Low Stock:\s*1\s*PCS/i);
    });

    it('TC-SSB-005: handles fractional decimal stock under 5 (e.g. 4.5 KG)', () => {
      const { container } = render(<StockStatusBadge currentStock={4.5} isLowStock baseUnit="KG" />);
      expect(container.textContent).toMatch(/Low Stock:\s*4\.5\s*KG/i);
    });

    it('TC-SSB-006: boundary test at 4.99 triggers Warning', () => {
      const { container } = render(<StockStatusBadge currentStock={4.99} isLowStock baseUnit="PCS" />);
      expect(container.textContent).toMatch(/Low Stock/i);
      const badge = container.firstChild as HTMLElement;
      expect(badge.className).toMatch(/amber|yellow|warning/i);
    });
  });

  describe('2b. Flag-only low stock and EPS boundaries (F6-b)', () => {
    it('does not warn for small stock when the server flag is false', () => {
      const { container } = render(<StockStatusBadge currentStock={3} baseUnit="PCS" isLowStock={false} />);
      expect(container.textContent).not.toContain('Low Stock');
      expect((container.firstChild as HTMLElement).className).toMatch(/emerald/);
    });
    it('warns for a large stock when the server flag is true', () => {
      const { container } = render(<StockStatusBadge currentStock={50} baseUnit="PCS" isLowStock />);
      expect(container.textContent).toContain('Low Stock: 50 PCS');
    });
    it('treats stock at or below EPS as out of stock, even with the low flag', () => {
      const a = render(<StockStatusBadge currentStock={1e-12} isLowStock />);
      expect(a.container.textContent).toContain('Out of Stock');
      const b = render(<StockStatusBadge currentStock={-0.5} />);
      expect(b.container.textContent).toContain('Out of Stock');
    });
    it('shows the per-item level as a tooltip, and "alert off" for 0', () => {
      const a = render(<StockStatusBadge currentStock={3} isLowStock baseUnit="KG" lowStockThreshold={10} />);
      expect((a.container.firstChild as HTMLElement).getAttribute('title')).toBe('Low when below 10 KG');
      const b = render(<StockStatusBadge currentStock={3} baseUnit="KG" lowStockThreshold={0} />);
      expect((b.container.firstChild as HTMLElement).getAttribute('title')).toBe('Low-stock alert off');
    });
    it('treats a just-above-EPS stock as in stock', () => {
      const { container } = render(<StockStatusBadge currentStock={0.001} baseUnit="KG" />);
      expect(container.textContent).toContain('0.001 KG');
    });
  });

  describe('3. Healthy Stock (>= 5 base units, Success)', () => {
    it('TC-SSB-007: renders "5 PCS" with Success styling at boundary threshold 5', () => {
      const { container } = render(<StockStatusBadge currentStock={5} baseUnit="PCS" />);

      expect(container.textContent).toContain('5 PCS');
      expect(container.textContent).not.toContain('Low Stock');
      expect(container.textContent).not.toContain('Out of Stock');

      // Verify Success semantic styling (emerald or green)
      const badge = container.firstChild as HTMLElement;
      expect(badge.className).toMatch(/emerald|green|success/i);
    });

    it('TC-SSB-008: renders bulk healthy stock (e.g. 150 REAM)', () => {
      const { container } = render(<StockStatusBadge currentStock={150} baseUnit="REAM" />);
      expect(container.textContent).toContain('150 REAM');
      const badge = container.firstChild as HTMLElement;
      expect(badge.className).toMatch(/emerald|green|success/i);
    });

    it('TC-SSB-009: defaults baseUnit to "PCS" if not explicitly provided', () => {
      const { container } = render(<StockStatusBadge currentStock={25} />);
      expect(container.textContent).toContain('25 PCS');
    });

    it('TC-SSB-010: boundary test at 5.01 renders Success', () => {
      const { container } = render(<StockStatusBadge currentStock={5.01} baseUnit="PCS" />);
      expect(container.textContent).not.toContain('Low Stock');
      const badge = container.firstChild as HTMLElement;
      expect(badge.className).toMatch(/emerald|green|success/i);
    });
  });
});
