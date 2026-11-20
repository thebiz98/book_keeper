import React, { act } from 'react';
import { createRoot, Root } from 'react-dom/client';
import { afterEach } from 'vitest';

// ============================================================================
// Genuine React 19 Test Render & Event Harness
// Compatible with standard React Testing Library API conventions
// ============================================================================

export interface RenderResult {
  container: HTMLDivElement;
  baseElement: HTMLElement;
  unmount: () => void;
  rerender: (ui: React.ReactElement) => void;
  getByText: (text: string | RegExp) => HTMLElement;
  queryByText: (text: string | RegExp) => HTMLElement | null;
  getByRole: (role: string, options?: { name?: string | RegExp }) => HTMLElement;
  getByPlaceholderText: (text: string | RegExp) => HTMLElement;
  getByLabelText: (text: string | RegExp) => HTMLElement;
  getByTestId: (testId: string) => HTMLElement;
}

const activeRoots: Array<{ root: Root; container: HTMLDivElement }> = [];

export function render(ui: React.ReactElement): RenderResult {
  const container = document.createElement('div');
  document.body.appendChild(container);
  const root = createRoot(container);
  activeRoots.push({ root, container });

  act(() => {
    root.render(ui);
  });

  const getByText = (text: string | RegExp): HTMLElement => {
    const el = queryByText(text);
    if (!el) {
      throw new Error(`Unable to find element with text matching: ${text}\nIn container:\n${container.innerHTML}`);
    }
    return el;
  };

  const queryByText = (text: string | RegExp): HTMLElement | null => {
    const all = container.querySelectorAll('*');
    for (const el of Array.from(all)) {
      const content = el.textContent ?? '';
      if (typeof text === 'string') {
        if (content.trim() === text || content.includes(text)) {
          // If a leaf element or directly contains text
          return el as HTMLElement;
        }
      } else if (text.test(content)) {
        return el as HTMLElement;
      }
    }
    return null;
  };

  const getByPlaceholderText = (text: string | RegExp): HTMLElement => {
    const inputs = container.querySelectorAll('input, textarea');
    for (const el of Array.from(inputs)) {
      const ph = (el as HTMLInputElement).placeholder || '';
      if (typeof text === 'string') {
        if (ph.includes(text)) return el as HTMLElement;
      } else if (text.test(ph)) {
        return el as HTMLElement;
      }
    }
    throw new Error(`Unable to find element with placeholder matching: ${text}`);
  };

  const getByLabelText = (text: string | RegExp): HTMLElement => {
    const labels = container.querySelectorAll('label');
    for (const label of Array.from(labels)) {
      const content = label.textContent ?? '';
      const match = typeof text === 'string' ? content.includes(text) : text.test(content);
      if (match) {
        const htmlFor = label.getAttribute('for');
        if (htmlFor) {
          const el = document.getElementById(htmlFor);
          if (el) return el;
        }
        const input = label.querySelector('input, select, textarea');
        if (input) return input as HTMLElement;
      }
    }
    throw new Error(`Unable to find element with label matching: ${text}`);
  };

  const getByRole = (role: string, options?: { name?: string | RegExp }): HTMLElement => {
    let selector = role;
    if (role === 'button') selector = 'button, [role="button"], input[type="button"], input[type="submit"]';
    else if (role === 'textbox') selector = 'input[type="text"], input:not([type]), textarea, [role="textbox"]';
    else if (role === 'combobox') selector = 'select, [role="combobox"]';
    else if (role === 'dialog') selector = 'dialog, [role="dialog"]';

    const elements = container.querySelectorAll(selector);
    for (const el of Array.from(elements)) {
      if (options?.name) {
        const text = el.textContent || el.getAttribute('aria-label') || (el as HTMLInputElement).value || '';
        const match = typeof options.name === 'string' ? text.includes(options.name) : options.name.test(text);
        if (match) return el as HTMLElement;
      } else {
        return el as HTMLElement;
      }
    }
    throw new Error(`Unable to find element with role "${role}"${options?.name ? ` and name matching "${options.name}"` : ''}`);
  };

  const getByTestId = (testId: string): HTMLElement => {
    const el = container.querySelector(`[data-testid="${testId}"]`);
    if (!el) {
      throw new Error(`Unable to find element with data-testid="${testId}"`);
    }
    return el as HTMLElement;
  };

  const unmount = () => {
    act(() => {
      root.unmount();
    });
    if (container.parentNode) {
      container.parentNode.removeChild(container);
    }
  };

  const rerender = (newUi: React.ReactElement) => {
    act(() => {
      root.render(newUi);
    });
  };

  return {
    container,
    baseElement: document.body,
    unmount,
    rerender,
    getByText,
    queryByText,
    getByRole,
    getByPlaceholderText,
    getByLabelText,
    getByTestId,
  };
}

export const fireEvent = {
  click: (element: Element) => {
    act(() => {
      element.dispatchEvent(new MouseEvent('click', { bubbles: true, cancelable: true }));
    });
  },

  change: (element: Element, eventInit: { target: { value: string | number } }) => {
    act(() => {
      (element as any).value = eventInit.target.value;
      element.dispatchEvent(new Event('input', { bubbles: true, cancelable: true }));
      element.dispatchEvent(new Event('change', { bubbles: true, cancelable: true }));
    });
  },

  keyDown: (element: Element, init: KeyboardEventInit) => {
    act(() => {
      element.dispatchEvent(new KeyboardEvent('keydown', { bubbles: true, cancelable: true, ...init }));
    });
  },

  keyUp: (element: Element, init: KeyboardEventInit) => {
    act(() => {
      element.dispatchEvent(new KeyboardEvent('keyup', { bubbles: true, cancelable: true, ...init }));
    });
  },

  submit: (element: Element) => {
    act(() => {
      element.dispatchEvent(new Event('submit', { bubbles: true, cancelable: true }));
    });
  },

  focus: (element: Element) => {
    act(() => {
      (element as HTMLElement).focus();
      element.dispatchEvent(new FocusEvent('focus', { bubbles: true }));
    });
  },

  blur: (element: Element) => {
    act(() => {
      (element as HTMLElement).blur();
      element.dispatchEvent(new FocusEvent('blur', { bubbles: true }));
    });
  },
};

afterEach(() => {
  while (activeRoots.length > 0) {
    const item = activeRoots.pop();
    if (item) {
      try {
        act(() => {
          item.root.unmount();
        });
      } catch {
        // ignore unmount errors on test teardown
      }
      if (item.container.parentNode) {
        item.container.parentNode.removeChild(item.container);
      }
    }
  }
});
