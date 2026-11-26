import { describe, it, expect, vi } from 'vitest';
import { render } from '../testUtils';
import { click, flush, installApi, makeItem, typeInto } from '../inventory/testHelpers';
import { NewPurchasePage } from '@/pages/NewPurchasePage';
import { queuePurchasePrefill } from '@/pages/purchasePrefill';

describe('NewPurchasePage item picker (real typeahead)', () => {
  it('searches the catalog and selects a real item (no free-text item name)', async () => {
    const api = installApi({
      searchItemsFts: vi.fn().mockResolvedValue([makeItem({ itemId: 4, name: 'Camlin Ink' })]),
    });
    const { container } = render(<NewPurchasePage />);
    await flush();
    expect(container.textContent).not.toContain('Item Name *');

    typeInto(container.querySelector('[data-testid="purchase-item-search"] input') as HTMLInputElement, 'cam');
    await flush(250);
    expect(api.searchItemsFts).toHaveBeenCalledWith('cam', expect.anything());

    const hit = Array.from(container.querySelectorAll<HTMLElement>('div')).find(
      (d) => d.textContent === 'Camlin Ink' || (d.children.length === 0 && d.textContent === 'Camlin Ink')
    ) ?? (Array.from(container.querySelectorAll('span')).find((s) => s.textContent === 'Camlin Ink') as HTMLElement);
    click(hit);
    await flush();
    expect((container.querySelector('[data-testid="item-search-input"]') as HTMLInputElement).value).toBe('Camlin Ink');
    expect(container.querySelector('[data-testid="selected-item"]')?.textContent).toContain('PCS');
  });

  it('pre-selects an item queued by the dashboard Inward action', async () => {
    const api = installApi({
      searchItemsFts: vi.fn().mockResolvedValue([makeItem({ itemId: 4, name: 'Camlin Ink' })]),
    });
    queuePurchasePrefill({ itemId: 4, name: 'Camlin Ink' });
    const { container } = render(<NewPurchasePage />);
    await flush(20);
    expect(api.searchItemsFts).toHaveBeenCalledWith('Camlin Ink', 8);
    expect((container.querySelector('[data-testid="item-search-input"]') as HTMLInputElement).value).toBe('Camlin Ink');
    expect(container.querySelector('[data-testid="selected-item"]')?.textContent).toContain('PCS');
  });

  it('pre-selects by id via the catalog when only an id is given', async () => {
    const api = installApi({
      getItemsCatalog: vi.fn().mockResolvedValue({ items: [makeItem({ itemId: 4, name: 'Camlin Ink' })], total: 1, page: 1, limit: 200 }),
    });
    const { container } = render(<NewPurchasePage initialItemId={4} />);
    await flush(20);
    expect(api.getItemsCatalog).toHaveBeenCalled();
    expect((container.querySelector('[data-testid="item-search-input"]') as HTMLInputElement).value).toBe('Camlin Ink');
    expect(container.querySelector('[data-testid="selected-item"]')?.textContent).toContain('PCS');
  });
});
