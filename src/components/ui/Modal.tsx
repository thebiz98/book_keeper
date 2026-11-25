import React, { useId, useRef } from 'react';
import { X } from 'lucide-react';
import { cn } from '@/lib/utils';
import { useModalLayer } from '@/hooks/useHotkeys';
import { useFocusTrap } from '@/hooks/useFocusTrap';

export interface ModalProps {
  isOpen: boolean;
  onClose: () => void;
  title: string;
  description?: string;
  children: React.ReactNode;
  footer?: React.ReactNode;
  size?: 'sm' | 'md' | 'lg' | 'xl' | 'full';
  /** Enter (when focus is not in an input, button or link) runs this once. */
  defaultConfirmAction?: () => void;
  className?: string;
  /** Extra classes for the body (e.g. `overflow-hidden` when the content manages its own scroll). */
  bodyClassName?: string;
  /** Element to focus on open; default is the first control in the body, else the dialog. */
  initialFocusRef?: React.RefObject<HTMLElement | null>;
  /** Backdrop click closes the modal (default true). */
  closeOnBackdrop?: boolean;
}

export const Modal: React.FC<ModalProps> = ({
  isOpen,
  onClose,
  title,
  description,
  children,
  footer,
  size = 'md',
  defaultConfirmAction,
  className,
  bodyClassName,
  initialFocusRef,
  closeOnBackdrop = true,
}) => {
  const modalRef = useRef<HTMLDivElement>(null);
  const titleId = useId();
  const descId = useId();

  // Refs keep the layer/trap effects stable across parent re-renders.
  const onCloseRef = useRef(onClose);
  onCloseRef.current = onClose;
  const confirmRef = useRef(defaultConfirmAction);
  confirmRef.current = defaultConfirmAction;

  useModalLayer(() => onCloseRef.current(), isOpen);
  useFocusTrap(modalRef, isOpen, initialFocusRef);

  const handleKeyDown = (e: React.KeyboardEvent<HTMLDivElement>) => {
    if (e.key !== 'Enter' || e.defaultPrevented || e.repeat) return;
    const confirm = confirmRef.current;
    if (!confirm) return;
    const target = e.target as HTMLElement;
    const tag = target.tagName;
    if (
      tag === 'INPUT' ||
      tag === 'TEXTAREA' ||
      tag === 'SELECT' ||
      tag === 'BUTTON' ||
      tag === 'A' ||
      target.isContentEditable
    ) {
      return;
    }
    e.preventDefault();
    e.stopPropagation();
    confirm();
  };

  if (!isOpen) return null;

  const sizes = {
    sm: 'max-w-sm',
    md: 'max-w-md',
    lg: 'max-w-lg',
    xl: 'max-w-2xl',
    full: 'max-w-5xl',
  };

  return (
    <div
      className="fixed inset-0 bg-black/60 backdrop-blur-xs z-50 flex items-center justify-center p-4 overflow-y-auto"
      onClick={(e) => {
        if (closeOnBackdrop && e.target === e.currentTarget) onClose();
      }}
    >
      <div
        ref={modalRef}
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        aria-describedby={description ? descId : undefined}
        tabIndex={-1}
        onKeyDown={handleKeyDown}
        className={cn(
          'bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 rounded-lg shadow-2xl w-full max-h-[90vh] flex flex-col overflow-hidden animate-in fade-in zoom-in-95 duration-150',
          sizes[size],
          className
        )}
      >
        {/* Header */}
        <div className="flex items-center justify-between px-5 py-3.5 border-b border-slate-100 dark:border-slate-800 shrink-0">
          <div>
            <h2 id={titleId} className="text-base font-semibold text-slate-900 dark:text-slate-100">
              {title}
            </h2>
            {description && (
              <p id={descId} className="text-xs text-slate-500 dark:text-slate-400 mt-0.5">{description}</p>
            )}
          </div>
          <button
            type="button"
            onClick={onClose}
            aria-label="Close"
            data-modal-close="true"
            className="p-1 rounded-md text-slate-400 hover:text-slate-600 dark:text-slate-500 dark:hover:text-slate-300 hover:bg-slate-100 dark:hover:bg-slate-800 transition-colors"
          >
            <X className="h-4 w-4" />
          </button>
        </div>

        {/* Body */}
        <div data-modal-body className={cn('p-5 overflow-y-auto flex-1', bodyClassName)}>{children}</div>

        {/* Footer */}
        {footer && (
          <div className="px-5 py-3 bg-slate-50 dark:bg-slate-900/50 border-t border-slate-100 dark:border-slate-800 shrink-0 flex items-center justify-end gap-2">
            {footer}
          </div>
        )}
      </div>
    </div>
  );
};

export default Modal;
