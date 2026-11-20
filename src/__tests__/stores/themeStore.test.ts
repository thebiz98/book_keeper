import { describe, it, expect, beforeEach } from 'vitest';
import { useThemeStore } from '@/store/useThemeStore';

// ============================================================================
// Test Suite: useThemeStore
// Spec: docs/book_keeper_UI_LLD.md §3, §4.1, PROJECT.md F30
// ============================================================================

describe('useThemeStore - Dual Theme Engine & Tailwind Class Toggling', () => {
  beforeEach(() => {
    // Reset document class and localStorage
    document.documentElement.className = '';
    localStorage.clear();
    useThemeStore.setState({ theme: 'light' });
  });

  describe('1. Theme Toggling', () => {
    it('TC-THM-001: defaults to light mode on initial load when no stored preference exists', () => {
      const state = useThemeStore.getState();
      expect(state.theme).toBe('light');
    });

    it('TC-THM-002: toggleTheme transitions from light to dark', () => {
      const store = useThemeStore.getState();
      expect(store.theme).toBe('light');

      store.toggleTheme();

      const state = useThemeStore.getState();
      expect(state.theme).toBe('dark');
    });

    it('TC-THM-003: toggleTheme transitions from dark back to light', () => {
      const store = useThemeStore.getState();
      store.setTheme('dark');
      expect(useThemeStore.getState().theme).toBe('dark');

      store.toggleTheme();

      expect(useThemeStore.getState().theme).toBe('light');
    });
  });

  describe('2. DOM Document Element Class Toggling', () => {
    it('TC-THM-004: adds "dark" class to document.documentElement when switching to dark mode', () => {
      const store = useThemeStore.getState();
      expect(document.documentElement.classList.contains('dark')).toBe(false);

      store.setTheme('dark');

      expect(document.documentElement.classList.contains('dark')).toBe(true);
    });

    it('TC-THM-005: removes "dark" class from document.documentElement when switching to light mode', () => {
      const store = useThemeStore.getState();
      store.setTheme('dark');
      expect(document.documentElement.classList.contains('dark')).toBe(true);

      store.setTheme('light');

      expect(document.documentElement.classList.contains('dark')).toBe(false);
    });
  });

  describe('3. LocalStorage Persistence', () => {
    it('TC-THM-006: persists "dark" preference to localStorage on toggle or setTheme', () => {
      const store = useThemeStore.getState();
      store.setTheme('dark');

      const stored = localStorage.getItem('book_keeper_theme') || localStorage.getItem('theme');
      expect(stored).toBe('dark');
    });

    it('TC-THM-007: persists "light" preference to localStorage', () => {
      const store = useThemeStore.getState();
      store.setTheme('dark');
      store.setTheme('light');

      const stored = localStorage.getItem('book_keeper_theme') || localStorage.getItem('theme');
      expect(stored).toBe('light');
    });
  });
});
