import { describe, it, expect, vi } from 'vitest';
import { act } from 'react';
import { render } from '../testUtils';
import { FormActions, enterToSubmit } from '@/components/ui/FormActions';
import { press } from './saleFixtures';

const btn = (re: RegExp) =>
  Array.from(document.querySelectorAll('button')).find((b) => re.test(b.textContent ?? ''))!;
const click = (el: Element) =>
  act(() => {
    el.dispatchEvent(new MouseEvent('click', { bubbles: true, cancelable: true }));
  });

describe('FormActions', () => {
  it('Save and Cancel clicks call their handlers', () => {
    const onSave = vi.fn();
    const onCancel = vi.fn();
    render(<FormActions onSave={onSave} onCancel={onCancel} />);
    click(btn(/^Save/));
    click(btn(/^Cancel/));
    expect(onSave).toHaveBeenCalledTimes(1);
    expect(onCancel).toHaveBeenCalledTimes(1);
  });

  it('canSave=false disables Save and a click does nothing', () => {
    const onSave = vi.fn();
    render(<FormActions onSave={onSave} onCancel={() => {}} canSave={false} />);
    const save = btn(/^Save/);
    expect(save.disabled).toBe(true);
    click(save);
    expect(onSave).not.toHaveBeenCalled();
  });

  it('isSubmitting disables both buttons', () => {
    render(<FormActions onSave={() => {}} onCancel={() => {}} isSubmitting />);
    const buttons = Array.from(document.querySelectorAll('button'));
    expect(buttons).toHaveLength(2);
    buttons.forEach((b) => expect(b.disabled).toBe(true));
  });
});

describe('enterToSubmit', () => {
  function setup(submit: () => void) {
    render(
      <div onKeyDown={enterToSubmit(submit)}>
        <input aria-label="a" />
        <input aria-label="scan" data-enter-submit="off" />
        <button>b</button>
      </div>,
    );
    return {
      a: document.querySelector<HTMLInputElement>('input[aria-label="a"]')!,
      scan: document.querySelector<HTMLInputElement>('input[aria-label="scan"]')!,
      b: document.querySelector<HTMLButtonElement>('button')!,
    };
  }

  it('Enter and Ctrl+Enter on an input submit', () => {
    const submit = vi.fn();
    const { a } = setup(submit);
    press(a, 'Enter');
    press(a, 'Enter', { ctrlKey: true });
    expect(submit).toHaveBeenCalledTimes(2);
  });

  it('Shift+Enter does not submit', () => {
    const submit = vi.fn();
    const { a } = setup(submit);
    press(a, 'Enter', { shiftKey: true });
    expect(submit).not.toHaveBeenCalled();
  });

  it('Enter on data-enter-submit="off" does not submit', () => {
    const submit = vi.fn();
    const { scan } = setup(submit);
    const ev = press(scan, 'Enter');
    expect(submit).not.toHaveBeenCalled();
    expect(ev.defaultPrevented).toBe(true);
  });

  it('Enter on a button does not submit', () => {
    const submit = vi.fn();
    const { b } = setup(submit);
    press(b, 'Enter');
    expect(submit).not.toHaveBeenCalled();
  });
});
