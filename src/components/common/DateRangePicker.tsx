import React, { useState } from 'react';
import { Calendar } from 'lucide-react';
import { cn } from '@/lib/utils';
import { addDays, parseLocalDate, startOfMonth, startOfWeek, startOfYear, todayLocal, toLocalDateString } from '@/lib/dates';

export type DatePresetId = 'all' | 'today' | '7d' | '30d' | 'week' | 'month' | 'year' | 'custom';

export interface DateRange {
  startDate: string;
  endDate: string;
}

export interface DateRangePickerProps {
  startDate?: string;
  endDate?: string;
  onChange: (range: DateRange) => void;
  /** Presets shown, in order. Default: today, week, month, year, custom. */
  presets?: DatePresetId[];
  className?: string;
}

export const DEFAULT_PRESETS: DatePresetId[] = ['today', 'week', 'month', 'year', 'custom'];

const LABELS: Record<DatePresetId, string> = {
  all: 'All',
  today: 'Today',
  '7d': 'Last 7 Days',
  '30d': 'Last 30 Days',
  week: 'This Week',
  month: 'This Month',
  year: 'This Year',
  custom: 'Custom',
};

/** Inclusive local-date range for a preset ('all' = both empty). */
export function presetRange(preset: Exclude<DatePresetId, 'custom'>, now: Date = parseLocalDate(todayLocal())): DateRange {
  const end = toLocalDateString(now);
  switch (preset) {
    case 'all':
      return { startDate: '', endDate: '' };
    case 'today':
      return { startDate: end, endDate: end };
    case '7d':
      return { startDate: addDays(now, -6), endDate: end };
    case '30d':
      return { startDate: addDays(now, -29), endDate: end };
    case 'week':
      return { startDate: startOfWeek(now), endDate: end };
    case 'month':
      return { startDate: startOfMonth(now), endDate: end };
    case 'year':
      return { startDate: startOfYear(now), endDate: end };
  }
}

/** Which preset (if any) the range corresponds to. */
export function detectPreset(startDate: string, endDate: string, presets: DatePresetId[]): DatePresetId {
  for (const p of presets) {
    if (p === 'custom') continue;
    const r = presetRange(p);
    if (r.startDate === startDate && r.endDate === endDate) return p;
  }
  return 'custom';
}

export const DateRangePicker: React.FC<DateRangePickerProps> = ({
  startDate,
  endDate,
  onChange,
  presets = DEFAULT_PRESETS,
  className,
}) => {
  const today = todayLocal();
  const start = startDate ?? today;
  const end = endDate ?? today;
  const [forceCustom, setForceCustom] = useState(false);
  const active: DatePresetId = forceCustom ? 'custom' : detectPreset(start, end, presets);

  const choose = (p: DatePresetId) => {
    if (p === 'custom') {
      setForceCustom(true);
      return;
    }
    setForceCustom(false);
    onChange(presetRange(p));
  };

  return (
    <div className={cn('flex flex-wrap items-center gap-2 text-xs', className)}>
      <div className="flex rounded-md border border-slate-300 dark:border-slate-700 p-0.5 bg-slate-100 dark:bg-slate-800">
        {presets.map((p) => (
          <button
            key={p}
            type="button"
            data-preset={p}
            aria-pressed={active === p}
            onClick={() => choose(p)}
            className={cn(
              'px-2 py-1 rounded text-xs font-medium transition-colors cursor-pointer',
              active === p
                ? 'bg-white dark:bg-slate-900 text-blue-600 dark:text-blue-400 shadow-2xs font-semibold'
                : 'text-slate-600 dark:text-slate-400 hover:text-slate-900 dark:hover:text-slate-200'
            )}
          >
            {LABELS[p]}
          </button>
        ))}
      </div>

      <div className="flex items-center gap-1.5 bg-white dark:bg-slate-900 border border-slate-300 dark:border-slate-700 rounded-md px-2.5 py-1">
        <Calendar className="h-3.5 w-3.5 text-slate-400" />
        <input
          type="date"
          aria-label="Start date"
          value={start}
          max={end || undefined}
          onChange={(e) => {
            const v = e.target.value;
            if (!v) return;
            setForceCustom(true);
            onChange({ startDate: v, endDate: end && end >= v ? end : v });
          }}
          className="bg-transparent text-xs text-slate-800 dark:text-slate-200 focus:outline-none tabular-nums font-mono"
        />
        <span className="text-slate-400">to</span>
        <input
          type="date"
          aria-label="End date"
          value={end}
          min={start || undefined}
          onChange={(e) => {
            const v = e.target.value;
            if (!v) return;
            setForceCustom(true);
            onChange({ startDate: start && start <= v ? start : v, endDate: v });
          }}
          className="bg-transparent text-xs text-slate-800 dark:text-slate-200 focus:outline-none tabular-nums font-mono"
        />
      </div>
    </div>
  );
};

export default DateRangePicker;
