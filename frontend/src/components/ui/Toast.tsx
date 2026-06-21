import { useEffect } from 'react';
import { create } from 'zustand';
import './Toast.css';

/** Toast notification types matching the dark luxury aesthetic. */
export type ToastType = 'success' | 'info' | 'warning' | 'error';

interface ToastItem {
  readonly id: string;
  readonly type: ToastType;
  readonly message: string;
  readonly createdAt: number;
}

interface ToastStore {
  readonly toasts: readonly ToastItem[];
  addToast: (type: ToastType, message: string) => void;
  removeToast: (id: string) => void;
}

let toastCounter = 0;

/** Global toast store — accessible from hooks and plain functions. */
export const useToastStore = create<ToastStore>((set) => ({
  toasts: [],
  addToast: (type, message) => {
    const id = `toast-${++toastCounter}-${Date.now()}`;
    const toast: ToastItem = { id, type, message, createdAt: Date.now() };
    set((state) => ({ toasts: [...state.toasts, toast] }));
  },
  removeToast: (id) => {
    set((state) => ({ toasts: state.toasts.filter((t) => t.id !== id) }));
  },
}));

/** Convenience function for adding toasts without needing the hook. */
export function addToast(type: ToastType, message: string): void {
  useToastStore.getState().addToast(type, message);
}

const AUTO_DISMISS_MS = 4000;

/** Single toast notification item. */
function ToastItem({ toast, onDismiss }: { readonly toast: ToastItem; readonly onDismiss: (id: string) => void }) {
  useEffect(() => {
    const timer = setTimeout(() => {
      onDismiss(toast.id);
    }, AUTO_DISMISS_MS);
    return () => clearTimeout(timer);
  }, [toast.id, onDismiss]);

  return (
    <div
      className={`toast-item toast-item--${toast.type}`}
      role="alert"
      aria-live="polite"
    >
      <div className="toast-item__icon" aria-hidden="true">
        {toast.type === 'success' && (
          <svg width="16" height="16" viewBox="0 0 16 16" fill="none">
            <circle cx="8" cy="8" r="7" stroke="currentColor" strokeWidth="1.5" />
            <polyline points="5 8 7 10 11 6" stroke="currentColor" strokeWidth="1.5" fill="none" strokeLinecap="round" strokeLinejoin="round" />
          </svg>
        )}
        {toast.type === 'info' && (
          <svg width="16" height="16" viewBox="0 0 16 16" fill="none">
            <circle cx="8" cy="8" r="7" stroke="currentColor" strokeWidth="1.5" />
            <line x1="8" y1="7" x2="8" y2="11" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
            <circle cx="8" cy="5" r="0.75" fill="currentColor" />
          </svg>
        )}
        {toast.type === 'warning' && (
          <svg width="16" height="16" viewBox="0 0 16 16" fill="none">
            <path d="M8 2L14.5 13H1.5L8 2Z" stroke="currentColor" strokeWidth="1.5" strokeLinejoin="round" />
            <line x1="8" y1="6" x2="8" y2="9.5" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
            <circle cx="8" cy="11.5" r="0.75" fill="currentColor" />
          </svg>
        )}
        {toast.type === 'error' && (
          <svg width="16" height="16" viewBox="0 0 16 16" fill="none">
            <circle cx="8" cy="8" r="7" stroke="currentColor" strokeWidth="1.5" />
            <line x1="5.5" y1="5.5" x2="10.5" y2="10.5" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
            <line x1="10.5" y1="5.5" x2="5.5" y2="10.5" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
          </svg>
        )}
      </div>
      <span className="toast-item__message">{toast.message}</span>
      <button
        className="toast-item__close"
        onClick={() => onDismiss(toast.id)}
        aria-label="Dismiss notification"
        type="button"
      >
        <svg width="12" height="12" viewBox="0 0 12 12" fill="none">
          <line x1="2" y1="2" x2="10" y2="10" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
          <line x1="10" y1="2" x2="2" y2="10" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
        </svg>
      </button>
    </div>
  );
}

/** Toast container — renders all active toasts, positioned bottom-right. */
export function ToastContainer() {
  const toasts = useToastStore((s) => s.toasts);
  const removeToast = useToastStore((s) => s.removeToast);

  if (toasts.length === 0) return null;

  return (
    <div className="toast-container" aria-label="Notifications">
      {toasts.map((toast) => (
        <ToastItem key={toast.id} toast={toast} onDismiss={removeToast} />
      ))}
    </div>
  );
}
