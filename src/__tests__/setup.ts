import { expect, afterEach, vi } from 'vitest';

// ============================================================================
// Vitest DOM Environment Setup & Tauri IPC Mock
// Spec: docs/book_keeper_UI_LLD.md, TEST_INFRA.md §4
// ============================================================================

// 1. Mock window.__TAURI_INTERNALS__ and window.__TAURI__
const invokeMock = vi.fn(async (_cmd: string, _args?: unknown): Promise<unknown> => {
  return null;
});

const tauriInternalsMock = {
  invoke: invokeMock,
  metadata: {
    currentWindow: { label: 'main' },
  },
  transformCallback: vi.fn(),
};

Object.defineProperty(window, '__TAURI_INTERNALS__', {
  value: tauriInternalsMock,
  writable: true,
  configurable: true,
});

Object.defineProperty(window, '__TAURI__', {
  value: {
    core: {
      invoke: invokeMock,
    },
  },
  writable: true,
  configurable: true,
});

// 2. Mock localStorage
const createLocalStorageMock = () => {
  let store: Record<string, string> = {};
  return {
    getItem: vi.fn((key: string): string | null => store[key] ?? null),
    setItem: vi.fn((key: string, value: string) => {
      store[key] = String(value);
    }),
    removeItem: vi.fn((key: string) => {
      delete store[key];
    }),
    clear: vi.fn(() => {
      store = {};
    }),
    get length() {
      return Object.keys(store).length;
    },
    key: vi.fn((index: number): string | null => Object.keys(store)[index] ?? null),
  };
};

const localStorageMock = createLocalStorageMock();
Object.defineProperty(window, 'localStorage', {
  value: localStorageMock,
  writable: true,
  configurable: true,
});

// 3. Mock matchMedia for Tailwind dark mode and system theme queries
Object.defineProperty(window, 'matchMedia', {
  writable: true,
  configurable: true,
  value: vi.fn().mockImplementation((query: string) => ({
    matches: false,
    media: query,
    onchange: null,
    addListener: vi.fn(),
    removeListener: vi.fn(),
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
    dispatchEvent: vi.fn(),
  })),
});

// 4. Custom DOM Matchers for Vitest (compatible with @testing-library/jest-dom)
interface CustomMatchers<R = unknown> {
  toBeInTheDocument(): R;
  toHaveClass(...classNames: string[]): R;
  toHaveTextContent(text: string | RegExp): R;
  toBeVisible(): R;
  toBeDisabled(): R;
  toHaveValue(value: string | number | string[]): R;
}

declare module 'vitest' {
  interface Assertion<T = any> extends CustomMatchers<T> {}
  interface AsymmetricMatchersContaining extends CustomMatchers {}
}

expect.extend({
  toBeInTheDocument(received: Element | null | undefined) {
    const pass = received !== null && received !== undefined && (
      received.ownerDocument ? received.ownerDocument.contains(received) : false
    );
    return {
      pass,
      message: () =>
        pass
          ? `Expected element not to be in document, but it was found`
          : `Expected element to be in document, but it was not found`,
    };
  },

  toHaveClass(received: Element | null | undefined, ...expectedClasses: string[]) {
    if (!received) {
      return {
        pass: false,
        message: () => `Expected element to have class(es) "${expectedClasses.join(' ')}", but element is null/undefined`,
      };
    }
    const classList = Array.from(received.classList);
    const missing = expectedClasses.filter((c) => !classList.includes(c));
    const pass = missing.length === 0;
    return {
      pass,
      message: () =>
        pass
          ? `Expected element not to have class(es) "${expectedClasses.join(' ')}", but it has: "${classList.join(' ')}"`
          : `Expected element to have class(es) "${expectedClasses.join(' ')}", but missing: "${missing.join(' ')}". Present: "${classList.join(' ')}"`,
    };
  },

  toHaveTextContent(received: Element | null | undefined, expectedText: string | RegExp) {
    if (!received) {
      return {
        pass: false,
        message: () => `Expected element to have text content "${expectedText}", but element is null/undefined`,
      };
    }
    const text = received.textContent ?? '';
    const pass = typeof expectedText === 'string'
      ? text.includes(expectedText)
      : expectedText.test(text);
    return {
      pass,
      message: () =>
        pass
          ? `Expected element text not to match "${expectedText}", but got "${text}"`
          : `Expected element text to match "${expectedText}", but got "${text}"`,
    };
  },

  toBeVisible(received: HTMLElement | null | undefined) {
    if (!received) {
      return {
        pass: false,
        message: () => `Expected element to be visible, but element is null/undefined`,
      };
    }
    const style = window.getComputedStyle ? window.getComputedStyle(received) : (received.style as CSSStyleDeclaration);
    const isHidden =
      style?.display === 'none' ||
      style?.visibility === 'hidden' ||
      style?.opacity === '0' ||
      received.hidden;
    const pass = !isHidden;
    return {
      pass,
      message: () =>
        pass
          ? `Expected element to be hidden, but it is visible`
          : `Expected element to be visible, but it is hidden`,
    };
  },

  toBeDisabled(received: (Element & { disabled?: boolean }) | null | undefined) {
    if (!received) {
      return {
        pass: false,
        message: () => `Expected element to be disabled, but element is null/undefined`,
      };
    }
    const pass = received.disabled === true || received.getAttribute('aria-disabled') === 'true';
    return {
      pass,
      message: () =>
        pass
          ? `Expected element not to be disabled, but disabled is true`
          : `Expected element to be disabled, but disabled is false`,
    };
  },

  toHaveValue(received: (Element & { value?: unknown }) | null | undefined, expectedValue: string | number | string[]) {
    if (!received) {
      return {
        pass: false,
        message: () => `Expected element to have value "${expectedValue}", but element is null/undefined`,
      };
    }
    const actualValue = (received as any).value;
    const pass = String(actualValue) === String(expectedValue);
    return {
      pass,
      message: () =>
        pass
          ? `Expected element not to have value "${expectedValue}", but got "${actualValue}"`
          : `Expected element to have value "${expectedValue}", but got "${actualValue}"`,
    };
  },
});

// 5. Cleanup after each test
afterEach(() => {
  vi.clearAllMocks();
  localStorage.clear();
  if (document.documentElement) {
    document.documentElement.className = '';
  }
  document.body.innerHTML = '';
});

export { invokeMock, localStorageMock, tauriInternalsMock };
