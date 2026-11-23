import { describe, it, expect, beforeEach, vi } from 'vitest';
import { act } from 'react';
import { render } from '../testUtils';
import { ConfirmDialog } from '@/components/common/ConfirmDialog';
import { setInput, click, buttonByText } from '../pages/helpers';

function enter(el: Element) {
  act(() => {
    el.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true, cancelable: true }));
  });
}

describe('ConfirmDialog requireText (FR-20 typed delete)', () => {
  beforeEach(() => {
    document.body.innerHTML = '';
  });

  const open = (onConfirm = vi.fn(), requireText?: string) => {
    render(
      <ConfirmDialog isOpen variant="danger" title="Delete SAL-1001?" message="Sure?" confirmText="Delete" requireText={requireText} onConfirm={onConfirm} onCancel={() => {}}>
        <p>inline-child</p>
      </ConfirmDialog>
    );
    return onConfirm;
  };

  it('without requireText it behaves as before and renders children', () => {
    const onConfirm = open(vi.fn());
    expect(document.querySelector('[data-testid="confirm-require-text"]')).toBeNull();
    expect(document.body.textContent).toContain('inline-child');
    click(buttonByText('Delete'));
    expect(onConfirm).toHaveBeenCalledTimes(1);
  });

  it('keeps Confirm disabled and Enter inert until the exact text is typed', () => {
    const onConfirm = open(vi.fn(), 'SAL-1001');
    const confirm = buttonByText('Delete') as HTMLButtonElement;
    expect(confirm.disabled).toBe(true);
    click(confirm);
    enter(document.querySelector('[role="dialog"]')!);
    const input = document.querySelector('[data-testid="confirm-require-text"]') as HTMLInputElement;
    enter(input);
    expect(onConfirm).not.toHaveBeenCalled();

    setInput(input, 'SAL-100');
    expect((buttonByText('Delete') as HTMLButtonElement).disabled).toBe(true);
    enter(input);
    expect(onConfirm).not.toHaveBeenCalled();

    setInput(input, '  SAL-1001 ');
    expect((buttonByText('Delete') as HTMLButtonElement).disabled).toBe(false);
    enter(input);
    expect(onConfirm).toHaveBeenCalledTimes(1);
    click(buttonByText('Delete'));
    expect(onConfirm).toHaveBeenCalledTimes(2);
  });

  it('Enter on the dialog body confirms once the text matches', () => {
    const onConfirm = open(vi.fn(), 'SAL-1001');
    setInput(document.querySelector('[data-testid="confirm-require-text"]')!, 'SAL-1001');
    enter(document.querySelector('[role="dialog"] p')!);
    expect(onConfirm).toHaveBeenCalledTimes(1);
  });
});
