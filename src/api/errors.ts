/**
 * Typed error contract for Tauri IPC (DD-26, LLD 10.1).
 * Mirrors Rust `AppError` serialized as `{ type, details }`.
 * `details` keys are left snake_case exactly as sent by the backend (not case-converted).
 * Messages are derived here: Rust does not send a `message` for struct variants other
 * than ValidationError.
 */
export type AppError =
  | { type: 'ValidationError'; details: { field: string; message: string } }
  | {
      type: 'InsufficientStockError';
      details: { item_id: number; item_name: string; required: number; available: number };
    }
  | { type: 'NotFound'; details: { entity: string; id: string } }
  | { type: 'ConcurrencyConflict'; details: string }
  | { type: 'DatabaseError'; details: string }
  | { type: 'SearchIndexError'; details: string }
  | { type: 'MigrationError'; details: string }
  | { type: 'IoError'; details: string }
  | { type: 'OperationNotAllowed'; details: string }
  | { type: 'BillLocked'; details: { code: string; message: string } }
  /** FR-45 / FR-46: the name belongs to a deleted record; `existing_id` can be restored. */
  | { type: 'DeletedNameClash'; details: { field: string; code: string; existing_id: number; message: string } };

/** The deleted record behind a `DeletedNameClash` (e.g. `ITEM_NAME_DELETED`), else null. */
export function deletedNameClash(e: unknown): { code: string; existingId: number; message: string } | null {
  if (typeof e === 'object' && e !== null && (e as { type?: unknown }).type === 'DeletedNameClash') {
    const d = (e as { details: { code: string; existing_id: number; message: string } }).details;
    return { code: d.code, existingId: d.existing_id, message: d.message };
  }
  return null;
}

export const APP_ERROR_TYPES = [
  'ValidationError',
  'InsufficientStockError',
  'NotFound',
  'ConcurrencyConflict',
  'DatabaseError',
  'SearchIndexError',
  'MigrationError',
  'IoError',
  'OperationNotAllowed',
  'BillLocked',
  'DeletedNameClash',
] as const;

export function isAppError(x: unknown): x is AppError {
  if (typeof x !== 'object' || x === null) return false;
  const t = (x as { type?: unknown }).type;
  return typeof t === 'string' && (APP_ERROR_TYPES as readonly string[]).includes(t) && 'details' in x;
}

/** Human-readable message for any thrown value. */
export function errorMessage(e: unknown): string {
  if (isAppError(e)) {
    const d: unknown = e.details;
    if (typeof d === 'string') return d;
    if (d && typeof d === 'object') {
      const o = d as Record<string, unknown>;
      if (typeof o.message === 'string' && o.message) return o.message;
      if (e.type === 'InsufficientStockError') {
        return `Insufficient stock for item '${o.item_name}'. Required: ${o.required}, Available: ${o.available}`;
      }
      if (e.type === 'NotFound') {
        return `Entity not found: ${o.entity} with identifier ${o.id}`;
      }
    }
    return e.type;
  }
  if (e instanceof Error) return e.message;
  if (typeof e === 'object' && e !== null && typeof (e as { message?: unknown }).message === 'string') {
    return (e as { message: string }).message;
  }
  return String(e);
}

/**
 * Normalises whatever Tauri delivers (object, JSON string, plain string, Error)
 * into an AppError. Unknown shapes become `IoError`-free `DatabaseError`-like
 * fallbacks only when nothing better exists: we use `IoError` for transport failures.
 */
export function normalizeError(err: unknown, cmd = 'command'): AppError {
  let v: unknown = err;
  if (typeof v === 'string') {
    try {
      v = JSON.parse(v);
    } catch {
      return { type: 'IoError', details: v as string };
    }
  }
  if (isAppError(v)) return v;
  return { type: 'IoError', details: errorMessage(v) || `IPC command "${cmd}" failed` };
}
