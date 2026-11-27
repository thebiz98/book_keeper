/** Local-time YYYY-MM-DD helpers. Never use toISOString (UTC shift, RC-9). */

const pad = (n: number) => String(n).padStart(2, '0');

export function toLocalDateString(d: Date): string {
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
}

export function todayLocal(): string {
  return new Intl.DateTimeFormat('en-CA', { timeZone: shopTimeZone() }).format(new Date());
}

/** Parse YYYY-MM-DD as a local date (noon-safe, no UTC shift). */
export function parseLocalDate(s: string): Date {
  const [y, m, d] = s.split('-').map(Number);
  return new Date(y, m - 1, d);
}

export function addDays(date: string | Date, days: number): string {
  const d = typeof date === 'string' ? parseLocalDate(date) : new Date(date);
  d.setDate(d.getDate() + days);
  return toLocalDateString(d);
}

/** Monday of the week containing `date`. */
export function startOfWeek(date: string | Date = new Date()): string {
  const d = typeof date === 'string' ? parseLocalDate(date) : new Date(date);
  d.setDate(d.getDate() - ((d.getDay() + 6) % 7));
  return toLocalDateString(d);
}

export function startOfMonth(date: string | Date = new Date()): string {
  const d = typeof date === 'string' ? parseLocalDate(date) : new Date(date);
  return toLocalDateString(new Date(d.getFullYear(), d.getMonth(), 1));
}

export function startOfYear(date: string | Date = new Date()): string {
  const d = typeof date === 'string' ? parseLocalDate(date) : new Date(date);
  return toLocalDateString(new Date(d.getFullYear(), 0, 1));
}

const DEFAULT_TZ = 'Asia/Kolkata';
let shopTz = DEFAULT_TZ;

/** The shop time zone (from app config `timeZone`), independent of the PC's zone. */
export function shopTimeZone(): string {
  return shopTz;
}

/** Sets the shop zone; an invalid name is ignored. Returns true when the zone changed. */
export function setShopTimeZone(tz: string): boolean {
  try {
    new Intl.DateTimeFormat('en-IN', { timeZone: tz });
  } catch {
    return false;
  }
  const changed = tz !== shopTz;
  shopTz = tz;
  return changed;
}

/**
 * Parses a stored timestamp. Zone-less stamps (SQLite `YYYY-MM-DD HH:MM:SS`) are UTC.
 * Returns null when empty or invalid.
 */
export function parseStamp(ts: string): Date | null {
  if (typeof ts !== 'string') return null;
  let s = ts.trim();
  if (!s) return null;
  if (!/(Z|[+-]\d{2}:?\d{2})$/i.test(s)) {
    s = `${s.replace(' ', 'T')}Z`;
  }
  const d = new Date(s);
  return Number.isNaN(d.getTime()) ? null : d;
}

function toDate(ts: string | Date | null | undefined): Date | null {
  if (ts == null) return null;
  if (ts instanceof Date) return Number.isNaN(ts.getTime()) ? null : ts;
  return parseStamp(ts);
}

function istTime(d: Date): string {
  return new Intl.DateTimeFormat('en-IN', {
    timeZone: shopTimeZone(),
    hour: 'numeric',
    minute: '2-digit',
    hour12: true,
  })
    .format(d)
    .replace(/\s*(am|pm)$/i, (_m, p: string) => ` ${p.toUpperCase()}`);
}

/** `2:35 PM` in India time; empty when null/invalid. */
export function formatTime(ts: string | null | undefined): string {
  const d = toDate(ts);
  return d ? istTime(d) : '';
}

/** `YYYY-MM-DD` of the instant in India time; empty when invalid. */
export function shopDateOf(ts: string | Date | null | undefined): string {
  const d = toDate(ts);
  return d ? new Intl.DateTimeFormat('en-CA', { timeZone: shopTimeZone() }).format(d) : '';
}

/** `27 Nov 2026, 2:35 PM` in India time; empty when invalid. */
export function formatDateTime(ts: string | Date | null | undefined): string {
  const d = toDate(ts);
  if (!d) return '';
  const date = new Intl.DateTimeFormat('en-GB', {
    timeZone: shopTimeZone(),
    day: 'numeric',
    month: 'short',
    year: 'numeric',
  }).format(d);
  return `${date}, ${istTime(d)}`;
}
