import { clsx, type ClassValue } from 'clsx';
import { twMerge } from 'tailwind-merge';

export function cn(...inputs: ClassValue[]): string {
  return twMerge(clsx(inputs));
}

/** Keyboard-selected row/option: clear blue fill plus a left bar (hover stays light slate). */
export const HIGHLIGHT_ROW =
  'bg-blue-100 dark:bg-blue-900/50 text-blue-950 dark:text-blue-50 shadow-[inset_3px_0_0_0_#3b82f6]';
