import React from 'react';
import { Button } from './Button';
import { cn } from '@/lib/utils';

export interface FormActionsProps {
  onSave: () => void;
  onCancel: () => void;
  saveLabel?: string;
  cancelLabel?: string;
  isSubmitting?: boolean;
  canSave?: boolean;
  saveHotkey?: string;
  cancelHotkey?: string;
  size?: 'xs' | 'sm' | 'md';
  className?: string;
}

/** Visible Save / Cancel pair for quick-create forms (FR-22, DD-44). */
export const FormActions: React.FC<FormActionsProps> = ({
  onSave,
  onCancel,
  saveLabel = 'Save',
  cancelLabel = 'Cancel',
  isSubmitting = false,
  canSave = true,
  saveHotkey = 'Enter',
  cancelHotkey = 'Esc',
  size = 'sm',
  className,
}) => (
  <div data-form-actions className={cn('flex items-center justify-end gap-2', className)}>
    <Button
      type="button"
      variant="outline"
      size={size}
      onClick={onCancel}
      disabled={isSubmitting}
      hotkey={cancelHotkey}
    >
      {cancelLabel}
    </Button>
    <Button
      type="button"
      variant="primary"
      size={size}
      onClick={onSave}
      disabled={!canSave || isSubmitting}
      isLoading={isSubmitting}
      hotkey={saveHotkey}
    >
      {saveLabel}
    </Button>
  </div>
);

/**
 * onKeyDown handler for the wrapper around form fields: plain Enter or Ctrl+Enter submits.
 * Fields with data-enter-submit="off" (scanner barcode inputs) swallow Enter without submitting.
 */
export function enterToSubmit(submit: () => void) {
  return (e: React.KeyboardEvent) => {
    if (e.key !== 'Enter' || e.defaultPrevented || e.repeat || e.shiftKey || e.altKey) return;
    const t = e.target as HTMLElement;
    if (['TEXTAREA', 'SELECT', 'BUTTON'].includes(t.tagName)) return;
    e.preventDefault();
    e.stopPropagation();
    if (t.closest('[data-enter-submit="off"]') || t.getAttribute('data-enter-submit') === 'off') return;
    submit();
  };
}

export default FormActions;
