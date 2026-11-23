import { describe, it, expect, vi } from 'vitest';
import { render } from '../testUtils';
import { byText, click, flush, installApi, key, typeInto } from './testHelpers';
import { BundleCreationModal } from '@/components/domain/BundleCreationModal';

const suggestions = [
  { bundleName: 'Box', unitsPerBundle: 50, usageCount: 9 },
  { bundleName: 'Dozen', unitsPerBundle: 12, usageCount: 5 },
  { bundleName: 'Bundle of Ten', unitsPerBundle: 10, usageCount: 1 },
];
const saved = { id: 3, itemId: 1, bundleName: 'Box', unitsPerBundle: 50, mrp: 80, defaultBundlePrice: 70, barcode: null };

function mount(over: Record<string, unknown> = {}, props: Record<string, unknown> = {}) {
  const api = installApi({
    getBundleSuggestions: vi.fn().mockResolvedValue(suggestions),
    createBundle: vi.fn().mockResolvedValue(saved),
    ...over,
  });
  const onBundleCreated = vi.fn();
  const onClose = vi.fn();
  const r = render(
    <BundleCreationModal isOpen onClose={onClose} onBundleCreated={onBundleCreated} initialItemId={1} {...props} />
  );
  return { api, onBundleCreated, onClose, ...r };
}

const nameInput = (c: HTMLElement) => c.querySelector('input[placeholder^="e.g. Box"]') as HTMLInputElement;
const multInput = (c: HTMLElement) => c.querySelector('input[type="number"][min="2"]') as HTMLInputElement;

describe('BundleCreationModal', () => {
  it('filters dynamic suggestions as the user types and fills name + multiplier, focusing price', async () => {
    const { container } = mount();
    await flush(60);
    typeInto(nameInput(container), 'doz');
    await flush();
    const opts = Array.from(container.querySelectorAll('[role="option"]'));
    expect(opts.map((o) => o.textContent)).toEqual(['Dozen(12 PCS)']);

    act_mouseDown(opts[0]);
    await flush(10);
    expect(nameInput(container).value).toBe('Dozen');
    expect(multInput(container).value).toBe('12');
    const active = document.activeElement as HTMLInputElement;
    expect(document.querySelector(`label[for="${active.id}"]`)?.textContent).toContain('Bundle Sale Price');
  });

  it('requires multiplier > 1 and does not call the API', async () => {
    const { container, api } = mount();
    await flush(60);
    typeInto(nameInput(container), 'Single');
    typeInto(multInput(container), '1');
    click(byText(container, 'button', /Create Bundle/));
    await flush();
    expect(api.createBundle).not.toHaveBeenCalled();
    expect(container.textContent).toContain('Multiplier must be greater than 1');
  });

  it('Ctrl+Enter saves; MRP, price and barcode go through; full ItemBundle is returned', async () => {
    const { container, api, onBundleCreated, onClose } = mount();
    await flush(60);
    typeInto(nameInput(container), 'Box');
    typeInto(multInput(container), '50');
    const nums = container.querySelectorAll<HTMLInputElement>('input[type="number"]');
    typeInto(nums[1], '70');
    typeInto(nums[2], '80');
    typeInto(container.querySelector('input[placeholder^="Scan outer"]') as HTMLInputElement, ' 123 ');
    key(nameInput(container), { key: 'Enter', ctrlKey: true });
    await flush();
    expect(api.createBundle).toHaveBeenCalledWith({
      itemId: 1,
      bundleName: 'Box',
      unitsPerBundle: 50,
      defaultBundlePrice: 70,
      mrp: 80,
      barcode: '123',
    });
    expect(onBundleCreated).toHaveBeenCalledWith(saved);
    expect(onClose).toHaveBeenCalled();
  });

  it('reads the backend ValidationError message and keeps the modal open', async () => {
    const { container, onClose } = mount({
      createBundle: vi.fn().mockRejectedValue({
        type: 'ValidationError',
        details: { field: 'barcode', message: 'Barcode already used by item X' },
      }),
    });
    await flush(60);
    typeInto(nameInput(container), 'Box');
    typeInto(multInput(container), '50');
    click(byText(container, 'button', /Create Bundle/));
    await flush();
    expect(container.textContent).toContain('Barcode already used by item X');
    expect(onClose).not.toHaveBeenCalled();
  });

  it('plain Enter in the multiplier input saves', async () => {
    const { container, api } = mount();
    await flush(60);
    typeInto(nameInput(container), 'Box');
    typeInto(multInput(container), '50');
    key(multInput(container), { key: 'Enter' });
    await flush();
    expect(api.createBundle).toHaveBeenCalledTimes(1);
  });

  it('Enter in the outer barcode input (scanner) does not save', async () => {
    const { container, api } = mount();
    await flush(60);
    typeInto(nameInput(container), 'Box');
    typeInto(multInput(container), '50');
    const barcode = container.querySelector('input[placeholder^="Scan outer"]') as HTMLInputElement;
    typeInto(barcode, '8901234567890');
    key(barcode, { key: 'Enter' });
    await flush();
    expect(api.createBundle).not.toHaveBeenCalled();
  });

  it('Enter in the name field with suggestions open applies the suggestion and does not save', async () => {
    const { container, api } = mount();
    await flush(60);
    typeInto(nameInput(container), 'doz');
    await flush();
    expect(container.querySelector('[role="option"]')).not.toBeNull();
    key(nameInput(container), { key: 'Enter' });
    await flush(10);
    expect(api.createBundle).not.toHaveBeenCalled();
    expect(nameInput(container).value).toBe('Dozen');
    expect(multInput(container).value).toBe('12');
  });

  it('Cancel calls onClose', async () => {
    const { container, onClose } = mount();
    await flush(60);
    click(byText(container, 'button', /^Cancel/));
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it('Create Bundle is disabled while the name is empty', async () => {
    const { container } = mount();
    await flush(60);
    const btn = byText(container, 'button', /Create Bundle/) as HTMLButtonElement;
    expect(btn.disabled).toBe(true);
    typeInto(nameInput(container), 'Box');
    expect((byText(container, 'button', /Create Bundle/) as HTMLButtonElement).disabled).toBe(false);
  });
});

function act_mouseDown(el: Element) {
  el.dispatchEvent(new MouseEvent('mousedown', { bubbles: true, cancelable: true }));
}
