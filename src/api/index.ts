/**
 * Book Keeper API Client Singleton & Runtime Mode Detection.
 * Automatically selects TauriClient when running inside Tauri v2, or MockApiClient in browser/test mode.
 */

import type { ApiClient } from '@/types/api';
import { TauriClient } from './ipc';
import { MockApiClient } from './mock/mockClient';

export { TauriClient } from './ipc';
export { isAppError, errorMessage, normalizeError, APP_ERROR_TYPES } from './errors';
export type { AppError } from './errors';
export { MockApiClient, mockApiClient } from './mock/mockClient';

/**
 * Checks if the application is running within the Tauri desktop container.
 */
export function isTauri(): boolean {
  if (typeof window === 'undefined') {
    return false;
  }
  return '__TAURI_INTERNALS__' in window || '__TAURI__' in window;
}

let activeApiClient: ApiClient | null = null;

/**
 * Retrieves or lazily initializes the singleton ApiClient.
 */
export function getApiClient(): ApiClient {
  if (!activeApiClient) {
    if (isTauri()) {
      activeApiClient = new TauriClient();
    } else {
      activeApiClient = new MockApiClient();
    }
  }
  return activeApiClient;
}

/**
 * Explicit setter to override the active ApiClient instance (useful in integration tests).
 */
export function setApiClient(client: ApiClient): void {
  activeApiClient = client;
}

/**
 * Transparent proxy providing singleton access to the active ApiClient.
 */
export const api: ApiClient = new Proxy({} as ApiClient, {
  get(_target, prop: string | symbol) {
    const client = getApiClient();
    const value: unknown = Reflect.get(client, prop);
    return typeof value === "function" ? value.bind(client) : value;
  },
});

export default api;
