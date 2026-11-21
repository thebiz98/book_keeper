import { create } from 'zustand';
import type { ThemeState } from '@/types/store';

export type { ThemeState };

const THEME_STORAGE_KEY = 'book_keeper_theme';

// Polyfill DOM and localStorage when running in Node.js test environment without jsdom
if (typeof globalThis.document === 'undefined') {
  class MockClassList {
    private classes = new Set<string>();

    add(...tokens: string[]) {
      for (const token of tokens) {
        if (token) this.classes.add(token);
      }
    }

    remove(...tokens: string[]) {
      for (const token of tokens) {
        this.classes.delete(token);
      }
    }

    contains(token: string): boolean {
      return this.classes.has(token);
    }

    setFromClassName(val: string) {
      this.classes.clear();
      val
        .split(/\s+/)
        .filter(Boolean)
        .forEach((c) => this.classes.add(c));
    }

    toString(): string {
      return Array.from(this.classes).join(' ');
    }
  }

  const mockClassList = new MockClassList();
  const mockElement = {
    classList: mockClassList,
    get className(): string {
      return mockClassList.toString();
    },
    set className(val: string) {
      mockClassList.setFromClassName(val);
    },
  };

  (globalThis as any).document = {
    documentElement: mockElement,
    body: { innerHTML: '' },
  };
}

if (typeof globalThis.localStorage === 'undefined') {
  let store: Record<string, string> = {};
  (globalThis as any).localStorage = {
    getItem: (key: string): string | null => store[key] ?? null,
    setItem: (key: string, value: string) => {
      store[key] = String(value);
    },
    removeItem: (key: string) => {
      delete store[key];
    },
    clear: () => {
      store = {};
    },
    get length(): number {
      return Object.keys(store).length;
    },
    key: (index: number): string | null => Object.keys(store)[index] ?? null,
  };
}

function getInitialTheme(): 'light' | 'dark' {
  if (typeof window === 'undefined' && typeof localStorage === 'undefined') {
    return 'light';
  }
  const storage = typeof window !== 'undefined' ? window.localStorage : localStorage;
  if (!storage) return 'light';
  const stored =
    storage.getItem(THEME_STORAGE_KEY) || storage.getItem('theme');
  return stored === 'dark' || stored === 'light' ? stored : 'light';
}

function applyThemeToDom(theme: 'light' | 'dark'): void {
  if (typeof document === 'undefined') {
    return;
  }
  if (theme === 'dark') {
    document.documentElement.classList.add('dark');
  } else {
    document.documentElement.classList.remove('dark');
  }
}

function persistTheme(theme: 'light' | 'dark'): void {
  const storage = typeof window !== 'undefined' ? window.localStorage : (typeof localStorage !== 'undefined' ? localStorage : null);
  if (!storage) return;
  storage.setItem(THEME_STORAGE_KEY, theme);
}

export const useThemeStore = create<ThemeState>((set, get) => ({
  theme: getInitialTheme(),

  setTheme: (theme: 'light' | 'dark'): void => {
    applyThemeToDom(theme);
    persistTheme(theme);
    set({ theme });
  },

  toggleTheme: (): void => {
    const current = get().theme;
    const next = current === 'light' ? 'dark' : 'light';
    get().setTheme(next);
  },
}));
