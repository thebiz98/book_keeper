import { isAppError } from '@/api/errors';

/** Parse an optional money/number field: empty => null, otherwise the number (NaN => NaN). */
export function parseOptional(raw: string): number | null {
  const t = raw.trim();
  if (t === '') return null;
  return Number(t);
}

/** Map a backend ValidationError onto a field key; null when the error is not field-scoped. */
export function validationField(e: unknown): string | null {
  if (isAppError(e) && e.type === 'ValidationError') {
    const f = e.details?.field;
    return typeof f === 'string' && f ? f : null;
  }
  return null;
}
