import { useEffect } from 'react';

export interface GlobalHotkeyHandlers {
  /** Map of key spec ("F1", "Ctrl+B") to handler. */
  [spec: string]: () => void;
}

function specFor(e: KeyboardEvent): string {
  const parts: string[] = [];
  if (e.ctrlKey || e.metaKey) parts.push('Ctrl');
  if (e.altKey) parts.push('Alt');
  if (e.shiftKey) parts.push('Shift');
  parts.push(e.key.length === 1 ? e.key.toUpperCase() : e.key);
  return parts.join('+');
}

/**
 * Window-level hotkey engine. Function keys and Ctrl/Alt combos fire even
 * while an input is focused (per UI LLD 4.1).
 */
export function useGlobalHotkeys(handlers: GlobalHotkeyHandlers): void {
  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      const handler = handlers[specFor(e)];
      if (handler) {
        e.preventDefault();
        handler();
      }
    };
    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, [handlers]);
}
