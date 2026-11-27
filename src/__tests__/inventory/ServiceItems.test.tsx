import { describe, it, expect, vi, beforeEach } from 'vitest';
import { act } from 'react';
import { render } from '../testUtils';
import { byText, click, flush, inputByLabel, installApi, makeItem, typeInto } from './testHelpers';
import { __resetHotkeys } from '@/hooks/useHotkeys';
import type { ItemSearchResult } from '@/types';

// The component picker is a typeahead; the test drives it through this stub.
let pickItem: (i: ItemSearchResult) => void = () => {};
vi.mock('@/components/domain/ItemSearchTypeahead', () => ({
  ItemSearchTypeahead: (p: { onSelectItem: (i: ItemSearchResult) => void }) => {
    pickItem = (i) => p.onSelectItem(i);
    return <input data-testid="stub-search" />;
  },
}));

import { InventoryPage } from '@/pages/InventoryPage';

const paper = makeItem({ itemId: 1, name: 'Photo paper', baseUnit: 'SHEET', currentStock: 40 });
const print = makeItem({
  itemId: 2,
  name: 'Photo print',
  baseUnit: 'PRINT',
  itemType: 'SERVICE',
  currentStock: 0,
  isLowStock: false,
  lowStockThreshold: 5,
  serviceCost: 3,
  canMake: 12,
});
const labour = makeItem({ itemId: 3, name: 'Lamination', baseUnit: 'PCS', itemType: 'SERVICE', currentStock: 0, canMake: null });

function mount(over: Record<string, unknown> = {}) {
  const api = installApi({
    getItemsCatalog: vi.fn().mockResolvedValue({ items: [paper, print, labour], total: 3, page: 1, limit: 50 }),
    createItem: vi.fn().mockResolvedValue(9),
    updateItem: vi.fn().mockResolvedValue(undefined),
    previewServiceCost: vi.fn().mockResolvedValue({ componentsCost: 8, miscCost: 2, unitCost: 10 }),
    getServiceComponents: vi
      .fn()
      .mockResolvedValue([{ componentItemId: 1, name: 'Photo paper', baseUnit: 'SHEET', quantityPerUnit: 2, currentStock: 40 }]),
    ...over,
  });
  const r = render(<InventoryPage />);
  return { api, ...r };
}

beforeEach(() => {
  document.body.innerHTML = '';
  __resetHotkeys();
});

describe('Service items in the item list (FR-36)', () => {
  it('shows a Service badge, Can make N or a dash, and hides stock actions for services', async () => {
    const { container } = mount();
    await flush();
    const rows = Array.from(container.querySelectorAll('[data-testid="item-row"]')) as HTMLElement[];
    expect(rows.length).toBe(3);
    const [rPaper, rPrint, rLabour] = rows;
    expect(rPaper.querySelector('[data-testid="service-badge"]')).toBeNull();
    expect(rPrint.querySelector('[data-testid="service-badge"]')?.textContent).toBe('Service');
    expect(rPrint.querySelector('[data-testid="can-make"]')?.textContent).toBe('Can make 12 PRINT');
    expect(rLabour.querySelector('[data-testid="can-make"]')?.textContent).toBe('—');
    expect(rPrint.textContent).not.toContain('Out of Stock');
    for (const label of ['Adjust stock for', 'Set stock for', 'Add bundle for', 'Show lots for']) {
      expect(rPrint.querySelector(`button[aria-label^="${label}"]`)).toBeNull();
      expect(rPaper.querySelector(`button[aria-label^="${label}"]`)).not.toBeNull();
    }
    expect(rPrint.querySelector('button[aria-label="Edit Photo print"]')).not.toBeNull();
  });
});

describe('Service drawer (FR-36)', () => {
  it('switching the type to Service hides stock fields and shows components with the backend cost line', async () => {
    const { api, container } = mount();
    await flush();
    click(byText(container, 'button', /Add Item/));
    await flush();
    expect(container.querySelector('[data-testid="service-components"]')).toBeNull();
    click(container.querySelector('[data-testid="type-service"]') as HTMLElement);
    await flush();

    expect(container.querySelector('[data-testid="service-components"]')).not.toBeNull();
    const labels = Array.from(container.querySelectorAll('label')).map((l) => l.textContent ?? '');
    expect(labels.some((l) => /Opening Stock/.test(l))).toBe(false);
    expect(labels.some((l) => /Low Stock Level/.test(l))).toBe(false);
    expect(labels.some((l) => /Service cost/.test(l))).toBe(true);

    act(() => pickItem(paper));
    await flush(150);
    const rows = container.querySelectorAll('[data-testid="component-row"]');
    expect(rows.length).toBe(1);
    expect(rows[0].textContent).toContain('Photo paper');
    typeInto(container.querySelector('[data-testid="component-qty"]') as HTMLInputElement, '2');
    typeInto(inputByLabel(container, /Service cost/), '10');
    await flush(250);
    // The figures are the backend's, shown as returned.
    expect(api.previewServiceCost).toHaveBeenLastCalledWith({
      components: [{ componentItemId: 1, quantityPerUnit: 2 }],
      serviceCost: 10,
    });
    expect(container.querySelector('[data-testid="service-cost-preview"]')?.textContent).toBe(
      'Components ₹8.00 · Misc ₹2.00 · Cost per unit ₹10.00'
    );

    // The same component cannot be added twice.
    act(() => pickItem(paper));
    await flush();
    expect(container.querySelectorAll('[data-testid="component-row"]').length).toBe(1);
    expect(container.querySelector('[data-testid="component-hint"]')?.textContent).toContain('already');

    typeInto(inputByLabel(container, /Service Name/), 'Photo print');
    typeInto(inputByLabel(container, /Unit/), 'print');
    typeInto(inputByLabel(container, /Sale Price/), '12');
    click(byText(container, 'button', /Save Service/));
    await flush();
    expect(api.createItem).toHaveBeenCalledTimes(1);
    expect(api.createItem).toHaveBeenCalledWith(
      expect.objectContaining({
        name: 'Photo print',
        baseUnit: 'PRINT',
        itemType: 'SERVICE',
        defaultSalePrice: 12,
        serviceCost: 10,
        components: [{ componentItemId: 1, quantityPerUnit: 2 }],
        initialStock: null,
        initialCost: null,
        bundles: null,
      })
    );
  });

  it('edit drawer of a service: type fixed, components loaded, save sends the replacement list', async () => {
    const { api, container } = mount();
    await flush();
    click(container.querySelector('button[aria-label="Edit Photo print"]') as HTMLElement);
    await flush();
    expect(api.getServiceComponents).toHaveBeenCalledWith(2);
    const typeButtons = Array.from(container.querySelectorAll('[data-testid="item-type"] button')) as HTMLButtonElement[];
    expect(typeButtons.every((b) => b.disabled)).toBe(true);
    expect(container.querySelector('[data-testid="component-row"]')?.textContent).toContain('Photo paper');
    expect((container.querySelector('[data-testid="component-qty"]') as HTMLInputElement).value).toBe('2');
    expect(container.querySelector('[data-testid="set-stock-button"]')).toBeNull();

    click(container.querySelector('button[aria-label="Remove Photo paper"]') as HTMLElement);
    await flush();
    click(byText(container, 'button', /Save Changes/));
    await flush();
    expect(api.updateItem).toHaveBeenCalledWith(
      2,
      expect.objectContaining({ serviceCost: 3, components: [], name: 'Photo print' })
    );
  });

  it('shows a backend component error on the form', async () => {
    const { container } = mount({
      createItem: vi.fn().mockRejectedValue({ type: 'ValidationError', details: { field: 'components', message: 'A service cannot be its own component' } }),
    });
    await flush();
    click(byText(container, 'button', /Add Item/));
    await flush();
    click(container.querySelector('[data-testid="type-service"]') as HTMLElement);
    typeInto(inputByLabel(container, /Service Name/), 'Loop');
    typeInto(inputByLabel(container, /Sale Price/), '1');
    click(byText(container, 'button', /Save Service/));
    await flush();
    expect(container.querySelector('[role="alert"]')?.textContent).toContain('cannot be its own component');
  });
});
