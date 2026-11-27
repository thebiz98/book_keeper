import { afterEach, describe, expect, it } from 'vitest';
import { formatDateTime, formatTime, parseStamp, setShopTimeZone, shopDateOf, shopTimeZone, todayLocal } from '../lib/dates';

describe('shop-zone stamp formatters (default Asia/Kolkata)', () => {
  afterEach(() => {
    setShopTimeZone('Asia/Kolkata');
  });

  it('formats an offset stamp as 12-hour IST', () => {
    expect(formatTime('2026-11-27T09:05:00+00:00')).toBe('2:35 PM');
  });
  it('reads a zone-less stamp as UTC', () => {
    expect(formatTime('2026-11-27 09:05:00')).toBe('2:35 PM');
  });
  it('rolls the date forward past 18:30 UTC', () => {
    expect(shopDateOf('2026-11-26T19:00:00Z')).toBe('2026-11-27');
  });
  it('shows midnight and noon', () => {
    expect(formatTime('2026-11-27T18:30:00Z')).toBe('12:00 AM');
    expect(formatTime('2026-11-27T06:30:00Z')).toBe('12:00 PM');
  });
  it('formats date and time together', () => {
    expect(formatDateTime('2026-11-27T09:05:00+00:00')).toBe('27 Nov 2026, 2:35 PM');
    expect(formatDateTime(new Date('2026-11-27T09:05:00Z'))).toBe('27 Nov 2026, 2:35 PM');
  });
  it('returns empty for empty or garbage input', () => {
    for (const bad of ['', '   ', 'garbage']) {
      expect(parseStamp(bad)).toBeNull();
      expect(formatTime(bad)).toBe('');
      expect(shopDateOf(bad)).toBe('');
      expect(formatDateTime(bad)).toBe('');
    }
  });
  it('todayLocal is a YYYY-MM-DD IST date', () => {
    expect(todayLocal()).toMatch(/^\d{4}-\d{2}-\d{2}$/);
    expect(todayLocal()).toBe(shopDateOf(new Date()));
  });
});

describe('configurable shop zone', () => {
  afterEach(() => {
    setShopTimeZone('Asia/Kolkata');
  });
  it('formats in the configured zone', () => {
    expect(setShopTimeZone('UTC')).toBe(true);
    expect(formatTime('2026-11-27T09:05:00Z')).toBe('9:05 AM');
    expect(shopDateOf('2026-11-27T23:30:00Z')).toBe('2026-11-27');
  });
  it('keeps the previous zone when the name is invalid', () => {
    setShopTimeZone('UTC');
    expect(setShopTimeZone('Mars/Base')).toBe(false);
    expect(shopTimeZone()).toBe('UTC');
  });
});
