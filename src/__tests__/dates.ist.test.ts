import { describe, expect, it } from 'vitest';
import { formatIstDateTime, formatIstTime, istDateOf, parseStamp, todayLocal } from '../lib/dates';

describe('India-time stamp formatters', () => {
  it('formats an offset stamp as 12-hour IST', () => {
    expect(formatIstTime('2026-11-27T09:05:00+00:00')).toBe('2:35 PM');
  });
  it('reads a zone-less stamp as UTC', () => {
    expect(formatIstTime('2026-11-27 09:05:00')).toBe('2:35 PM');
  });
  it('rolls the date forward past 18:30 UTC', () => {
    expect(istDateOf('2026-11-26T19:00:00Z')).toBe('2026-11-27');
  });
  it('shows midnight and noon', () => {
    expect(formatIstTime('2026-11-27T18:30:00Z')).toBe('12:00 AM');
    expect(formatIstTime('2026-11-27T06:30:00Z')).toBe('12:00 PM');
  });
  it('formats date and time together', () => {
    expect(formatIstDateTime('2026-11-27T09:05:00+00:00')).toBe('27 Nov 2026, 2:35 PM');
    expect(formatIstDateTime(new Date('2026-11-27T09:05:00Z'))).toBe('27 Nov 2026, 2:35 PM');
  });
  it('returns empty for empty or garbage input', () => {
    for (const bad of ['', '   ', 'garbage']) {
      expect(parseStamp(bad)).toBeNull();
      expect(formatIstTime(bad)).toBe('');
      expect(istDateOf(bad)).toBe('');
      expect(formatIstDateTime(bad)).toBe('');
    }
  });
  it('todayLocal is a YYYY-MM-DD IST date', () => {
    expect(todayLocal()).toMatch(/^\d{4}-\d{2}-\d{2}$/);
    expect(todayLocal()).toBe(istDateOf(new Date()));
  });
});
