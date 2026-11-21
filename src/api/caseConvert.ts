/**
 * Key-case converters for the Tauri IPC boundary.
 * Rust serializes snake_case; the frontend works in camelCase.
 */

const toCamel = (s: string): string =>
  s.replace(/_([a-z0-9])/g, (_, c: string) => c.toUpperCase());

const toSnake = (s: string): string =>
  s.replace(/[A-Z]/g, (c) => `_${c.toLowerCase()}`);

function isPlainObject(v: unknown): v is Record<string, unknown> {
  return typeof v === 'object' && v !== null && !Array.isArray(v) && !(v instanceof Date);
}

function convertKeys(value: unknown, fn: (k: string) => string): unknown {
  if (Array.isArray(value)) return value.map((v) => convertKeys(v, fn));
  if (isPlainObject(value)) {
    const out: Record<string, unknown> = {};
    for (const [k, v] of Object.entries(value)) out[fn(k)] = convertKeys(v, fn);
    return out;
  }
  return value;
}

/** Deep-convert object keys snake_case -> camelCase (used on responses). */
export function snakeToCamel<T = unknown>(value: unknown): T {
  return convertKeys(value, toCamel) as T;
}

/** Deep-convert object keys camelCase -> snake_case (used on request payloads). */
export function camelToSnake<T = unknown>(value: unknown): T {
  return convertKeys(value, toSnake) as T;
}
