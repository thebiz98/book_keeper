import { create } from 'zustand';
import type { ThemeState } from '@/types/store';

export type { ThemeState };

const THEME_STORAGE_KEY = 'book_keeper_theme';

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
