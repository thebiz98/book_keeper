import { describe, it, expect } from 'vitest';
import { render } from '../testUtils';
import CurrencyDisplay from '@/components/common/CurrencyDisplay';

// ============================================================================
// Test Suite: CurrencyDisplay
// Spec: docs/book_keeper_UI_LLD.md §5.2 (Item 1), PROJECT.md F43
// Formatting: Intl.NumberFormat('en-IN', { minimumFractionDigits: 2 }), tabular-nums font-mono
// ============================================================================

describe('CurrencyDisplay - Indian Rupee (₹) Tabular Formatting', () => {
  describe('1. Default INR Formatting & Currency Symbol', () => {
    it('TC-CURR-001: formats positive integer amount with ₹ symbol and 2 decimal places', () => {
      const { container } = render(<CurrencyDisplay amount={1250} />);
      // Expected: ₹1,250.00 (Indian grouping)
      expect(container.textContent).toContain('₹');
      expect(container.textContent).toMatch(/1,250\.00/);
    });

    it('TC-CURR-002: formats large amounts using Indian Lakh separation (1,00,000.00)', () => {
      const { container } = render(<CurrencyDisplay amount={100000} />);
      // Indian numbering system puts commas at 1,00,000.00
      expect(container.textContent).toContain('₹');
      expect(container.textContent).toMatch(/1,00,000\.00/);
      expect(container.textContent).not.toMatch(/100,000\.00/);
    });

    it('TC-CURR-003: formats zero amount as ₹0.00', () => {
      const { container } = render(<CurrencyDisplay amount={0} />);
      expect(container.textContent).toContain('₹');
      expect(container.textContent).toMatch(/0\.00/);
    });

    it('TC-CURR-004: formats single decimal input to 2 decimal places (99.5 -> ₹99.50)', () => {
      const { container } = render(<CurrencyDisplay amount={99.5} />);
      expect(container.textContent).toMatch(/99\.50/);
    });

    it('TC-CURR-005: rounds fractional fractions beyond 2 decimals (e.g. 10.456 -> ₹10.46)', () => {
      const { container } = render(<CurrencyDisplay amount={10.456} />);
      expect(container.textContent).toMatch(/10\.46/);
    });
  });

  describe('2. Typography & Layout Classes', () => {
    it('TC-CURR-006: enforces tabular-nums and font-mono for jitter-free counter display', () => {
      const { container } = render(<CurrencyDisplay amount={450.0} />);
      const el = container.firstChild as HTMLElement;
      expect(el.className).toContain('tabular-nums');
      expect(el.className).toContain('font-mono');
    });

    it('TC-CURR-007: renders hero size typography for grand totals', () => {
      const { container } = render(<CurrencyDisplay amount={3450.0} size="hero" />);
      const el = container.firstChild as HTMLElement;
      expect(el.className).toMatch(/text-3xl|text-4xl|hero/);
    });

    it('TC-CURR-008: supports small (sm) and large (lg) size variants', () => {
      const { container: smContainer } = render(<CurrencyDisplay amount={50} size="sm" />);
      const smEl = smContainer.firstChild as HTMLElement;
      expect(smEl.className).toMatch(/text-xs|text-sm/);

      const { container: lgContainer } = render(<CurrencyDisplay amount={50} size="lg" />);
      const lgEl = lgContainer.firstChild as HTMLElement;
      expect(lgEl.className).toMatch(/text-lg|text-xl/);
    });
  });

  describe('3. Colored & ShowSign Props', () => {
    it('TC-CURR-009: colored=true applies emerald/green class for positive cash inflow', () => {
      const { container } = render(<CurrencyDisplay amount={500} colored />);
      const el = container.firstChild as HTMLElement;
      expect(el.className).toMatch(/emerald|green/);
    });

    it('TC-CURR-010: colored=true applies rose/red class for negative cash outflow', () => {
      const { container } = render(<CurrencyDisplay amount={-250} colored />);
      const el = container.firstChild as HTMLElement;
      expect(el.className).toMatch(/rose|red/);
    });

    it('TC-CURR-011: colored=true retains neutral styling for zero amount', () => {
      const { container } = render(<CurrencyDisplay amount={0} colored />);
      const el = container.firstChild as HTMLElement;
      // Zero should not be styled with positive or negative colored alert
      expect(el.className).not.toMatch(/text-rose-600|text-emerald-600/);
    });

    it('TC-CURR-012: showSign=true prepends "+" sign for positive amounts', () => {
      const { container } = render(<CurrencyDisplay amount={750} showSign />);
      expect(container.textContent).toContain('+');
    });

    it('TC-CURR-013: handles negative numbers with "-" sign', () => {
      const { container } = render(<CurrencyDisplay amount={-150} />);
      expect(container.textContent).toContain('-');
      expect(container.textContent).toMatch(/150\.00/);
    });
  });

  describe('4. Custom Currency Symbol', () => {
    it('TC-CURR-014: allows overriding default ₹ with custom currency symbol', () => {
      const { container } = render(<CurrencyDisplay amount={500} currencySymbol="$" />);
      expect(container.textContent).toContain('$');
      expect(container.textContent).not.toContain('₹');
    });
  });
});
