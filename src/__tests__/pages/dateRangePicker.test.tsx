import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { render } from '../testUtils';
import { DateRangePicker, presetRange } from '@/components/common/DateRangePicker';
import { click } from './helpers';

describe('DateRangePicker (local dates)', () => {
  beforeEach(() => {
    // Late evening local time: toISOString() would already be tomorrow in UTC+ zones.
    vi.useFakeTimers({ toFake: ['Date'] });
    vi.setSystemTime(new Date(2026, 10, 21, 23, 30));
    document.body.innerHTML = '';
  });
  afterEach(() => vi.useRealTimers());

  it('computes inclusive local presets', () => {
    expect(presetRange('today')).toEqual({ startDate: '2026-11-21', endDate: '2026-11-21' });
    expect(presetRange('week')).toEqual({ startDate: '2026-11-16', endDate: '2026-11-21' });
    expect(presetRange('month')).toEqual({ startDate: '2026-11-01', endDate: '2026-11-21' });
    expect(presetRange('year')).toEqual({ startDate: '2026-01-01', endDate: '2026-11-21' });
    expect(presetRange('7d')).toEqual({ startDate: '2026-11-15', endDate: '2026-11-21' }); // exactly 7 days
    expect(presetRange('all')).toEqual({ startDate: '', endDate: '' });
  });

  it('starts on the Today preset and emits preset ranges on click', () => {
    const calls: unknown[] = [];
    const { container } = render(<DateRangePicker onChange={(r) => calls.push(r)} />);
    expect(container.querySelector('[data-preset="today"]')!.getAttribute('aria-pressed')).toBe('true');
    click(container.querySelector('[data-preset="month"]')!);
    click(container.querySelector('[data-preset="week"]')!);
    expect(calls).toEqual([
      { startDate: '2026-11-01', endDate: '2026-11-21' },
      { startDate: '2026-11-16', endDate: '2026-11-21' },
    ]);
  });

  it('highlights the preset matching the given range, else Custom', () => {
    const { container } = render(<DateRangePicker startDate="2026-11-01" endDate="2026-11-21" onChange={() => {}} />);
    expect(container.querySelector('[data-preset="month"]')!.getAttribute('aria-pressed')).toBe('true');
    const r2 = render(<DateRangePicker startDate="2026-03-03" endDate="2026-03-09" onChange={() => {}} />);
    expect(r2.container.querySelector('[data-preset="custom"]')!.getAttribute('aria-pressed')).toBe('true');
  });
});
