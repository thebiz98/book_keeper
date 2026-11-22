import React from 'react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render } from '../testUtils';
import { press } from '../components/saleFixtures';
import {
  useHotkey,
  useModalLayer,
  useEscapeLayer,
  useBusyLock,
  __resetHotkeys,
  normalizeCombo,
  eventCombo,
  HOTKEYS,
  hotkeyBadge,
} from '@/hooks/useHotkeys';

const Probe: React.FC<{
  nav?: () => void;
  page?: () => void;
  modalKey?: () => void;
  enter?: () => void;
  plain?: () => void;
  modalOpen?: boolean;
  dropdownOpen?: boolean;
  busy?: boolean;
  onModalEsc?: () => void;
  onDropdownEsc?: () => void;
  pageEsc?: () => void;
}> = ({ nav, page, modalKey, enter, plain, modalOpen = false, dropdownOpen = false, busy = false, onModalEsc, onDropdownEsc, pageEsc }) => {
  useHotkey('F2', () => nav?.(), { scope: 'global' });
  useHotkey('Ctrl+Enter', () => page?.(), { scope: 'page' });
  useHotkey('Alt+C', () => modalKey?.(), { scope: 'modal' });
  useHotkey('Enter', () => enter?.(), { scope: 'page' });
  useHotkey('Delete', () => plain?.(), { scope: 'page' });
  useHotkey('Escape', () => pageEsc?.(), { scope: 'page', allowInInputs: true });
  useModalLayer(() => onModalEsc?.(), modalOpen);
  useEscapeLayer(() => onDropdownEsc?.(), dropdownOpen);
  useBusyLock(busy);
  return <input data-testid="inp" />;
};

describe('hotkey registry', () => {
  beforeEach(() => __resetHotkeys());

  it('normalises combos and reads them from events (letters via code)', () => {
    expect(normalizeCombo('ctrl+enter')).toBe('Ctrl+Enter');
    expect(normalizeCombo('esc')).toBe('Escape');
    expect(eventCombo(new KeyboardEvent('keydown', { key: '∫', code: 'KeyB', altKey: true }))).toBe('Alt+B');
    expect(eventCombo(new KeyboardEvent('keydown', { key: 'F4' }))).toBe('F4');
  });

  it('exposes the DD-19 map for badges', () => {
    expect(hotkeyBadge('lookup')).toBe('F4');
    expect(HOTKEYS.lookup.combos).toContain('Ctrl+F');
    expect(hotkeyBadge('back')).toBe('Esc');
    expect(HOTKEYS.newCustomer.combos).toEqual(['Alt+C']);
    expect(HOTKEYS.newVendor.combos).toEqual(['Alt+V']);
  });

  it('fires a registered global hotkey and ignores e.repeat', () => {
    const nav = vi.fn();
    render(<Probe nav={nav} />);
    press(window, 'F2');
    expect(nav).toHaveBeenCalledTimes(1);
    press(window, 'F2', { repeat: true });
    expect(nav).toHaveBeenCalledTimes(1);
  });

  it('blocks global and page hotkeys while a modal is open, allows modal-scope ones', () => {
    const nav = vi.fn();
    const page = vi.fn();
    const modalKey = vi.fn();
    const { rerender } = render(<Probe nav={nav} page={page} modalKey={modalKey} />);
    press(window, 'Alt+C', { altKey: true, code: 'KeyC' });
    expect(modalKey).not.toHaveBeenCalled(); // modal scope needs an open modal

    rerender(<Probe nav={nav} page={page} modalKey={modalKey} modalOpen />);
    press(window, 'F2');
    press(window, 'Enter', { ctrlKey: true });
    expect(nav).not.toHaveBeenCalled();
    expect(page).not.toHaveBeenCalled();
    press(window, 'c', { altKey: true, code: 'KeyC' });
    expect(modalKey).toHaveBeenCalledTimes(1);

    rerender(<Probe nav={nav} page={page} modalKey={modalKey} />);
    press(window, 'F2');
    expect(nav).toHaveBeenCalledTimes(1);
  });

  it('blocks navigation while a submit is in flight', () => {
    const nav = vi.fn();
    const page = vi.fn();
    const { rerender } = render(<Probe nav={nav} page={page} busy />);
    press(window, 'F2');
    press(window, 'Enter', { ctrlKey: true });
    expect(nav).not.toHaveBeenCalled();
    expect(page).not.toHaveBeenCalled();
    rerender(<Probe nav={nav} page={page} />);
    press(window, 'Enter', { ctrlKey: true });
    expect(page).toHaveBeenCalledTimes(1);
  });

  it('Esc goes to the top-most layer: dropdown, then modal, then the page', () => {
    const onModalEsc = vi.fn();
    const onDropdownEsc = vi.fn();
    const pageEsc = vi.fn();
    const { rerender } = render(<Probe pageEsc={pageEsc} onModalEsc={onModalEsc} onDropdownEsc={onDropdownEsc} modalOpen />);
    rerender(<Probe pageEsc={pageEsc} onModalEsc={onModalEsc} onDropdownEsc={onDropdownEsc} modalOpen dropdownOpen />);

    press(window, 'Escape');
    expect(onDropdownEsc).toHaveBeenCalledTimes(1);
    expect(onModalEsc).not.toHaveBeenCalled();

    rerender(<Probe pageEsc={pageEsc} onModalEsc={onModalEsc} onDropdownEsc={onDropdownEsc} modalOpen />);
    press(window, 'Escape');
    expect(onModalEsc).toHaveBeenCalledTimes(1);
    expect(pageEsc).not.toHaveBeenCalled();

    rerender(<Probe pageEsc={pageEsc} onModalEsc={onModalEsc} onDropdownEsc={onDropdownEsc} />);
    press(window, 'Escape');
    expect(pageEsc).toHaveBeenCalledTimes(1);
  });

  it('plain keys do not fire inside inputs, Ctrl/Alt/F-keys do', () => {
    const plain = vi.fn();
    const nav = vi.fn();
    const page = vi.fn();
    const { getByTestId } = render(<Probe plain={plain} nav={nav} page={page} />);
    const input = getByTestId('inp');
    press(input, 'Delete');
    expect(plain).not.toHaveBeenCalled();
    press(input, 'F2');
    press(input, 'Enter', { ctrlKey: true });
    expect(nav).toHaveBeenCalledTimes(1);
    expect(page).toHaveBeenCalledTimes(1);
    press(window, 'Delete');
    expect(plain).toHaveBeenCalledTimes(1);
  });

  it('always preventDefaults F5 and Ctrl+F even with no handler', () => {
    render(<Probe />);
    expect(press(window, 'F5').defaultPrevented).toBe(true);
    expect(press(window, 'f', { ctrlKey: true, code: 'KeyF' }).defaultPrevented).toBe(true);
  });

  it('a handler change re-renders without re-registering (latest handler wins)', () => {
    const a = vi.fn();
    const b = vi.fn();
    const { rerender } = render(<Probe nav={a} />);
    rerender(<Probe nav={b} />);
    press(window, 'F2');
    expect(a).not.toHaveBeenCalled();
    expect(b).toHaveBeenCalledTimes(1);
  });

  it('unregisters on unmount', () => {
    const nav = vi.fn();
    const { unmount } = render(<Probe nav={nav} />);
    unmount();
    press(window, 'F2');
    expect(nav).not.toHaveBeenCalled();
  });
});
