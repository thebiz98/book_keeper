import { useEffect, useRef } from 'react';
import type { RefObject } from 'react';

const FOCUSABLE =
  'a[href], button:not([disabled]), input:not([disabled]):not([type="hidden"]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';

export function getFocusable(root: HTMLElement | null): HTMLElement[] {
  if (!root) return [];
  return Array.from(root.querySelectorAll<HTMLElement>(FOCUSABLE)).filter(
    (el) => el.getAttribute('aria-hidden') !== 'true'
  );
}

const trapStack: object[] = [];

/**
 * Trap Tab focus inside `containerRef` while `active`, move focus in on open
 * and restore it to the previously focused element on close. The effect depends
 * only on `active`, so a parent re-render never steals or restores focus.
 */
export function useFocusTrap(
  containerRef: RefObject<HTMLElement | null>,
  active: boolean,
  initialFocusRef?: RefObject<HTMLElement | null>
): void {
  const initialRef = useRef(initialFocusRef);
  initialRef.current = initialFocusRef;

  useEffect(() => {
    if (!active) return;
    const container = containerRef.current;
    const prev = document.activeElement as HTMLElement | null;

    if (container && !container.contains(document.activeElement)) {
      // Prefer the first control in the body (marked `data-modal-body`), never the
      // header close button or footer actions, so Enter can reach the confirm action.
      const body = container.querySelector<HTMLElement>('[data-modal-body]') ?? container;
      const target = initialRef.current?.current ?? getFocusable(body)[0] ?? container;
      target.focus();
    }

    const token = {};
    trapStack.push(token);
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key !== 'Tab' || !container) return;
      if (trapStack[trapStack.length - 1] !== token) return;
      const items = getFocusable(container);
      if (items.length === 0) {
        e.preventDefault();
        container.focus();
        return;
      }
      const first = items[0];
      const last = items[items.length - 1];
      const current = document.activeElement as HTMLElement | null;
      if (e.shiftKey && (current === first || current === container || !container.contains(current))) {
        e.preventDefault();
        last.focus();
      } else if (!e.shiftKey && (current === last || !container.contains(current))) {
        e.preventDefault();
        first.focus();
      }
    };
    document.addEventListener('keydown', onKeyDown);

    return () => {
      document.removeEventListener('keydown', onKeyDown);
      const i = trapStack.indexOf(token);
      if (i >= 0) trapStack.splice(i, 1);
      if (prev && prev.isConnected && typeof prev.focus === 'function') prev.focus();
    };
  }, [active, containerRef]);
}
