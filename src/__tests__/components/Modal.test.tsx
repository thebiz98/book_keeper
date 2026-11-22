import React, { useState } from 'react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, fireEvent } from '../testUtils';
import { press } from './saleFixtures';
import { Modal } from '@/components/ui/Modal';
import { ConfirmDialog } from '@/components/common/ConfirmDialog';
import { Drawer } from '@/components/ui/Drawer';
import { __resetHotkeys } from '@/hooks/useHotkeys';

describe('Modal', () => {
  beforeEach(() => __resetHotkeys());

  it('uses unique title ids and aria attributes', () => {
    render(
      <>
        <Modal isOpen onClose={vi.fn()} title="One" description="d1"><button>a</button></Modal>
        <Modal isOpen onClose={vi.fn()} title="Two"><button>b</button></Modal>
      </>
    );
    const dialogs = Array.from(document.querySelectorAll('[role="dialog"]'));
    expect(dialogs).toHaveLength(2);
    const ids = dialogs.map((d) => d.getAttribute('aria-labelledby'));
    expect(ids[0]).not.toBe(ids[1]);
    expect(document.getElementById(ids[0]!)?.textContent).toBe('One');
    expect(dialogs[0].getAttribute('aria-modal')).toBe('true');
    expect(dialogs[0].getAttribute('aria-describedby')).toBeTruthy();
  });

  it('moves focus into the dialog, traps Tab, and restores focus on close', () => {
    const Host: React.FC = () => {
      const [open, setOpen] = useState(false);
      return (
        <div>
          <button data-testid="opener" onClick={() => setOpen(true)}>open</button>
          <Modal isOpen={open} onClose={() => setOpen(false)} title="T">
            <input data-testid="first" />
            <button data-testid="last">ok</button>
          </Modal>
        </div>
      );
    };
    const { getByTestId } = render(<Host />);
    const opener = getByTestId('opener');
    opener.focus();
    fireEvent.click(opener);
    expect(document.activeElement).toBe(document.querySelector('[data-testid="first"]'));

    const last = document.querySelector<HTMLElement>('[data-testid="last"]')!;
    const dialog = document.querySelector('[role="dialog"]')!;
    last.focus();
    const tabEv = press(last, 'Tab');
    expect(tabEv.defaultPrevented).toBe(true); // wrapped instead of leaving the dialog
    expect(dialog.contains(document.activeElement)).toBe(true);
    expect(document.activeElement).toBe(dialog.querySelector('[data-modal-close]'));
    const shiftTab = press(document.activeElement!, 'Tab', { shiftKey: true });
    expect(shiftTab.defaultPrevented).toBe(true);
    expect(document.activeElement).toBe(last);

    press(window, 'Escape');
    expect(document.querySelector('[role="dialog"]')).toBeNull();
    expect(document.activeElement).toBe(opener);
  });

  it('does not refocus or re-run effects when the parent re-renders with fresh callbacks', () => {
    const Host: React.FC = () => {
      const [n, setN] = useState(0);
      return (
        <div>
          <button data-testid="bump" onClick={() => setN(n + 1)}>bump {n}</button>
          <Modal isOpen onClose={() => setN(-1)} defaultConfirmAction={() => setN((x) => x)} title="T">
            <input data-testid="field" />
          </Modal>
        </div>
      );
    };
    const { getByTestId } = render(<Host />);
    const field = document.querySelector<HTMLElement>('[data-testid="field"]')!;
    field.focus();
    fireEvent.click(getByTestId('bump'));
    fireEvent.click(getByTestId('bump'));
    expect(document.activeElement).toBe(field);
  });

  it('Enter confirms exactly once and not from inputs or buttons; Space on Cancel does not confirm', () => {
    const confirm = vi.fn();
    const cancel = vi.fn();
    render(<ConfirmDialog isOpen title="Sure?" message="m" onConfirm={confirm} onCancel={cancel} />);
    const dialog = document.querySelector<HTMLElement>('[role="dialog"]')!;
    expect(document.activeElement).toBe(dialog);
    press(dialog, 'Enter');
    expect(confirm).toHaveBeenCalledTimes(1);

    const cancelBtn = Array.from(dialog.querySelectorAll('button')).find((b) => b.textContent?.includes('Cancel'))!;
    cancelBtn.focus();
    press(cancelBtn, ' ');
    press(cancelBtn, 'Enter');
    expect(confirm).toHaveBeenCalledTimes(1);
  });

  it('Esc closes the top-most modal only', () => {
    const outer = vi.fn();
    const inner = vi.fn();
    render(
      <>
        <Modal isOpen onClose={outer} title="outer"><button>x</button></Modal>
        <Modal isOpen onClose={inner} title="inner"><button>y</button></Modal>
      </>
    );
    press(window, 'Escape');
    expect(inner).toHaveBeenCalledTimes(1);
    expect(outer).not.toHaveBeenCalled();
  });

  it('backdrop click can be disabled', () => {
    const onClose = vi.fn();
    render(<Modal isOpen onClose={onClose} title="T" closeOnBackdrop={false}><button>x</button></Modal>);
    const backdrop = document.querySelector('[role="dialog"]')!.parentElement!;
    fireEvent.click(backdrop);
    expect(onClose).not.toHaveBeenCalled();
  });

  it('Drawer traps focus, labels itself and closes on Esc', () => {
    const onClose = vi.fn();
    render(<Drawer isOpen onClose={onClose} title="Panel"><input data-testid="d1" /></Drawer>);
    const dialog = document.querySelector('[role="dialog"]')!;
    expect(dialog.getAttribute('aria-labelledby')).toBeTruthy();
    expect(dialog.contains(document.activeElement)).toBe(true);
    press(window, 'Escape');
    expect(onClose).toHaveBeenCalledTimes(1);
  });
});
