/**
 * Native file dialogs, drag-and-drop and "show in folder", wrapped so the UI degrades to a
 * typed path in browser/mock mode (no Tauri runtime there).
 */
import { isTauri } from './index';

export interface FileFilter {
  name: string;
  extensions: string[];
}

export const SPREADSHEET_FILTERS: FileFilter[] = [
  { name: 'Spreadsheet (.xlsx, .csv)', extensions: ['xlsx', 'csv', 'xls'] },
];
export const VEDBILL_FILTERS: FileFilter[] = [{ name: 'VedBill backup (.vb)', extensions: ['vb'] }];

/** True when native dialogs / file drop are available. */
export function hasNativeFiles(): boolean {
  return isTauri();
}

/** `Documents\<sub>` (or Documents itself), or undefined outside Tauri. */
export async function documentsPath(sub?: string): Promise<string | undefined> {
  if (!isTauri()) return undefined;
  try {
    const { documentDir, join } = await import('@tauri-apps/api/path');
    const docs = await documentDir();
    return sub ? await join(docs, sub) : docs;
  } catch {
    return undefined;
  }
}

/** Opens the native "Open file" dialog; resolves to the chosen path, or null if cancelled/unavailable. */
export async function pickFile(filters: FileFilter[], defaultPath?: string, title?: string): Promise<string | null> {
  if (!isTauri()) return null;
  const { open } = await import('@tauri-apps/plugin-dialog');
  const picked = await open({ multiple: false, directory: false, filters, defaultPath, title });
  return typeof picked === 'string' ? picked : null;
}

/** Opens the native "Save as" dialog; resolves to the chosen path, or null if cancelled/unavailable. */
export async function pickSavePath(filters: FileFilter[], defaultPath?: string, title?: string): Promise<string | null> {
  if (!isTauri()) return null;
  const { save } = await import('@tauri-apps/plugin-dialog');
  return (await save({ filters, defaultPath, title })) ?? null;
}

/** Opens Explorer with the file selected. Errors are swallowed (purely a convenience). */
export async function revealInFolder(path: string): Promise<void> {
  if (!isTauri()) return;
  try {
    const { revealItemInDir } = await import('@tauri-apps/plugin-opener');
    await revealItemInDir(path);
  } catch {
    /* ignore */
  }
}

export type DropState = 'over' | 'leave' | { dropped: string[] };

/**
 * Subscribes to OS file drops on the window (Tauri gives real paths, unlike HTML5 drop).
 * Returns an unsubscribe function; a no-op outside Tauri.
 */
export async function onFileDrop(handler: (s: DropState) => void): Promise<() => void> {
  if (!isTauri()) return () => {};
  try {
    const { getCurrentWebview } = await import('@tauri-apps/api/webview');
    return await getCurrentWebview().onDragDropEvent((event) => {
      const p = event.payload;
      if (p.type === 'enter' || p.type === 'over') handler('over');
      else if (p.type === 'leave') handler('leave');
      else if (p.type === 'drop') handler({ dropped: p.paths });
    });
  } catch {
    return () => {};
  }
}
