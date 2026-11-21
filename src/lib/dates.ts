/** Local-time YYYY-MM-DD helpers. Never use toISOString (UTC shift, RC-9). */

const pad = (n: number) => String(n).padStart(2, '0');

export function toLocalDateString(d: Date): string {
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
}

export function todayLocal(): string {
  return toLocalDateString(new Date());
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
