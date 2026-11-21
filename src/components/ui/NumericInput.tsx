import React from 'react';
import { TextInput, TextInputProps } from './TextInput';
import { cn } from '@/lib/utils';

export interface NumericInputProps
  extends Omit<TextInputProps, 'onChange' | 'value'> {
  value: number;
  onChange: (val: number) => void;
  allowDecimals?: boolean;
  min?: number;
  max?: number;
  precision?: number;
}

export const NumericInput = React.forwardRef<HTMLInputElement, NumericInputProps>(
  (
    {
      value,
      onChange,
      allowDecimals = true,
      min,
      max,
      precision = 2,
      autoSelectOnFocus = true,
      className,
      ...props
    },
    ref
  ) => {
    const [displayVal, setDisplayVal] = React.useState<string>(
      value !== undefined && value !== null ? String(value) : ''
    );

    React.useEffect(() => {
      setDisplayVal(value !== undefined && value !== null ? String(value) : '');
    }, [value]);

    const handleChange = (e: React.ChangeEvent<HTMLInputElement>) => {
      const raw = e.target.value;
      setDisplayVal(raw);

      if (raw === '' || raw === '-') {
        onChange(0);
        return;
      }

      let parsed = allowDecimals ? parseFloat(raw) : parseInt(raw, 10);
      if (isNaN(parsed)) {
        return;
      }

      if (min !== undefined && parsed < min) {
        parsed = min;
      }
      if (max !== undefined && parsed > max) {
        parsed = max;
      }

      onChange(parsed);
    };

    const handleBlur = (e: React.FocusEvent<HTMLInputElement>) => {
      let parsed = allowDecimals ? parseFloat(displayVal) : parseInt(displayVal, 10);
      if (isNaN(parsed)) {
        parsed = min !== undefined ? min : 0;
      } else {
        if (min !== undefined && parsed < min) parsed = min;
        if (max !== undefined && parsed > max) parsed = max;
      }
      if (allowDecimals && precision !== undefined && !isNaN(parsed)) {
        setDisplayVal(parsed.toFixed(precision));
      } else {
        setDisplayVal(String(parsed));
      }
      onChange(parsed);
      props.onBlur?.(e);
    };

    return (
      <TextInput
        ref={ref}
        type="number"
        value={displayVal}
        onChange={handleChange}
        onBlur={handleBlur}
        autoSelectOnFocus={autoSelectOnFocus}
        className={cn('tabular-nums font-mono text-right', className)}
        {...props}
      />
    );
  }
);

NumericInput.displayName = 'NumericInput';
export default NumericInput;
