import { describe, it, expect } from 'vitest';
import { render } from '../testUtils';
import { ItemStockStatus } from '@/components/common/ItemStockStatus';

const base = { currentStock: 0, isLowStock: false, lowStockThreshold: 0, baseUnit: 'PRINT' };

describe('ItemStockStatus', () => {
  it('physical items use the stock badge', () => {
    const out = render(<ItemStockStatus item={{ ...base, itemType: 'PHYSICAL' }} />);
    expect(out.container.textContent).toBe('Out of Stock');
    const low = render(
      <ItemStockStatus item={{ ...base, itemType: 'PHYSICAL', currentStock: 2, isLowStock: true }} />
    );
    expect(low.container.textContent).toBe('Low Stock: 2 PRINT');
    const ok = render(<ItemStockStatus item={{ ...base, itemType: 'PHYSICAL', currentStock: 9 }} />);
    expect(ok.container.textContent).toBe('9 PRINT');
  });

  it('service with canMake 5 shows a green Can make badge', () => {
    const { getByTestId } = render(<ItemStockStatus item={{ ...base, itemType: 'SERVICE', canMake: 5 }} />);
    const el = getByTestId('can-make');
    expect(el.textContent).toBe('Can make 5 PRINT');
    expect(el.className).toContain('emerald');
  });

  it('service with canMake 0 shows a red Can\'t make badge', () => {
    const { getByTestId } = render(<ItemStockStatus item={{ ...base, itemType: 'SERVICE', canMake: 0 }} />);
    const el = getByTestId('can-make');
    expect(el.textContent).toBe("Can't make");
    expect(el.className).toContain('rose');
  });

  it('service without components shows a neutral dash', () => {
    const { getByTestId } = render(<ItemStockStatus item={{ ...base, itemType: 'SERVICE', canMake: null }} />);
    const el = getByTestId('can-make');
    expect(el.textContent).toBe('—');
    expect(el.className).toContain('slate');
  });

  it('a service with currentStock 0 never shows Out of Stock', () => {
    const { container } = render(<ItemStockStatus item={{ ...base, itemType: 'SERVICE', canMake: 3 }} />);
    expect(container.textContent).not.toContain('Out of Stock');
  });
});
