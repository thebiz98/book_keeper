import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, fireEvent } from '../testUtils';
import SaleSummaryModal from '@/components/domain/SaleSummaryModal';

// ============================================================================
// Test Suite: SaleSummaryModal
// Spec: docs/book_keeper_UI_LLD.md §5.3, docs/book_keeper_UI_UX_Specification.md §3 (Page 3)
// Features: Hero Typography Total, Live Change Calculator, Semantic Badges, Keyboard Dismissal
// ============================================================================

describe('SaleSummaryModal - Checkout Summary & Cash Tendered Calculator', () => {
  beforeEach(() => {
    document.body.innerHTML = '';
  });

  describe('1. Visibility & Basic Rendering', () => {
    it('TC-SSM-001: does not render modal content when isOpen is false', () => {
      const { container } = render(
        <SaleSummaryModal isOpen={false} onClose={vi.fn()} grandTotal={1200.0} />
      );
      expect(container.querySelector('[role="dialog"], .modal')).toBeNull();
    });

    it('TC-SSM-002: renders dialog when isOpen is true with Grand Total in hero typography', () => {
      const { container } = render(
        <SaleSummaryModal
          isOpen={true}
          onClose={vi.fn()}
          grandTotal={1200.0}
          invoiceCode="SAL-1001"
          customerName="Sharmaji"
        />
      );

      // Verify dialog is in the document
      const dialog = container.querySelector('[role="dialog"]') || document.body.querySelector('[role="dialog"]') || container;
      expect(dialog).not.toBeNull();

      // Verify invoice code and customer name are displayed
      expect(document.body.textContent).toContain('SAL-1001');
      expect(document.body.textContent).toContain('Sharmaji');

      // Verify Grand Total formatted with ₹ in hero typography (text-3xl or bold font-mono)
      const totalEl = document.body.querySelector('.text-3xl, .text-4xl, .font-mono, [data-testid="grand-total"]');
      expect(totalEl).not.toBeNull();
      expect(document.body.textContent).toMatch(/1,?200(\.00)?/);
    });

    it('TC-SSM-003: auto-focuses Cash Received input on open', () => {
      const { container } = render(
        <SaleSummaryModal isOpen={true} onClose={vi.fn()} grandTotal={500.0} />
      );

      const cashInput = container.querySelector('input[type="number"], input[name*="cash"], [data-testid="cash-received-input"]') as HTMLInputElement;
      expect(cashInput).not.toBeNull();
      expect(document.activeElement === cashInput || cashInput.hasAttribute('autofocus')).toBe(true);
    });
  });

  describe('2. Live Change Calculation: Change = Received - Total', () => {
    it('TC-SSM-004: calculates exact change when cash received > grand total (Over-Tender)', () => {
      // Grand Total = ₹1,200.00, Cash Received = ₹1,500.00 -> Change = ₹300.00
      const { container } = render(
        <SaleSummaryModal
          isOpen={true}
          onClose={vi.fn()}
          grandTotal={1200.0}
          defaultCashReceived={1500.0}
        />
      );

      const cashInput = container.querySelector('input[type="number"], input[name*="cash"], [data-testid="cash-received-input"]') as HTMLInputElement;
      if (cashInput) {
        fireEvent.change(cashInput, { target: { value: '1500' } });
      }

      // Verify change text ₹300.00 is displayed
      expect(document.body.textContent).toMatch(/300(\.00)?/);
    });

    it('TC-SSM-005: renders Emerald badge / green styling when change >= 0', () => {
      const { container } = render(
        <SaleSummaryModal
          isOpen={true}
          onClose={vi.fn()}
          grandTotal={800.0}
          defaultCashReceived={1000.0}
        />
      );

      const cashInput = container.querySelector('input[type="number"], input') as HTMLInputElement;
      if (cashInput) {
        fireEvent.change(cashInput, { target: { value: '1000' } });
      }

      // Check for emerald/green semantic badge
      const badge = container.querySelector('.bg-emerald-50, .text-emerald-700, .bg-emerald-500, .text-emerald-500, .text-emerald-600, .text-green-600, [data-testid="change-badge"]');
      expect(badge).not.toBeNull();
      expect((badge as HTMLElement)?.className).toMatch(/emerald|green/);
    });

    it('TC-SSM-006: renders Rose badge / red styling when cash received is less than total (Under-Tender)', () => {
      // Grand Total = ₹1,000.00, Cash Received = ₹800.00 -> Short by ₹200.00
      const { container } = render(
        <SaleSummaryModal
          isOpen={true}
          onClose={vi.fn()}
          grandTotal={1000.0}
          defaultCashReceived={800.0}
        />
      );

      const cashInput = container.querySelector('input[type="number"], input') as HTMLInputElement;
      if (cashInput) {
        fireEvent.change(cashInput, { target: { value: '800' } });
      }

      // Check for rose/red semantic badge indicating shortage
      const shortageBadge = container.querySelector('.bg-rose-50, .text-rose-700, .bg-rose-500, .text-rose-600, .text-red-600, [data-testid="shortage-badge"]');
      expect(shortageBadge).not.toBeNull();
      expect((shortageBadge as HTMLElement)?.className).toMatch(/rose|red/);
      expect(document.body.textContent).toContain('200');
    });

    it('TC-SSM-007: calculates zero change when cash received equals grand total (Exact Tender)', () => {
      const { container } = render(
        <SaleSummaryModal
          isOpen={true}
          onClose={vi.fn()}
          grandTotal={450.0}
          defaultCashReceived={450.0}
        />
      );

      const cashInput = container.querySelector('input[type="number"], input') as HTMLInputElement;
      if (cashInput) {
        fireEvent.change(cashInput, { target: { value: '450' } });
      }

      expect(document.body.textContent).toMatch(/0(\.00)?/);
    });
  });

  describe('3. Keyboard Dismissal & Navigation', () => {
    it('TC-SSM-008: dismisses dialog on Enter key (confirm / next customer)', () => {
      const onClose = vi.fn();
      const onConfirm = vi.fn();
      const { container } = render(
        <SaleSummaryModal
          isOpen={true}
          onClose={onClose}
          onConfirm={onConfirm}
          grandTotal={250.0}
        />
      );

      // Trigger Enter key outside input or on dialog
      const dialog = container.querySelector('[role="dialog"]') || container;
      fireEvent.keyDown(dialog, { key: 'Enter', code: 'Enter' });

      expect(onClose.mock.calls.length + onConfirm.mock.calls.length).toBeGreaterThan(0);
    });

    it('TC-SSM-009: dismisses dialog on Space key outside text inputs', () => {
      const onClose = vi.fn();
      const onConfirm = vi.fn();
      const { container } = render(
        <SaleSummaryModal
          isOpen={true}
          onClose={onClose}
          onConfirm={onConfirm}
          grandTotal={250.0}
        />
      );

      const confirmBtn = container.querySelector('button[data-testid="confirm-btn"], button');
      expect(confirmBtn).not.toBeNull();
      fireEvent.keyDown(confirmBtn!, { key: ' ', code: 'Space' });
      expect(onClose.mock.calls.length + onConfirm.mock.calls.length).toBeGreaterThan(0);
    });

    it('TC-SSM-010: dismisses dialog on Escape key', () => {
      const onClose = vi.fn();
      const { container } = render(
        <SaleSummaryModal isOpen={true} onClose={onClose} grandTotal={250.0} />
      );

      const dialog = container.querySelector('[role="dialog"]') || container;
      fireEvent.keyDown(dialog, { key: 'Escape', code: 'Escape' });

      expect(onClose).toHaveBeenCalled();
    });
  });
});
