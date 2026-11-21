import React from 'react';
import { cn } from '@/lib/utils';

export interface HotkeyBadgeProps {
  hotkey: string;
  size?: 'xs' | 'sm' | 'md';
  className?: string;
}

export const HotkeyBadge: React.FC<HotkeyBadgeProps> = ({
  hotkey,
  size = 'sm',
  className,
}) => {
  const sizes = {
    xs: 'px-1 py-0.2 text-[9px]',
    sm: 'px-1.5 py-0.5 text-[10px]',
    md: 'px-2 py-0.5 text-xs',
  };

  return (
    <kbd
      className={cn(
        'inline-flex items-center font-mono font-semibold rounded uppercase tracking-wider bg-slate-200 dark:bg-slate-800 text-slate-700 dark:text-slate-300 border border-slate-300 dark:border-slate-700 shadow-2xs select-none',
        sizes[size],
        className
      )}
    >
      {hotkey}
    </kbd>
  );
};

export default HotkeyBadge;
