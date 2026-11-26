import React, { useEffect, useRef, useState } from 'react';
import { AlertTriangle, AlertCircle, Info } from 'lucide-react';
import { Modal } from '../ui/Modal';
import { Button } from '../ui/Button';
import { TextInput } from '../ui/TextInput';

export interface ConfirmDialogProps {
  isOpen: boolean;
  title: string;
  message: string;
  confirmText?: string;
  cancelText?: string;
  variant?: 'danger' | 'warning' | 'primary';
  /** May return a Promise: the dialog then stays busy (Confirm loading, Cancel/Esc off, repeats ignored) until it settles. */
  onConfirm: () => void | Promise<unknown>;
  onCancel: () => void;
  /**
   * Typed confirmation (FR-20 deletes): Confirm stays disabled, and Enter does nothing, until the
   * trimmed text typed by the user equals this exactly.
   */
  requireText?: string;
  /** Keeps Confirm (and Enter) off, e.g. when the backend says the action is not allowed. */
  confirmDisabled?: boolean;
  /** Extra body content under the message (e.g. an inline error). */
  children?: React.ReactNode;
}

export const ConfirmDialog: React.FC<ConfirmDialogProps> = ({
  isOpen,
  title,
  message,
  confirmText = 'Confirm',
  cancelText = 'Cancel',
  variant = 'primary',
  onConfirm,
  onCancel,
  requireText,
  confirmDisabled = false,
  children,
}) => {
  const [typed, setTyped] = useState('');
  useEffect(() => {
    if (isOpen) setTyped('');
  }, [isOpen, requireText]);

  const canConfirm = !confirmDisabled && (requireText === undefined || typed.trim() === requireText);
  const pendingRef = useRef(false);
  const [pending, setPending] = useState(false);
  const mounted = useRef(true);
  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
    };
  }, []);
  // One guarded path for the button click, the Enter hotkey and the Modal default action.
  const confirm = () => {
    if (!canConfirm || pendingRef.current) return;
    const r = onConfirm();
    if (r && typeof (r as Promise<unknown>).then === 'function') {
      pendingRef.current = true;
      setPending(true);
      const done = () => {
        pendingRef.current = false;
        if (mounted.current) setPending(false);
      };
      (r as Promise<unknown>).then(done, done);
    }
  };
  const cancel = () => {
    if (!pendingRef.current) onCancel();
  };

  const getIcon = () => {
    switch (variant) {
      case 'danger':
        return <AlertCircle className="h-5 w-5 text-rose-600 shrink-0" />;
      case 'warning':
        return <AlertTriangle className="h-5 w-5 text-amber-600 shrink-0" />;
      default:
        return <Info className="h-5 w-5 text-blue-600 shrink-0" />;
    }
  };

  const getButtonVariant = () => {
    if (variant === 'danger') return 'danger';
    return 'primary';
  };

  return (
    <Modal
      isOpen={isOpen}
      onClose={cancel}
      title={title}
      size="sm"
      defaultConfirmAction={canConfirm ? confirm : undefined}
      footer={
        <>
          <Button variant="outline" size="sm" onClick={cancel} disabled={pending} hotkey="Esc">
            {cancelText}
          </Button>
          <Button variant={getButtonVariant()} size="sm" onClick={confirm} disabled={!canConfirm} isLoading={pending} hotkey="Enter">
            {confirmText}
          </Button>
        </>
      }
    >
      <div className="flex items-start gap-3 py-1">
        {getIcon()}
        <p className="text-xs text-slate-600 dark:text-slate-300 leading-relaxed">{message}</p>
      </div>
      {requireText !== undefined && (
        <div className="mt-3">
          <TextInput
            label={`Type ${requireText} to confirm`}
            value={typed}
            onChange={(e) => setTyped(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === 'Enter') {
                e.preventDefault();
                confirm();
              }
            }}
            autoFocus
            autoComplete="off"
            data-testid="confirm-require-text"
          />
        </div>
      )}
      {children}
    </Modal>
  );
};

export default ConfirmDialog;
