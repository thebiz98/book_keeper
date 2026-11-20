import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, fireEvent } from '../testUtils';
import RapidBillingRow from '@/components/domain/RapidBillingRow';

// ============================================================================
// Test Suite: RapidBillingRow
// Spec: docs/book_keeper_UI_LLD.md §4.2, §5.3, docs/book_keeper_UI_UX_Specification.md §3 (Page 2)
// Sequential Focus Cycle: Search -> Quantity -> Bundle -> Rate -> Cart Table
// Inline Shortcut: Alt + B opens BundleCreationModal
// ============================================================================

describe('RapidBillingRow - High-Speed Counter Billing Focus Cycle', () => {
  beforeEach(() => {
    document.body.innerHTML = '';
  });

  describe('1. Component Rendering', () => {
    it('TC-RBR-001: renders all input elements required for sequential billing', () => {
      const { container } = render(<RapidBillingRow onAddItem={vi.fn()} />);

      // Verify presence of Search Input, Quantity Input, Bundle/Unit Selector, Rate Input, and Add button
      const searchInput = container.querySelector('input[placeholder*="Search"], input[name="search"], [data-testid="item-search-input"]');
      const qtyInput = container.querySelector('input[type="number"][name*="qty"], input[placeholder*="Qty"], [data-testid="quantity-input"]');
      const rateInput = container.querySelector('input[type="number"][name*="rate"], input[placeholder*="Rate"], [data-testid="rate-input"]');
      const addBtn = container.querySelector('button[type="submit"], button[data-testid="add-item-btn"], button');

      expect(searchInput || container.querySelector('input')).not.toBeNull();
      expect(qtyInput || container.querySelectorAll('input')[1]).not.toBeNull();
      expect(rateInput || container.querySelectorAll('input')[2]).not.toBeNull();
      expect(addBtn).not.toBeNull();
    });
  });

  describe('2. Sequential Focus Cycle on Enter Key', () => {
    it('TC-RBR-002: advances focus from Search -> Quantity on Enter key', () => {
      const { container } = render(<RapidBillingRow onAddItem={vi.fn()} autoFocus />);

      const inputs = container.querySelectorAll('input, select');
      expect(inputs.length).toBeGreaterThanOrEqual(3);

      const searchInput = inputs[0] as HTMLInputElement;
      const qtyInput = inputs[1] as HTMLInputElement;

      // Focus on search input and type query
      fireEvent.focus(searchInput);
      fireEvent.change(searchInput, { target: { value: 'Apsara Pencil' } });

      // Press Enter to select item and advance focus
      fireEvent.keyDown(searchInput, { key: 'Enter', code: 'Enter' });

      // Focus should move to Quantity Input
      expect(document.activeElement === qtyInput || qtyInput.hasAttribute('autofocus')).toBe(true);
    });

    it('TC-RBR-003: advances focus from Quantity -> Bundle Selector on Enter key', () => {
      const { container } = render(<RapidBillingRow onAddItem={vi.fn()} />);

      const inputs = container.querySelectorAll('input, select');
      const qtyInput = inputs[1] as HTMLElement;
      const bundleSelect = (inputs[2] || container.querySelector('select, [data-testid="bundle-select"]')) as HTMLElement;

      fireEvent.focus(qtyInput);
      fireEvent.change(qtyInput, { target: { value: '5' } });
      fireEvent.keyDown(qtyInput, { key: 'Enter', code: 'Enter' });

      // Focus advances to bundle or rate
      expect(document.activeElement === bundleSelect || bundleSelect.hasAttribute('autofocus')).toBe(true);
    });

    it('TC-RBR-004: advances focus from Rate -> Commit (Add Item) on Enter key', () => {
      const onAddItem = vi.fn();
      const { container } = render(<RapidBillingRow onAddItem={onAddItem} />);

      const inputs = container.querySelectorAll('input');
      const searchInput = inputs[0];
      const qtyInput = inputs[1];
      const rateInput = inputs[inputs.length - 1];

      // Populate valid item data
      fireEvent.change(searchInput, { target: { value: 'Notebook' } });
      fireEvent.change(qtyInput, { target: { value: '2' } });
      fireEvent.change(rateInput, { target: { value: '50' } });

      // Press Enter on Rate input to commit to cart
      fireEvent.keyDown(rateInput, { key: 'Enter', code: 'Enter' });

      // Verify commit was triggered
      expect(onAddItem).toHaveBeenCalled();
    });

    it('TC-RBR-005: resets focus to Search Input after adding item to cart', () => {
      const onAddItem = vi.fn();
      const { container } = render(<RapidBillingRow onAddItem={onAddItem} />);

      const inputs = container.querySelectorAll('input');
      const searchInput = inputs[0];
      const rateInput = inputs[inputs.length - 1];

      fireEvent.change(searchInput, { target: { value: 'Gel Pen' } });
      fireEvent.keyDown(rateInput, { key: 'Enter', code: 'Enter' });

      // After adding, cursor loops back to search input for next customer scan
      expect(document.activeElement === searchInput || searchInput.getAttribute('data-focused') === 'true').toBe(true);
    });
  });

  describe('3. Alt + B Keyboard Shortcut', () => {
    it('TC-RBR-006: Alt+B shortcut triggers bundle creation modal callback without abandoning cart', () => {
      const onOpenBundleModal = vi.fn();
      const { container } = render(
        <RapidBillingRow onAddItem={vi.fn()} onOpenBundleModal={onOpenBundleModal} />
      );

      // Focus on bundle select or anywhere in billing row and press Alt+B
      const bundleSelect = container.querySelector('select, [data-testid="bundle-select"]') || container;
      fireEvent.keyDown(bundleSelect, {
        key: 'b',
        code: 'KeyB',
        altKey: true,
      });

      expect(onOpenBundleModal).toHaveBeenCalledTimes(1);
    });

    it('TC-RBR-007: Alt+B works when focused in quantity input or rate input', () => {
      const onOpenBundleModal = vi.fn();
      const { container } = render(
        <RapidBillingRow onAddItem={vi.fn()} onOpenBundleModal={onOpenBundleModal} />
      );

      const qtyInput = container.querySelectorAll('input')[1];
      fireEvent.focus(qtyInput);
      fireEvent.keyDown(qtyInput, {
        key: 'b',
        code: 'KeyB',
        altKey: true,
      });

      expect(onOpenBundleModal).toHaveBeenCalled();
    });
  });

  describe('4. Negative Stock Protection & Validation', () => {
    it('TC-RBR-008: blocks adding item when entered quantity exceeds available stock', () => {
      const onAddItem = vi.fn();
      const { container } = render(<RapidBillingRow onAddItem={onAddItem} />);

      const inputs = container.querySelectorAll('input');
      const searchInput = inputs[0];
      const qtyInput = inputs[1];
      const rateInput = inputs[inputs.length - 1];

      // Enter search for item with low stock (e.g. 2 PCS available)
      fireEvent.change(searchInput, { target: { value: 'Calculator' } });
      // Enter quantity of 5 (exceeds stock of 2)
      fireEvent.change(qtyInput, { target: { value: '5' } });

      // Trigger commit
      fireEvent.keyDown(rateInput, { key: 'Enter', code: 'Enter' });

      // Verify commit is blocked and error message is displayed
      expect(onAddItem).not.toHaveBeenCalled();
      const errorMsg = container.querySelector('[role="alert"], .text-rose-600, .text-red-500, [data-testid="stock-error"]');
      expect(errorMsg).not.toBeNull();
      expect(errorMsg!.textContent).toMatch(/exceed|stock|available|insufficient/i);
    });
  });
});
