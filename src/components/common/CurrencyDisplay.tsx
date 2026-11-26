import React from 'react';
import { cn } from '@/lib/utils';
import { round2 } from '@/lib/money';

export interface CurrencyDisplayProps {
  amount: number;
  currencySymbol?: string;
  size?: 'sm' | 'md' | 'lg' | 'xl' | 'hero';
  colored?: boolean;
  showSign?: boolean;
  /** With `colored`: positive is red and negative green (money owed by the shop, e.g. vendor dues). */
  invertColors?: boolean;
  className?: string;
}

export const CurrencyDisplay: React.FC<CurrencyDisplayProps> = ({
  amount = 0,
  currencySymbol = '₹',
  size = 'md',
  colored = false,
  showSign = false,
  invertColors = false,
  className,
}) => {
  // Round first so sub-paisa noise never renders as "-₹0.00".
  const rounded = round2(Number.isFinite(amount) ? amount : 0);
  const isPositive = rounded > 0;
  const isNegative = rounded < 0;

  // Format absolute value with Indian number formatting (en-IN)
  const absAmount = Math.abs(rounded);
  const formattedNumber = new Intl.NumberFormat('en-IN', {
    minimumFractionDigits: 2,
    maximumFractionDigits: 2,
  }).format(absAmount);

  let signPrefix = '';
  if (isNegative) {
    signPrefix = '-';
  } else if (showSign && isPositive) {
    signPrefix = '+';
  }

  const sizes = {
    sm: 'text-xs',
    md: 'text-sm',
    lg: 'text-lg',
    xl: 'text-xl',
    hero: 'text-3xl sm:text-4xl font-bold',
  };

  let colorClass = 'text-slate-900 dark:text-slate-100';
  if (colored) {
    const good = 'text-emerald-600 dark:text-emerald-400';
    const bad = 'text-rose-600 dark:text-rose-400';
    if (isPositive) {
      colorClass = invertColors ? bad : good;
    } else if (isNegative) {
      colorClass = invertColors ? good : bad;
    }
  }

  return (
    <span
      className={cn(
        'tabular-nums font-mono inline-block',
        sizes[size],
        colorClass,
        className
      )}
    >
      {signPrefix}
      {currencySymbol}
      {formattedNumber}
    </span>
  );
};

export default CurrencyDisplay;
