import React, { useState } from 'react';
import { Calendar } from 'lucide-react';
import { cn } from '@/lib/utils';

export interface DateRangePickerProps {
  startDate?: string;
  endDate?: string;
  onChange: (range: { startDate: string; endDate: string }) => void;
  className?: string;
}

export const DateRangePicker: React.FC<DateRangePickerProps> = ({
  startDate = new Date().toISOString().split('T')[0],
  endDate = new Date().toISOString().split('T')[0],
  onChange,
  className,
}) => {
  const [activePreset, setActivePreset] = useState<string>('today');

  const applyPreset = (preset: string) => {
    setActivePreset(preset);
    const today = new Date();
    const end = today.toISOString().split('T')[0];
    let start = end;

    if (preset === 'today') {
      start = end;
    } else if (preset === '7d') {
      const d = new Date();
      d.setDate(d.getDate() - 7);
      start = d.toISOString().split('T')[0];
    } else if (preset === '30d') {
      const d = new Date();
      d.setDate(d.getDate() - 30);
      start = d.toISOString().split('T')[0];
    } else if (preset === 'year') {
      const d = new Date();
      d.setFullYear(d.getFullYear() - 1);
      start = d.toISOString().split('T')[0];
    }

    onChange({ startDate: start, endDate: end });
  };

  return (
    <div className={cn('flex flex-wrap items-center gap-2 text-xs', className)}>
      <div className="flex rounded-md border border-slate-300 dark:border-slate-700 p-0.5 bg-slate-100 dark:bg-slate-800">
        {[
          { id: 'today', label: 'Today' },
          { id: '7d', label: '7 Days' },
          { id: '30d', label: '30 Days' },
          { id: 'year', label: '1 Year' },
        ].map((p) => (
          <button
            key={p.id}
            type="button"
            onClick={() => applyPreset(p.id)}
            className={cn(
              'px-2 py-1 rounded text-xs font-medium transition-colors cursor-pointer',
              activePreset === p.id
                ? 'bg-white dark:bg-slate-900 text-blue-600 dark:text-blue-400 shadow-2xs font-semibold'
                : 'text-slate-600 dark:text-slate-400 hover:text-slate-900 dark:hover:text-slate-200'
            )}
          >
            {p.label}
          </button>
        ))}
      </div>

      <div className="flex items-center gap-1.5 bg-white dark:bg-slate-900 border border-slate-300 dark:border-slate-700 rounded-md px-2.5 py-1">
        <Calendar className="h-3.5 w-3.5 text-slate-400" />
        <input
          type="date"
          value={startDate}
          onChange={(e) => {
            setActivePreset('custom');
            onChange({ startDate: e.target.value, endDate });
          }}
          className="bg-transparent text-xs text-slate-800 dark:text-slate-200 focus:outline-none tabular-nums font-mono"
        />
        <span className="text-slate-400">to</span>
        <input
          type="date"
          value={endDate}
          onChange={(e) => {
            setActivePreset('custom');
            onChange({ startDate, endDate: e.target.value });
          }}
          className="bg-transparent text-xs text-slate-800 dark:text-slate-200 focus:outline-none tabular-nums font-mono"
        />
      </div>
    </div>
  );
};

export default DateRangePicker;
